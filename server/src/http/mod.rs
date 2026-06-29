pub mod admin;
pub mod auth;
pub mod consumer;
pub mod dashboard;
pub mod events;
pub mod ipfilter;
pub mod metrics;
pub mod payment;
pub mod prometheus;
pub mod ratelimit_mw;
pub mod worker;

use std::sync::Arc;

use axum::routing::{delete, get, post};
use axum::Router;

use crate::state::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    // /auth/* 走 per-IP 限流（防注册/登录/找回爆破）。
    let auth_routes = Router::new()
        .route("/auth/register", post(dashboard::register))
        .route("/auth/login", post(dashboard::login))
        .route("/auth/login/2fa", post(dashboard::login_2fa))
        .route("/auth/forgot", post(dashboard::forgot))
        .route("/auth/reset", post(dashboard::reset))
        .route("/auth/verify-email", get(dashboard::verify_email))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            ratelimit_mw::auth_rate_limit,
        ));

    // /worker/claim 走独立 per-IP 限流（扩展集中领取身份）。
    let worker_routes = Router::new()
        .route("/worker/claim", post(worker::claim_worker))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            ratelimit_mw::worker_claim_rate_limit,
        ));

    Router::new()
        // 消费 API（api_key）
        .route("/v1/chat/completions", post(consumer::chat_completions))
        .route("/v1/messages", post(consumer::messages)) // A1：Anthropic Messages API
        .route("/v1/models", get(consumer::list_models))
        .route("/v1/pricing", get(consumer::pricing))
        // 用户 dashboard（JWT）
        .route("/v1/me", get(dashboard::me))
        .route("/v1/logout-all", post(dashboard::logout_all))
        .route("/v1/2fa/setup", post(dashboard::totp_setup))
        .route("/v1/2fa/enable", post(dashboard::totp_enable))
        .route("/v1/2fa/disable", post(dashboard::totp_disable))
        .route("/v1/balance", get(dashboard::balance))
        .route("/v1/transactions", get(dashboard::transactions))
        .route("/v1/redeem", post(dashboard::redeem))
        .route("/v1/keys", get(dashboard::list_keys).post(dashboard::create_key))
        .route("/v1/keys/{id}", delete(dashboard::delete_key))
        .route("/v1/keys/{id}/disable", post(dashboard::disable_key))
        .route("/v1/keys/{id}/enable", post(dashboard::enable_key))
        // 在线支付
        .route("/v1/payment/create", post(payment::create_order))
        .route("/v1/payment/orders/{id}", get(payment::order_status))
        .route("/payment/notify", get(payment::notify_get).post(payment::notify_post))
        .route("/payment/return", get(payment::return_page))
        // admin：worker / session / logs（AdminGuard）
        .route("/admin/workers", get(admin::list_workers))
        .route("/admin/workers/{id}/disable", post(admin::disable_worker))
        .route("/admin/workers/{id}/enable", post(admin::enable_worker))
        .route("/admin/sessions", get(admin::list_sessions))
        .route("/admin/sessions/{id}", get(admin::get_session))
        .route("/admin/logs", get(admin::list_logs))
        .route("/admin/metrics", get(metrics::list_metrics))
        .route("/admin/stats/usage", get(admin::usage_stats))
        .route("/admin/audit", get(admin::list_audit))
        .route("/admin/events", get(events::admin_events))
        // admin：用户管理
        .route("/admin/users", get(admin::list_users).post(admin::create_user))
        .route("/admin/users/{id}", get(admin::get_user))
        .route("/admin/users/{id}/ban", post(admin::ban_user))
        .route("/admin/users/{id}/unban", post(admin::unban_user))
        .route("/admin/users/{id}/recharge", post(admin::recharge_user))
        .route("/admin/users/{id}/rate-limit", post(admin::set_rate_limit))
        .route("/admin/users/{id}/2fa/reset", post(admin::reset_2fa))
        .route("/admin/users/{id}/keys", get(admin::list_user_keys).post(admin::create_user_key))
        .route("/admin/api-keys/{id}/disable", post(admin::admin_disable_key))
        .route("/admin/api-keys/{id}/group", post(admin::set_key_group))
        .route("/admin/api-keys/{id}/enable", post(admin::admin_enable_key))
        // admin：兑换码
        .route("/admin/redeem-codes", get(admin::list_redeem_codes).post(admin::create_redeem_codes))
        // 健康探针 + Prometheus 指标（独立 metrics_token，非 AdminGuard）
        .route("/healthz", get(admin::healthz))
        .route("/health", get(admin::healthz)) // /healthz 别名（不少探针默认探 /health）
        .route("/livez", get(admin::livez))
        .route("/readyz", get(admin::readyz))
        .route("/metrics", get(prometheus::prometheus))
        .merge(auth_routes)
        .merge(worker_routes)
        .with_state(state)
}
