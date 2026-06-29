//! 调度：在容量 + 状态约束下挑 worker，支持排队等待（queue_wait）。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::time::{timeout, Instant};

use super::registry::{Lease, Registry};

#[derive(Debug)]
pub enum SchedError {
    /// 续接会话的目标 worker 不在线/不可派发。
    WorkerOffline,
    /// 等待超时仍无可派发槽位。
    Timeout,
}

const TICK: Duration = Duration::from_millis(1000);

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 在 (provider, group) 池里取一个可派发槽位，最长等待 wait。B2。
pub async fn acquire_any(reg: &Registry, provider: &str, group: &str, wait: Duration) -> Result<Lease, SchedError> {
    let deadline = Instant::now() + wait;
    loop {
        let notified = reg.notified(); // 先 arm
        if let Some(l) = reg.try_acquire_any(provider, group, now_secs()) {
            return Ok(l);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(SchedError::Timeout);
        }
        let _ = timeout(remaining.min(TICK), notified).await;
    }
}

/// 定向取某个 worker 的槽位（会话亲和续接）。worker 不在线立即返回 WorkerOffline；
/// 在线但暂不可派发（冷却/忙）则排队等待。
pub async fn acquire_specific(
    reg: &Registry,
    worker_id: &str,
    wait: Duration,
) -> Result<Lease, SchedError> {
    let deadline = Instant::now() + wait;
    loop {
        let notified = reg.notified();
        if !reg.worker_online(worker_id) {
            return Err(SchedError::WorkerOffline);
        }
        if let Some(l) = reg.try_acquire_specific(worker_id, now_secs()) {
            return Ok(l);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(SchedError::Timeout);
        }
        let _ = timeout(remaining.min(TICK), notified).await;
    }
}
