//! 后台健康任务：定时剔除心跳过期的 worker（让其在途请求快速失败），
//! 并把 last_seen 落库。在 main 启动钩子内 spawn。

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::state::AppState;

const SCAN_INTERVAL: Duration = Duration::from_secs(30);

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(SCAN_INTERVAL);
        loop {
            tick.tick().await;
            let now = now_secs();
            let threshold = now - state.config.heartbeat_stale_secs;
            for worker_id in state.registry.stale_worker_ids(threshold) {
                tracing::warn!(category = "connection", %worker_id, "worker 心跳过期，剔除");
                // 先抓 sid（剔除后注册表就查不到了）。
                let sid = state.registry.sid_of(&worker_id);
                // 先清理：remove_by_id 一并清 by_sid，使随后 disconnect 触发的（socketioxide 同步内联执行的）
                // on_disconnect 里 remove_by_sid 返回 None → 其 fail_worker_inflight/publish 被跳过，避免重复。
                state.fail_worker_inflight(&worker_id, "worker heartbeat timeout");
                state.registry.remove_by_id(&worker_id);
                state.publish(crate::metrics::LiveEvent::WorkerStatus {
                    worker_id: worker_id.clone(),
                    status: "offline".to_string(),
                });
                let _ = state.store.touch_worker(&worker_id, now).await;
                // 最后主动断开僵死 socket（触发客户端重连 → 重新注册），消除「注册表已删但 socket 仍连」的幽灵。
                if let Some(sid) = sid {
                    if let Some(s) = state.io.get_socket(sid) {
                        let _ = s.disconnect();
                    }
                }
            }
        }
    });
}
