//! 在线支付（易支付/epay）HTTP 端点：下单、异步回调（验签+幂等入账）、同步返回、订单查询。

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use axum::extract::{Form, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::account::CurrentUser;
use crate::payment::{epay, money_str, money_to_fen};
use crate::state::AppState;

fn jerr(code: StatusCode, msg: &str) -> Response {
    (code, Json(json!({"error": {"message": msg, "type": "payment_error"}}))).into_response()
}

#[derive(Deserialize)]
pub struct CreateBody {
    pub amount: i64,
    #[serde(default)]
    pub channel: String,
}

/// POST /v1/payment/create — 建订单并返回跳转支付 URL。
pub async fn create_order(
    State(state): State<Arc<AppState>>,
    CurrentUser(u): CurrentUser,
    Json(b): Json<CreateBody>,
) -> Response {
    let cfg = match state.config.payment.as_ref() {
        Some(c) if !c.epay_pid.is_empty() && !c.epay_key.is_empty() => c,
        _ => return jerr(StatusCode::SERVICE_UNAVAILABLE, "在线支付未配置"),
    };
    let channel = if b.channel.is_empty() {
        cfg.enabled_channels.first().cloned().unwrap_or_else(|| "alipay".to_string())
    } else {
        b.channel.clone()
    };
    if !cfg.enabled_channels.iter().any(|c| c == &channel) {
        return jerr(StatusCode::BAD_REQUEST, "不支持的支付渠道");
    }
    if b.amount < cfg.min_amount || b.amount > cfg.max_amount {
        return jerr(
            StatusCode::BAD_REQUEST,
            &format!("金额需在 {}~{} 积分之间", cfg.min_amount, cfg.max_amount),
        );
    }

    let order_id = format!("s2o-{}", Uuid::new_v4().simple());
    if let Err(e) = state
        .account
        .store
        .create_payment_order(&order_id, &u.id, b.amount, "epay")
        .await
    {
        return jerr(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string());
    }

    let base = state.config.public_base_url.trim_end_matches('/');
    let notify_url = format!("{base}/payment/notify");
    let return_url = format!("{base}/payment/return");
    let money = money_str(b.amount, cfg.credits_per_cny);
    let pay_url = epay::build_submit_url(&epay::PayRequest {
        gateway: &cfg.epay_url,
        pid: &cfg.epay_pid,
        key: &cfg.epay_key,
        out_trade_no: &order_id,
        channel: &channel,
        name: &format!("sub2org credits x{}", b.amount),
        money: &money,
        notify_url: &notify_url,
        return_url: &return_url,
    });

    tracing::info!(category = "payment", order_id = %order_id, user_id = %u.id, amount = b.amount, "payment order created");
    Json(json!({
        "order_id": order_id,
        "pay_url": pay_url,
        "amount": b.amount,
        "money": money,
        "channel": channel,
    }))
    .into_response()
}

/// 处理 epay 回调（GET query 或 POST form 同逻辑）。返回 epay 约定的纯文本。
async fn process_notify(state: &Arc<AppState>, map: HashMap<String, String>) -> &'static str {
    let cfg = match state.config.payment.as_ref() {
        Some(c) if !c.epay_key.is_empty() => c,
        _ => return "fail",
    };
    let params: BTreeMap<String, String> = map.into_iter().collect();
    if !epay::verify(&params, &cfg.epay_key) {
        tracing::warn!(category = "payment", "epay notify 验签失败");
        return "fail";
    }
    if params.get("trade_status").map(|s| s.as_str()) != Some("TRADE_SUCCESS") {
        return "fail";
    }
    let order_id = match params.get("out_trade_no") {
        Some(o) => o.clone(),
        None => return "fail",
    };
    let order = match state.account.store.get_payment_order(&order_id).await {
        Some(o) => o,
        None => return "fail",
    };
    // 校验金额一致（防篡改/防少付）。#8：数值比较而非严格字符串——网关把 "10.00" 回成 "10.0"
    // 不再误判失败（真金已付却入不了账）。注意：expected 仍按**当前** credits_per_cny 重算，
    // 若下单后管理员改了汇率，历史 pending 订单会对不上（reprice 缺陷）——根治需在下单时持久化原始金额，
    // 见修复说明的后续项。
    let expected = money_str(order.amount, cfg.credits_per_cny);
    let paid_ok = match (money_to_fen(params.get("money").map(|s| s.as_str()).unwrap_or("")), money_to_fen(&expected)) {
        (Some(paid), Some(exp)) => paid == exp,
        _ => false,
    };
    if !paid_ok {
        tracing::warn!(category = "payment", order_id = %order_id, paid = ?params.get("money"), expected = %expected, "epay notify 金额不匹配");
        return "fail";
    }
    match state.account.store.mark_payment_paid(&order_id).await {
        Ok(Some(bal)) => {
            tracing::info!(category = "payment", order_id = %order_id, user_id = %order.user_id, balance = bal, "payment paid + credited");
        }
        Ok(None) => {
            // 重复回调：已入账，幂等放行。
            tracing::info!(category = "payment", order_id = %order_id, "duplicate notify ignored");
        }
        Err(e) => {
            tracing::error!(category = "payment", order_id = %order_id, error = %e, "mark_payment_paid 失败");
            return "fail";
        }
    }
    "success"
}

/// GET /payment/notify
pub async fn notify_get(State(state): State<Arc<AppState>>, Query(q): Query<HashMap<String, String>>) -> Response {
    process_notify(&state, q).await.into_response()
}

/// POST /payment/notify
pub async fn notify_post(State(state): State<Arc<AppState>>, Form(f): Form<HashMap<String, String>>) -> Response {
    process_notify(&state, f).await.into_response()
}

/// GET /payment/return — 用户支付后浏览器跳回，重定向到控制台充值页。
pub async fn return_page(State(state): State<Arc<AppState>>, Query(q): Query<HashMap<String, String>>) -> Redirect {
    let base = state.config.public_base_url.trim_end_matches('/');
    let order = q.get("out_trade_no").cloned().unwrap_or_default();
    Redirect::to(&format!("{base}/recharge?order={}", epay::pct_encode(&order)))
}

/// GET /v1/payment/orders/{id} — 查询自己的订单状态（前端轮询）。
pub async fn order_status(
    State(state): State<Arc<AppState>>,
    CurrentUser(u): CurrentUser,
    Path(id): Path<String>,
) -> Response {
    match state.account.store.get_payment_order(&id).await {
        Some(o) if o.user_id == u.id => Json(json!({
            "order_id": o.order_id,
            "status": o.status,
            "amount": o.amount,
            "paid_at": o.paid_at,
            "created_at": o.created_at,
        }))
        .into_response(),
        _ => jerr(StatusCode::NOT_FOUND, "order not found"),
    }
}
