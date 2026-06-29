//! 内存 worker 注册表（M3）：每 worker 一个 Semaphore 控并发槽位 + 状态模型
//! （available / cooling / disabled）；全局 Notify 在「槽位释放 / 新 worker 上线 /
//! 状态变化」时唤醒排队中的请求。在线状态以本表为准（单实例）。

use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;

use dashmap::DashMap;
use socketioxide::socket::Sid;
use tokio::sync::futures::Notified;
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};

pub struct WorkerHandle {
    pub worker_id: String,
    pub sid: Sid,
    pub provider: String,
    /// B2 分组标签（"" = 默认池）。调度按 (provider, group) 过滤。
    pub group: String,
    pub account_label: String,
    pub capacity: usize,
    pub sem: Arc<Semaphore>,
    /// false = 掉登录 / 被风控 / 不可用
    pub available: Arc<AtomicBool>,
    /// 限流冷却到期 unix 秒；<=now 表示不在冷却
    pub cooling_until: Arc<AtomicI64>,
    /// admin 排空开关
    pub disabled: Arc<AtomicBool>,
    /// 最近心跳时间（unix 秒），用于过期剔除
    pub last_seen: Arc<AtomicI64>,
    /// 累计服务次数（done）
    pub total_served: Arc<AtomicU64>,
    /// B4 熔断滑窗：(窗口起点 unix 秒, 窗内错误计数)。一次成功 done 清零。
    pub breaker: Arc<std::sync::Mutex<(i64, u32)>>,
    /// B4 熔断打开到期 unix 秒；>now 表示已熔断（停止派发，到期自愈）
    pub breaker_until: Arc<AtomicI64>,
}

impl WorkerHandle {
    pub fn is_dispatchable(&self, now: i64) -> bool {
        self.available.load(Ordering::SeqCst)
            && !self.disabled.load(Ordering::SeqCst)
            && self.breaker_until.load(Ordering::SeqCst) <= now // B4：熔断中不派发
            && self.cooling_until.load(Ordering::SeqCst) <= now
            && self.sem.available_permits() > 0
    }

    /// idle / busy / cooling / logged_out / disabled / tripped
    pub fn status(&self, now: i64) -> &'static str {
        if self.disabled.load(Ordering::SeqCst) {
            "disabled"
        } else if self.breaker_until.load(Ordering::SeqCst) > now {
            "tripped" // B4：熔断打开
        } else if !self.available.load(Ordering::SeqCst) {
            "logged_out"
        } else if self.cooling_until.load(Ordering::SeqCst) > now {
            "cooling"
        } else if self.sem.available_permits() == 0 {
            "busy"
        } else {
            "idle"
        }
    }
}

/// 占用一个槽位的租约。drop 时归还 permit 并唤醒等待者。
pub struct Lease {
    pub worker_id: String,
    pub sid: Sid,
    permit: Option<OwnedSemaphorePermit>,
    notify: Arc<Notify>,
}

impl Drop for Lease {
    fn drop(&mut self) {
        drop(self.permit.take()); // 先归还槽位
        self.notify.notify_waiters(); // 再唤醒排队请求
    }
}

/// 一台 worker 的对外快照（admin 用）。
pub struct WorkerSnapshot {
    pub worker_id: String,
    pub provider: String,
    pub group: String,
    pub account_label: String,
    pub status: &'static str,
    pub in_flight: usize,
    pub free: usize,
    pub capacity: usize,
    pub cooldown_until: i64,
    pub last_seen: i64,
    pub total_served: u64,
}

