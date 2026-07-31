use std::collections::HashSet;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use dashmap::DashMap;
use socketioxide::socket::Sid;
use socketioxide::SocketIo;
use tokio::sync::broadcast;
use tokio::sync::mpsc::UnboundedSender;

use crate::account::{AccountService, Charge};
use crate::config::Config;
use crate::http::ipfilter::IpRuleSet;
use crate::http::ratelimit_mw::IpLimiter;
use crate::logging::{LogSink, LogStore};
use crate::metrics::{LiveEvent, QueueWaitStat, RequestRing};
use crate::pool::registry::Registry;
use crate::store::Store;
use crate::stream::StreamEvent;
use crate::ws::protocol::Cancel;

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 一个在途请求的服务端侧状态。计费句柄放在这里（而非 HTTP 响应 future）——
/// 使结算/退款由 **worker 的终止事件** 驱动，不依赖消费方是否还连着（治「断连即免费」#1）。
pub struct PendingEntry {
    pub tx: UnboundedSender<StreamEvent>,
    pub worker_id: String,
    /// 派发目标 socket，用于发 cancel（客户端断开/超时时中止 worker 运行）。
    pub sid: Sid,
    pub created_at: i64,
    /// 计费预扣句柄；done 时结算、失败时退款（取走一次）。未取走即随 entry drop → Charge::Drop 自动退款。
    pub charge: std::sync::Mutex<Option<Charge>>,
    /// 已向消费方路由出去的正文字节数（route 收到非空 Delta 时累加）：
    /// done.full 空但 deltas 已送达时仍判有内容（治「送了正文却退款」#16）。
    pub streamed_chars: AtomicU64,
}

