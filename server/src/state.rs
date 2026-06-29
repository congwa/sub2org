use std::collections::HashSet;
use std::sync::Arc;

use dashmap::DashMap;
use socketioxide::SocketIo;
use tokio::sync::broadcast;
use tokio::sync::mpsc::UnboundedSender;

use crate::account::AccountService;
use crate::config::Config;
use crate::http::ipfilter::IpRuleSet;
use crate::http::ratelimit_mw::IpLimiter;
use crate::logging::{LogSink, LogStore};
use crate::metrics::{LiveEvent, QueueWaitStat, RequestRing};
use crate::pool::registry::Registry;
use crate::store::Store;
use crate::stream::StreamEvent;

/// 一个在途请求的回传端 + 它派给了哪个 worker（用于断连快速失败）。
pub struct PendingEntry {
    pub tx: UnboundedSender<StreamEvent>,
    pub worker_id: String,
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
    pub fn add_pending(&self, req_id: String, worker_id: String, tx: UnboundedSender<StreamEvent>) {
        self.worker_index.entry(worker_id.clone()).or_default().insert(req_id.clone());
        self.pending.insert(req_id, PendingEntry { tx, worker_id });
    }

    /// 注销一条在途请求（同时从 worker_index 清理，空集回收避免无限增长）。P1#6。
    pub fn remove_pending(&self, req_id: &str) {
        if let Some((_, entry)) = self.pending.remove(req_id) {
            if let Some(mut set) = self.worker_index.get_mut(&entry.worker_id) {
                set.remove(req_id);
            }
            self.worker_index.remove_if(&entry.worker_id, |_, s| s.is_empty());
        }
    }

    /// 把一个 worker 事件路由到对应请求的通道。返回 false 表示请求已不存在。
    pub fn route(&self, req_id: &str, ev: StreamEvent) -> bool {
        if let Some(e) = self.pending.get(req_id) {
            e.tx.send(ev).is_ok()
        } else {
            false
        }
    }

    /// worker 断连/过期时，立即让它名下所有在途请求失败（不再干等生成超时）。
    /// P1#6：先快照 worker_index 拿到 req_id 列表（释放锁），再逐个 send；
    /// 不再 iter 全表 pending，杜绝「迭代未达分片被并发 remove 而漏剔」的竞态。
    pub fn fail_worker_inflight(&self, worker_id: &str, msg: &str) {
        let req_ids: Vec<String> = self
            .worker_index
            .get(worker_id)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default();
        for req_id in req_ids {
            if let Some(e) = self.pending.get(&req_id) {
                let _ = e.tx.send(StreamEvent::Error(msg.to_string()));
            }
        }
    }
}