#[derive(Default)]
pub struct Registry {
    workers: DashMap<String, Arc<WorkerHandle>>,
    by_sid: DashMap<Sid, String>,
    notify: Arc<Notify>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn register(
        &self,
        worker_id: String,
        sid: Sid,
        provider: String,
        group: String,
        account_label: String,
        capacity: usize,
        now: i64,
    ) {
        let cap = capacity.max(1);
        let handle = Arc::new(WorkerHandle {
            worker_id: worker_id.clone(),
            sid,
            provider,
            group,
            account_label,
            capacity: cap,
            sem: Arc::new(Semaphore::new(cap)),
            available: Arc::new(AtomicBool::new(true)),
            cooling_until: Arc::new(AtomicI64::new(0)),
            disabled: Arc::new(AtomicBool::new(false)),
            last_seen: Arc::new(AtomicI64::new(now)),
            total_served: Arc::new(AtomicU64::new(0)),
            breaker: Arc::new(std::sync::Mutex::new((0, 0))),
            breaker_until: Arc::new(AtomicI64::new(0)),
        });
        self.by_sid.insert(sid, worker_id.clone());
        self.workers.insert(worker_id, handle);
        self.notify.notify_waiters(); // 新 worker 上线 → 唤醒排队请求
    }

    pub fn remove_by_sid(&self, sid: &Sid) -> Option<String> {
        if let Some((_, worker_id)) = self.by_sid.remove(sid) {
            self.workers.remove(&worker_id);
            self.notify.notify_waiters();
            Some(worker_id)
        } else {
            None
        }
    }

    pub fn remove_by_id(&self, worker_id: &str) {
        if let Some((_, h)) = self.workers.remove(worker_id) {
            self.by_sid.remove(&h.sid);
            self.notify.notify_waiters();
        }
    }

