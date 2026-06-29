//! GET /admin/metrics — KPI 聚合 + 吞吐序列（控制台 Overview 用）。

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::account::AdminGuard;
use crate::state::AppState;

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub async fn list_metrics(_admin: AdminGuard, State(state): State<Arc<AppState>>) -> Json<Value> {
    let now = now_secs();
    let live = state.registry.snapshot(now);
    let online = live.len();
    let live_ids: HashSet<String> = live.iter().map(|w| w.worker_id.clone()).collect();
    let mut total = online;
    for row in state.store.list_workers().await {
        if !live_ids.contains(&row.worker_id) {
            total += 1;
        }
    }

    let series = state.req_ring.series(now, 60);
    let now_min = now / 60;
    let points: Vec<Value> = series
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let minute = now_min - (series.len() as i64 - 1 - i as i64);
            json!({ "t": minute * 60, "count": c })
        })
        .collect();

    // 存储后端类型（控制台据此对内存模式给出「重启丢数据」警示）。
    let pg = state.config.database_url.as_deref().map(|u| !u.is_empty()).unwrap_or(false);

    Json(json!({
        "workers": { "online": online, "total": total },
        "active_sessions": state.pending.len(),
        "requests_24h": state.req_ring.total(now, 1440),
        "avg_queue_wait_ms": state.queue_wait.avg_ms(),
        "throughput": { "bucket_secs": 60, "points": points },
        "storage_backend": if pg { "postgres" } else { "memory" },
    }))
}
