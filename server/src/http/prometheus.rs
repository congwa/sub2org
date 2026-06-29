//! GET /metrics — Prometheus 文本曝露格式（text/plain; version=0.0.4）。
//! 手写格式，零新依赖；token 用独立 config.metrics_token（Bearer 或 ?token=）。
//! 空 token = 开放（需用 nginx allowlist 锁住，见 DEPLOY.md 第 8 节）。

use std::collections::HashSet;
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::state::AppState;

const CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[derive(Deserialize)]
pub struct MetricsAuth {
    pub token: Option<String>,
}

fn bearer_token(headers: &header::HeaderMap) -> Option<String> {
    let v = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let t = v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer "))?;
    Some(t.trim().to_string())
}

/// GET /metrics
pub async fn prometheus(
    State(state): State<Arc<AppState>>,
    headers: header::HeaderMap,
    Query(q): Query<MetricsAuth>,
) -> Response {
    let cfg = &state.config;
    if !cfg.metrics_token.is_empty() {
        let supplied = bearer_token(&headers).or(q.token);
        if supplied.as_deref() != Some(cfg.metrics_token.as_str()) {
            return (StatusCode::UNAUTHORIZED, "metrics auth required\n").into_response();
        }
    }

    let now = now_secs();

    // ---- 采集（与 http/metrics.rs::list_metrics 同源）----
    let live = state.registry.snapshot(now);
    let online = live.len();
    let live_ids: HashSet<String> = live.iter().map(|w| w.worker_id.clone()).collect();
    // served_total 必须单调（Prometheus counter）：用 DB 持久累计（list_workers 覆盖所有
    // 已知 worker，不随上下线波动），而非在线快照（重连会清零、断连会丢失）。
    let mut total = online;
    let mut served_total: u64 = 0;
    for row in state.store.list_workers().await {
        served_total += row.total_served.max(0) as u64;
        if !live_ids.contains(&row.worker_id) {
            total += 1;
        }
    }

    let mut idle = 0usize;
    let mut busy = 0usize;
    let mut cooling = 0usize;
    let mut logged_out = 0usize;
    let mut disabled = 0usize;
    let mut in_flight = 0usize;
    let mut capacity = 0usize;
    for w in &live {
        match w.status {
            "idle" => idle += 1,
            "busy" => busy += 1,
            "cooling" => cooling += 1,
            "logged_out" => logged_out += 1,
            "disabled" => disabled += 1,
            _ => {}
        }
        in_flight += w.in_flight;
        capacity += w.capacity;
    }

    let pending = state.pending.len();
    let req_1m = state.req_ring.total(now, 1);
    let req_24h = state.req_ring.total(now, 1440);
    let queue_wait_ms = state.queue_wait.avg_ms();
    let logs_dropped = state.log_sink.dropped();

    // ---- 渲染 ----
    let mut o = String::with_capacity(2048);
    g(&mut o, "sub2org_workers_online", "Workers currently connected (in-memory registry).", online as f64);
    g(&mut o, "sub2org_workers_total", "Workers known (online + DB-known offline).", total as f64);

    o.push_str("# HELP sub2org_workers_by_status Online workers grouped by status.\n");
    o.push_str("# TYPE sub2org_workers_by_status gauge\n");
    sline(&mut o, "sub2org_workers_by_status", "idle", idle as f64);
    sline(&mut o, "sub2org_workers_by_status", "busy", busy as f64);
    sline(&mut o, "sub2org_workers_by_status", "cooling", cooling as f64);
    sline(&mut o, "sub2org_workers_by_status", "logged_out", logged_out as f64);
    sline(&mut o, "sub2org_workers_by_status", "disabled", disabled as f64);

    g(&mut o, "sub2org_worker_slots_in_flight", "Occupied concurrency slots across online workers.", in_flight as f64);
    g(&mut o, "sub2org_worker_slots_capacity", "Total concurrency slot capacity across online workers.", capacity as f64);
    g(&mut o, "sub2org_requests_inflight", "Pending requests being relayed right now (AppState.pending).", pending as f64);

    // 滑动窗瞬时值 → gauge（非单调，不能当 counter 给 rate() 用）。
    g(&mut o, "sub2org_requests_1m", "Dispatched requests in the last minute (sliding window).", req_1m as f64);
    g(&mut o, "sub2org_requests_24h", "Dispatched requests in the last 24h (sliding window).", req_24h as f64);
    c(&mut o, "sub2org_worker_served_total", "Cumulative served (done) across all known workers (DB-persisted).", served_total as f64);
    c(&mut o, "sub2org_logs_dropped_total", "Log lines dropped due to channel overflow.", logs_dropped as f64);

    if let Some(ms) = queue_wait_ms {
        g(&mut o, "sub2org_queue_wait_ms", "EWMA of queue wait before dispatch (ms).", ms as f64);
    }

    ([(header::CONTENT_TYPE, CONTENT_TYPE)], o).into_response()
}

// ---- 文本格式小工具 ----
fn g(o: &mut String, name: &str, help: &str, v: f64) {
    let _ = writeln!(o, "# HELP {name} {help}\n# TYPE {name} gauge\n{name} {}", fmt_num(v));
}
fn c(o: &mut String, name: &str, help: &str, v: f64) {
    let _ = writeln!(o, "# HELP {name} {help}\n# TYPE {name} counter\n{name} {}", fmt_num(v));
}
fn sline(o: &mut String, name: &str, status: &str, v: f64) {
    let _ = writeln!(o, "{name}{{status=\"{status}\"}} {}", fmt_num(v));
}
/// 整数值不带小数，避免科学计数法。
fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}