    /// 用于排队等待：先 arm 再 try 再 await，避免丢唤醒。
    pub fn notified(&self) -> Notified<'_> {
        self.notify.notified()
    }

    pub fn worker_online(&self, worker_id: &str) -> bool {
        self.workers.contains_key(worker_id)
    }

    pub fn has_provider(&self, provider: &str) -> bool {
        self.workers.iter().any(|e| e.value().provider == provider)
    }

    pub fn sid_of(&self, worker_id: &str) -> Option<Sid> {
        self.workers.get(worker_id).map(|h| h.sid)
    }

    /// 当前状态字符串（用于实时事件广播）。
    pub fn status_of(&self, worker_id: &str, now: i64) -> Option<&'static str> {
        self.workers.get(worker_id).map(|h| h.status(now))
    }

    // ---- 状态变更（gateway / admin 调用）----

    pub fn set_status(&self, worker_id: &str, available: bool, cooling_until: i64) {
        if let Some(h) = self.workers.get(worker_id) {
            h.available.store(available, Ordering::SeqCst);
            h.cooling_until.store(cooling_until, Ordering::SeqCst);
        }
        self.notify.notify_waiters();
    }

    pub fn set_disabled(&self, worker_id: &str, disabled: bool) {
        if let Some(h) = self.workers.get(worker_id) {
            h.disabled.store(disabled, Ordering::SeqCst);
        }
    }

    pub fn touch(&self, worker_id: &str, now: i64) {
        if let Some(h) = self.workers.get(worker_id) {
            h.last_seen.store(now, Ordering::SeqCst);
        }
    }

    pub fn mark_served(&self, worker_id: &str) {
        if let Some(h) = self.workers.get(worker_id) {
            h.total_served.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// B4：记一次 worker 级错误；窗内累计达阈值 → 打开熔断（长冷却）。返回是否本次触发。
    pub fn record_worker_error(
        &self,
        worker_id: &str,
        now: i64,
        window: i64,
        threshold: u32,
        open_secs: i64,
    ) -> bool {
        let Some(h) = self.workers.get(worker_id) else {
            return false;
        };
        let mut g = h.breaker.lock().unwrap();
        if now - g.0 > window {
            g.0 = now;
            g.1 = 0; // 滑窗过期重置
        }
        g.1 += 1;
        let tripped = g.1 >= threshold;
        if tripped {
            h.breaker_until.store(now + open_secs, Ordering::SeqCst);
            g.0 = now;
            g.1 = 0; // 打开后清零，避免反复触发
        }
        drop(g);
        if tripped {
            self.notify.notify_waiters();
        }
        tripped
    }

    /// B4：worker 正常完成一次请求 → 清错误窗（成功即康复，杜绝慢累积误触）。
    pub fn record_worker_success(&self, worker_id: &str) {
        if let Some(h) = self.workers.get(worker_id) {
            let mut g = h.breaker.lock().unwrap();
            g.0 = 0;
            g.1 = 0;
        }
    }

    /// B4：清除熔断（admin enable 时调）。
    pub fn reset_breaker(&self, worker_id: &str) {
        if let Some(h) = self.workers.get(worker_id) {
            h.breaker_until.store(0, Ordering::SeqCst);
            let mut g = h.breaker.lock().unwrap();
            g.0 = 0;
            g.1 = 0;
        }
    }

    /// 心跳早于 threshold 的 worker_id（用于过期剔除）。
    pub fn stale_worker_ids(&self, threshold: i64) -> Vec<String> {
        self.workers
            .iter()
            .filter(|e| e.value().last_seen.load(Ordering::SeqCst) < threshold)
            .map(|e| e.key().clone())
            .collect()
    }

    fn lease_from(&self, h: &WorkerHandle, permit: OwnedSemaphorePermit) -> Lease {
        Lease {
            worker_id: h.worker_id.clone(),
            sid: h.sid,
            permit: Some(permit),
            notify: self.notify.clone(),
        }
    }

    /// 在该 (provider, group) 下挑一个可派发的 worker 并占用（负载均衡：选剩余槽位最多）。B2。
    pub fn try_acquire_any(&self, provider: &str, group: &str, now: i64) -> Option<Lease> {
        let mut best: Option<Arc<WorkerHandle>> = None;
        for e in self.workers.iter() {
            let h = e.value();
            if h.provider != provider || h.group != group || !h.is_dispatchable(now) {
                continue;
            }
            match &best {
                Some(b) if b.sem.available_permits() >= h.sem.available_permits() => {}
                _ => best = Some(h.clone()),
            }
        }
        let h = best?;
        let permit = h.sem.clone().try_acquire_owned().ok()?;
        Some(self.lease_from(&h, permit))
    }

    /// 定向占用某个 worker（会话亲和续接）。被禁用/掉登录/冷却中则返回 None。
    pub fn try_acquire_specific(&self, worker_id: &str, now: i64) -> Option<Lease> {
        let h = self.workers.get(worker_id)?.clone();
        if !h.is_dispatchable(now) {
            return None;
        }
        let permit = h.sem.clone().try_acquire_owned().ok()?;
        Some(self.lease_from(&h, permit))
    }

    /// 该 (provider, group) 下所有在线 worker 中最早的冷却到期时间（用于 503 提示）。B2。
    pub fn earliest_cooldown(&self, provider: &str, group: &str, now: i64) -> Option<i64> {
        self.workers
            .iter()
            .filter(|e| {
                let h = e.value();
                h.provider == provider
                    && h.group == group
                    && !h.disabled.load(Ordering::SeqCst)
                    && h.available.load(Ordering::SeqCst)
                    && h.cooling_until.load(Ordering::SeqCst) > now
            })
            .map(|e| e.value().cooling_until.load(Ordering::SeqCst))
            .min()
    }

    /// admin 快照。
    pub fn snapshot(&self, now: i64) -> Vec<WorkerSnapshot> {
        self.workers
            .iter()
            .map(|e| {
                let h = e.value();
                let free = h.sem.available_permits();
                WorkerSnapshot {
                    worker_id: h.worker_id.clone(),
                    provider: h.provider.clone(),
                    group: h.group.clone(),
                    account_label: h.account_label.clone(),
                    status: h.status(now),
                    in_flight: h.capacity.saturating_sub(free),
                    free,
                    capacity: h.capacity,
                    cooldown_until: h.cooling_until.load(Ordering::SeqCst),
                    last_seen: h.last_seen.load(Ordering::SeqCst),
                    total_served: h.total_served.load(Ordering::SeqCst),
                }
            })
            .collect()
    }
}
