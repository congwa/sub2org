//! per-IP 限流中间件（防注册/登录/找回密码爆破）。固定窗口计数，轻量够用；
//! 高吞吐的 /v1/* 由 per-user rate_limit + 余额 + nginx 层兜底，不在此限。

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use dashmap::DashMap;
use serde_json::json;

use crate::state::AppState;

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 固定窗口 per-key 计数器。
pub struct IpLimiter {
    windows: DashMap<String, (i64, u32)>, // ip -> (window_start, count)
    limit: u32,
    window_secs: i64,
}

impl IpLimiter {
    pub fn new(limit: u32, window_secs: i64) -> Self {
        Self {
            windows: DashMap::new(),
            limit,
            window_secs,
        }
    }

    /// 计数并判断是否放行。true=放行。
    pub fn check(&self, ip: &str) -> bool {
        let now = now();
        let mut e = self.windows.entry(ip.to_string()).or_insert((now, 0));
        if now - e.0 >= self.window_secs {
            e.0 = now;
            e.1 = 0;
        }
        if e.1 >= self.limit {
            return false;
        }
        e.1 += 1;
        true
    }
}

/// 取客户端 IP（P1#5）。
/// `trust_proxy=true`（默认，在 nginx 后）：优先反代写的 X-Real-IP / X-Forwarded-For 第一跳。
/// `trust_proxy=false`（直连暴露）：忽略这些可伪造头，只认 socket peer IP，杜绝伪造头绕过限流/IP 规则。
/// 两种情形都以 peer IP 兜底（无 ConnectInfo 时为 "direct"）。
pub fn client_ip(req: &Request, trust_proxy: bool) -> String {
    if trust_proxy {
        let h = req.headers();
        if let Some(rip) = h.get("x-real-ip").and_then(|v| v.to_str().ok()) {
            if !rip.trim().is_empty() {
                return rip.trim().to_string();
            }
        }
        if let Some(xff) = h.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            if let Some(first) = xff.split(',').next() {
                let ip = first.trim();
                if !ip.is_empty() {
                    return ip.to_string();
                }
            }
        }
    }
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip().to_string())
        .unwrap_or_else(|| "direct".to_string())
}

/// 挂在 /auth/* 上的限流中间件。
pub async fn auth_rate_limit(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let ip = client_ip(&req, state.config.trust_proxy_headers);
    if !state.auth_ip_limiter.check(&ip) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": {"message": "请求过于频繁，请稍后再试", "type": "rate_limited"}})),
        )
            .into_response();
    }
    next.run(req).await
}

/// 挂在 /worker/claim 上的限流中间件（领取身份罕见，单独一桶，与 auth 限流互不影响）。
pub async fn worker_claim_rate_limit(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let ip = client_ip(&req, state.config.trust_proxy_headers);
    if !state.worker_claim_limiter.check(&ip) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": {"message": "领取过于频繁，请稍后再试", "type": "rate_limited"}})),
        )
            .into_response();
    }
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ip_limiter_blocks_after_limit() {
        let l = IpLimiter::new(3, 60);
        assert!(l.check("1.2.3.4"));
        assert!(l.check("1.2.3.4"));
        assert!(l.check("1.2.3.4"));
        assert!(!l.check("1.2.3.4")); // 第 4 次被拦
        assert!(l.check("5.6.7.8")); // 另一个 IP 不受影响
    }
}