/// 全进程共享状态。在 main 的启动钩子内构造（避免 lazy 单例竞态）。
pub struct AppState {
    pub config: Arc<Config>,
    pub registry: Registry,
    /// req_id → 在途请求。
    pub pending: DashMap<String, PendingEntry>,
    /// worker_id → 它名下在途 req_id 集合（P1#6：断连时 O(1) 取列表，免全表扫 pending 且杜绝迭代期漏剔）。
    pub worker_index: DashMap<String, HashSet<String>>,
    /// 用于向指定 worker socket 派发 dispatch / cancel。
    pub io: SocketIo,
    /// 中转状态持久化（会话亲和 / worker 身份）。
    pub store: Arc<dyn Store>,
    /// 日志投递（非阻塞）。
    pub log_sink: LogSink,
    /// 日志查询（admin 用）。
    pub log_store: Arc<dyn LogStore>,
    /// 用户体系 / 鉴权 / 计费 / 限流。
    pub account: AccountService,
    /// 实时事件广播（admin SSE 订阅）。
    pub events: broadcast::Sender<LiveEvent>,
    /// 请求计数环（24h，始终在线，metrics 用）。
    pub req_ring: RequestRing,
    /// 排队等待滚动均值。
    pub queue_wait: QueueWaitStat,
    /// /auth/* per-IP 限流（防注册/登录爆破）。
    pub auth_ip_limiter: IpLimiter,
    /// /worker/claim per-IP 限流（领取身份罕见，单独一桶）。
    pub worker_claim_limiter: IpLimiter,
    /// 预编译的 IP allow/deny 规则（启动时一次性解析）。
    pub ip_rules: IpRuleSet,
    /// 图片 URL fetch 全局并发闸（独立于 per-user 限流，兜底内存放大）。
    pub image_fetch_sem: tokio::sync::Semaphore,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: Arc<Config>,
        io: SocketIo,
        store: Arc<dyn Store>,
        log_sink: LogSink,
        log_store: Arc<dyn LogStore>,
        account: AccountService,
        ip_rules: IpRuleSet,
    ) -> Self {
        Self {
            config,
            registry: Registry::new(),
            pending: DashMap::new(),
            worker_index: DashMap::new(),
            io,
            store,
            log_sink,
            log_store,
            account,
            // P1#7([29])：容量从 1024 提到 4096，降低 admin SSE 观察者在请求突发时丢事件（仅影响监控可见性，不影响中转）。
            events: broadcast::channel(4096).0,
            req_ring: RequestRing::new(),
            queue_wait: QueueWaitStat::new(),
            // /auth/*：每 IP 每 60s 最多 20 次（注册/登录/找回）。
            auth_ip_limiter: IpLimiter::new(20, 60),
            // /worker/claim：每 IP 每 60s 最多 10 次（领取罕见）。
            worker_claim_limiter: IpLimiter::new(10, 60),
            ip_rules,
            // 图片 URL fetch 全局并发上限（≤8 并发，配合 per-image 流式截断兜底内存）。
            image_fetch_sem: tokio::sync::Semaphore::new(8),
        }
    }

    /// 发布实时事件；无订阅者/满则忽略，绝不阻塞。
    pub fn publish(&self, ev: LiveEvent) {
        let _ = self.events.send(ev);
    }

    /// 登记一条在途请求（同时维护 worker_index 二级索引）。P1#6。
    /// charge 移入 pending：结算/退款由 gateway 的 worker 事件驱动，独立于 HTTP 连接（治断连即免费 #1）。
    pub fn add_pending(
        &self,
        req_id: String,
        worker_id: String,
        sid: Sid,
        tx: UnboundedSender<StreamEvent>,
        charge: Option<Charge>,
    ) {
        self.worker_index.entry(worker_id.clone()).or_default().insert(req_id.clone());
        self.pending.insert(
            req_id,
            PendingEntry {
                tx,
                worker_id,
                sid,
                created_at: now_secs(),
                charge: std::sync::Mutex::new(charge),
                streamed_chars: AtomicU64::new(0),
            },
        );
    }

    /// 注销一条在途请求（同时从 worker_index 清理，空集回收避免无限增长）。P1#6。
    /// 未取走的 Charge 随 entry drop → Charge::Drop 自动退款（失败退款不变量的最后兜底）。
    pub fn remove_pending(&self, req_id: &str) {
        if let Some((_, entry)) = self.pending.remove(req_id) {
            if let Some(mut set) = self.worker_index.get_mut(&entry.worker_id) {
                set.remove(req_id);
            }
            self.worker_index.remove_if(&entry.worker_id, |_, s| s.is_empty());
        }
    }

    /// 原子摘除一条在途请求并返回其 owned entry（含未取走的 Charge / streamed_chars / tx）。
    /// done 结算走这条路：先出 map、再从容 await 结算——杜绝结算期间被 fail_worker_inflight
    /// 找到同一 entry 并注入 Error（用户被扣却收到 502，#15）。
    pub fn take_pending(&self, req_id: &str) -> Option<PendingEntry> {
        if let Some((_, entry)) = self.pending.remove(req_id) {
            if let Some(mut set) = self.worker_index.get_mut(&entry.worker_id) {
                set.remove(req_id);
            }
            self.worker_index.remove_if(&entry.worker_id, |_, s| s.is_empty());
            Some(entry)
        } else {
            None
        }
    }

    /// 结算一个已从 pending 摘出的 Charge（owned）。定额计费：有内容 → commit（扣预扣额），无内容 → 退款。
    pub async fn settle_owned(&self, charge: Option<Charge>, has_content: bool) {
        if let Some(mut c) = charge {
            if has_content {
                c.commit();
            } else {
                c.refund().await;
            }
        }
    }

    /// 给某在途请求的 worker 发 cancel，让它中止运行、释放真实槽（治黑洞 + 僵尸生成）。
    pub fn cancel_req(&self, req_id: &str) {
        let sid = match self.pending.get(req_id) {
            Some(e) => e.sid,
            None => return,
        };
        if let Some(sock) = self.io.get_socket(sid) {
            let _ = sock.emit("cancel", &Cancel { req_id: req_id.to_string() });
        }
    }

    /// 把一个 worker 事件路由到对应请求的通道。返回 false 表示请求已不存在。
    /// 非空 Delta 会累加 streamed_chars（结算判「是否已交付内容」的服务端事实源）。
    pub fn route(&self, req_id: &str, ev: StreamEvent) -> bool {
        if let Some(e) = self.pending.get(req_id) {
            if let StreamEvent::Delta(d) = &ev {
                if !d.is_empty() {
                    e.streamed_chars
                        .fetch_add(d.len() as u64, std::sync::atomic::Ordering::Relaxed);
                }
            }
            e.tx.send(ev).is_ok()
        } else {
            false
        }
    }

    /// 清扫「既无终止事件、又未超时摘除」而滞留的在途请求（治僵尸 pending 泄漏预扣 #11）。
    /// 由 health 后台任务周期调用：取消 worker 侧运行 → 给消费方发 Error → 摘除（Charge Drop 退款）。
    pub fn reap_stale_pending(&self, cutoff: i64) -> usize {
        let stale: Vec<String> = self
            .pending
            .iter()
            .filter(|e| e.value().created_at < cutoff)
            .map(|e| e.key().clone())
            .collect();
        let mut n = 0;
        for req_id in stale {
            self.cancel_req(&req_id); // 先取消（cancel_req 要读 pending 拿 sid），再 take-first 摘除
            // take-first：若并发 done 已抢先 take_pending，本次返回 None → 不发 Error，消费方收到成功帧（#15 残留窗口）。
            if let Some(entry) = self.take_pending(&req_id) {
                let _ = entry.tx.send(StreamEvent::Error("request reaped: no terminal worker event".to_string()));
                n += 1; // 未取走的 Charge 随 entry Drop 自动退款
            }
        }
        n
    }

    /// worker 断连/过期时，立即让它名下所有在途请求失败（不再干等生成超时）。
    /// P1#6：先快照 worker_index 拿到 req_id 列表（释放锁），再逐个 send；
    /// 不再 iter 全表 pending，杜绝「迭代未达分片被并发 remove 而漏剔」的竞态。
    /// #1：摘除 entry → 未取走的 Charge 随 Drop 自动退款（worker 死亡属失败，退款正确）。
    pub fn fail_worker_inflight(&self, worker_id: &str, msg: &str) {
        let req_ids: Vec<String> = self
            .worker_index
            .get(worker_id)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default();
        for req_id in req_ids {
            // take-first：先原子摘除再经 owned tx 发 Error——若并发 done 已抢先 take_pending，本次返回 None，
            // 消费方只收到 done 成功帧（消除「已扣费却收 502」的 #15 残留窗口）。未取走的 Charge 随 Drop 退款。
            if let Some(entry) = self.take_pending(&req_id) {
                let _ = entry.tx.send(StreamEvent::Error(msg.to_string()));
            }
        }
    }
}
