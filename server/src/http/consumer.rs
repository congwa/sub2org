//! OpenAI 兼容消费入口（M2）：会话亲和 + 调度排队 + 持久化钉定。

use std::convert::Infallible;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine as _;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use uuid::Uuid;

use crate::account::{Charge, RateGuard};
use crate::http::auth;
use crate::metrics::LiveEvent;
use crate::pool::registry::Lease;
use crate::pool::scheduler::{self, SchedError};
use crate::state::AppState;
use crate::store::{Session, Store};
use crate::stream::StreamEvent;
use crate::ws::protocol::Dispatch;

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    #[serde(default)]
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: bool,
}

#[derive(Debug, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default)]
    pub content: Value,
}

/// 请求结束时把 req_id 从 pending 表移除，并把会话钉定持久化。
/// 响应流结束时向 worker 发 cancel（中止仍在跑的生成、释放真实槽）。
/// #1 关键：它**不再** remove_pending——结算/退款改由 gateway 的 worker 终止事件驱动，
/// 独立于 HTTP 连接是否还在。否则客户端读完正文即断连 → future drop → 摘除 entry →
/// Charge::Drop 全额退款 = 白嫖。entry 的清理交给 done/error/断连快速失败/reaper。
struct CancelGuard {
    state: Arc<AppState>,
    req_id: String,
}
impl Drop for CancelGuard {
    fn drop(&mut self) {
        self.state.cancel_req(&self.req_id);
    }
}

/// 会话钉定信息，随响应流一起持有，在 done 时落库。
#[derive(Clone)]
struct SessionPin {
    store: Arc<dyn Store>,
    session_id: String,
    api_key_id: String,
    worker_id: String,
    provider: String,
    prior_native: Option<String>,
}
impl SessionPin {
    async fn persist(&self, native_from_done: Option<String>) {
        let native = native_from_done
            .filter(|s| !s.is_empty())
            .or_else(|| self.prior_native.clone());
        let now = now_unix() as i64;
        let s = Session {
            session_id: self.session_id.clone(),
            api_key_id: self.api_key_id.clone(),
            worker_id: self.worker_id.clone(),
            provider: self.provider.clone(),
            native_conversation_id: native,
            created_at: now,
            last_used_at: now,
        };
        let _ = self.store.upsert_session(&s).await;
    }
}

pub async fn chat_completions(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<ChatRequest>,
) -> Response {
    // ---- 鉴权（api_key → 用户）----
    let key = match auth::bearer(&headers) {
        Some(k) => k,
        None => return err(StatusCode::UNAUTHORIZED, "missing bearer token"),
    };
    let resolved = match state.account.store.resolve_key(&key).await {
        Some(r) => r,
        None => return err(StatusCode::UNAUTHORIZED, "invalid api key"),
    };
    if resolved.key_status != "active" {
        return err(StatusCode::UNAUTHORIZED, "api key disabled");
    }
    if resolved.user_status != "active" {
        return err(StatusCode::FORBIDDEN, "user banned");
    }
    let user_id = resolved.user_id.clone();

    // ---- model 准入（语义收敛）：operator 声明了 models 白名单时，拒绝白名单外/无法路由的 model ----
    // models 为空（默认）= 宽松：任何 model 都按 provider_for_model 的默认路由放行（保持旧行为）。
    if !state.config.models.is_empty() && !state.config.model_supported(&body.model) {
        return err(
            StatusCode::BAD_REQUEST,
            &format!("model '{}' not supported (see GET /v1/models)", body.model),
        );
    }

    // ---- 邮箱验证门槛（默认关；require_email_verify 开启时强制）----
    if state.account.require_email_verify() {
        let verified = state.account.store.get_user(&user_id).await.map(|u| u.email_verified).unwrap_or(false);
        if !verified {
            return err(StatusCode::FORBIDDEN, "email not verified");
        }
    }

    // ---- 用户级并发限流 ----
    let rate_guard = match state.account.try_rate(&user_id, resolved.rate_limit) {
        Some(g) => g,
        None => return err(StatusCode::TOO_MANY_REQUESTS, "rate limit exceeded"),
    };

    // ---- 图片/多模态：从最后一条 user 消息提取图，归一为 data URI（base64 透传 / https 服务端 fetch）。
    // 在占用 worker 之前做（坏图/超限/私网 URL → 400，不消耗 lease）。----
    let images = if state.config.image.enabled {
        match collect_images(&state.config.image, &state.image_fetch_sem, &body.messages).await {
            Ok(imgs) => imgs,
            Err(e) => return err(StatusCode::BAD_REQUEST, &format!("image error: {e}")),
        }
    } else {
        Vec::new()
    };

    // OpenAI 路径：预渲染整段(new) + 末条 user(continue)，交给共享中继核心。
    let prompt_new = render_prompt(&body.messages);
    let prompt_continue = last_user_message(&body.messages).unwrap_or_else(|| prompt_new.clone());
    let prompt_tokens = estimate_prompt_tokens(&body.messages);
    dispatch_and_relay(
        state, headers, user_id, resolved.group, body.model, images,
        prompt_new, prompt_continue, prompt_tokens, body.stream, rate_guard,
        ResponseFormat::OpenAI,
    )
    .await
}

/// 对外响应格式：OpenAI Chat Completions 或 Anthropic Messages（A1）。
#[derive(Clone, Copy, PartialEq)]
enum ResponseFormat {
    OpenAI,
    Anthropic,
}

/// 共享中继核心（A1）：会话解析 → 调度/失败转移 → 计费预扣 → add_pending → Dispatch emit →
/// SessionPin → 按 (format, stream) 派发到对应 responder。OpenAI 与 Anthropic 两个入口共用此函数，
/// 杜绝中继不变量（CancelGuard / 服务端侧结算 / 快速失败 / worker_index）在两条路径上漂移。
#[allow(clippy::too_many_arguments)]
async fn dispatch_and_relay(
    state: Arc<AppState>,
    headers: HeaderMap,
    user_id: String,
    group: String,
    model: String,
    images: Vec<String>,
    prompt_new: String,
    prompt_continue: String,
    prompt_tokens: u32,
    stream: bool,
    rate_guard: RateGuard,
    format: ResponseFormat,
) -> Response {
    // 无路由命中时的 provider 默认：OpenAI→chatgpt（旧行为），Anthropic→claude。
    let default_provider = match format {
        ResponseFormat::OpenAI => state.config.provider_for_model(&model),
        ResponseFormat::Anthropic => state
            .config
            .provider_for_model_opt(&model)
            .unwrap_or_else(|| "claude".to_string()),
    };

    // ---- 会话解析 ----
    let session_hdr = header(&headers, "x-sub2org-session");
    let new_window = header(&headers, "x-sub2org-new-window")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let existing = match &session_hdr {
        Some(id) => state.store.get_session(id).await,
        None => None,
    };

    let (provider, action, target_worker, prior_native, session_id) = match existing {
        Some(sess) if !new_window => (
            sess.provider.clone(),
            "continue",
            Some(sess.worker_id.clone()),
            sess.native_conversation_id.clone(),
            sess.session_id.clone(),
        ),
        Some(sess) => (
            sess.provider.clone(),
            "new",
            Some(sess.worker_id.clone()),
            None,
            sess.session_id.clone(),
        ),
        None => {
            let provider = header(&headers, "x-sub2org-provider").unwrap_or(default_provider);
            let sid = session_hdr
                .clone()
                .unwrap_or_else(|| format!("sess-{}", Uuid::new_v4().simple()));
            (provider, "new", None, None, sid)
        }
    };

    // ---- 调度（含排队等待）----
    let q_start = Instant::now();
    let wait = Duration::from_secs(state.config.queue_wait_secs);
    let lease = match &target_worker {
        Some(wid) => match scheduler::acquire_specific(&state.registry, wid, wait).await {
            Ok(l) => l,
            Err(SchedError::WorkerOffline) => {
                if state.config.failover_on_offline {
                    match scheduler::acquire_any(&state.registry, &provider, &group, wait).await {
                        Ok(l) => l,
                        Err(_) => {
                            return err(StatusCode::SERVICE_UNAVAILABLE, "no idle worker (failover)")
                        }
                    }
                } else {
                    return err(StatusCode::CONFLICT, "session worker offline");
                }
            }
            Err(SchedError::Timeout) => {
                return err(StatusCode::SERVICE_UNAVAILABLE, "queue wait timed out")
            }
        },
        None => match scheduler::acquire_any(&state.registry, &provider, &group, wait).await {
            Ok(l) => l,
            Err(_) => {
                let hint = state
                    .registry
                    .earliest_cooldown(&provider, &group, now_unix() as i64)
                    .map(|t| format!(" (earliest worker recovers at unix {t})"))
                    .unwrap_or_default();
                return err(
                    StatusCode::SERVICE_UNAVAILABLE,
                    &format!("no available worker for provider '{provider}'{hint}"),
                );
            }
        },
    };

    state.queue_wait.record(q_start.elapsed().as_millis() as u64);

    // failover 后 worker 变了 → 强制 new 窗口
    let action = if target_worker.as_deref() != Some(lease.worker_id.as_str()) {
        "new"
    } else {
        action
    };
    let prior_native = if action == "new" { None } else { prior_native };

    let prompt = if action == "continue" { prompt_continue } else { prompt_new };

    let id_prefix = match format {
        ResponseFormat::OpenAI => "chatcmpl",
        ResponseFormat::Anthropic => "msg",
    };
    let req_id = format!("{id_prefix}-{}", Uuid::new_v4().simple());

    // ---- 计费预扣（billing 关或 cost=0 时跳过）----
    let cost = if state.account.billing_enabled() {
        let base = state.account.cost_for_model(&model); // 未乘倍率的基础价
        let c = state.account.cost_for(&model, 1.0); // B3：倍率（v1 per-key 恒 1.0）
        // #19：基础价>0 但全局/model 倍率四舍五入到 0 → floor 1，堵免费漏洞。
        if base > 0 && c == 0 { 1 } else { c }
    } else {
        0
    };
    let mut charge: Option<Charge> = None;
    if cost > 0 {
        match state.account.reserve_charge(&user_id, cost, &req_id, &model).await {
            Ok((c, balance_after)) => {
                if let Some(th) = state.account.balance_low_threshold() {
                    if th > 0 && balance_after < th {
                        state.publish(LiveEvent::BalanceLow {
                            user_id: user_id.clone(),
                            balance: balance_after,
                            threshold: th,
                        });
                    }
                }
                charge = Some(c);
            }
            Err(_) => return err(StatusCode::PAYMENT_REQUIRED, "insufficient balance"),
        }
    }

    // P1#7：暂保持无界通道（改有界涉 socketioxide 事件乱序风险，详见 v0.6.2 注释）。
    let (tx, rx) = mpsc::unbounded_channel::<StreamEvent>();
    // charge 移入 pending：结算/退款由 gateway 的 worker 事件驱动，独立于 HTTP 连接（治断连即免费 #1）。
    state.add_pending(req_id.clone(), lease.worker_id.clone(), lease.sid, tx, charge); // P1#6

    let dispatch = Dispatch {
        req_id: req_id.clone(),
        session_id: session_id.clone(),
        action: action.to_string(),
        native_conversation_id: prior_native.clone(),
        provider: provider.clone(),
        prompt,
        images,
    };

    let dispatched = match state.io.get_socket(lease.sid) {
        Some(sock) => sock.emit("dispatch", &dispatch).is_ok(),
        None => false,
    };
    if !dispatched {
        state.remove_pending(&req_id); // charge 随 entry drop 自动退款
        return err(StatusCode::BAD_GATEWAY, "failed to dispatch to worker");
    }

    tracing::info!(
        category = "dispatch",
        req_id = %req_id,
        worker_id = %lease.worker_id,
        session_id = %session_id,
        user_id = %user_id,
        action = action,
        provider = %provider,
        "dispatched to worker"
    );

    state.req_ring.incr(now_unix() as i64);
    state.publish(LiveEvent::RequestDispatched {
        req_id: req_id.clone(),
        model: model.clone(),
        worker_id: lease.worker_id.clone(),
        session_id: session_id.clone(),
        action: action.to_string(),
    });
    let started = Instant::now();

    let pin = SessionPin {
        store: state.store.clone(),
        session_id: session_id.clone(),
        api_key_id: user_id, // sessions 以 user 维度记归属
        worker_id: lease.worker_id.clone(),
        provider,
        prior_native,
    };

    let gen = Duration::from_secs(state.config.generation_secs);
    match (format, stream) {
        (ResponseFormat::OpenAI, true) => {
            stream_response(state, lease, rx, req_id, model, gen, pin, session_id, rate_guard, started, prompt_tokens)
        }
        (ResponseFormat::OpenAI, false) => {
            json_response(state, lease, rx, req_id, model, gen, pin, session_id, rate_guard, started, prompt_tokens).await
        }
        (ResponseFormat::Anthropic, true) => {
            messages_stream_response(state, lease, rx, req_id, model, gen, pin, session_id, rate_guard, started, prompt_tokens)
        }
        (ResponseFormat::Anthropic, false) => {
            messages_json_response(state, lease, rx, req_id, model, gen, pin, session_id, rate_guard, started, prompt_tokens).await
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn stream_response(
    state: Arc<AppState>,
    lease: Lease,
    mut rx: UnboundedReceiver<StreamEvent>,
    req_id: String,
    model: String,
    gen: Duration,
    pin: SessionPin,
    session_id: String,
    rate: RateGuard,
    started: Instant,
    prompt_tokens: u32,
) -> Response {
    let created = now_unix();
    let id = req_id.clone();
    let s = async_stream::stream! {
        let _lease = lease; // 占用槽位直到流结束
        let _rate = rate;   // 占用并发名额直到流结束
        let mut got_content = false; // E3：是否真有内容流出（含 DOM 兜底的 chunk）
        let mut acc = String::new(); // A2：累计正文用于估算 completion_tokens
        // 计费不在此结算：charge 在 pending 里，由 gateway 的 done/error 驱动（治断连即免费 #1）。
        let _guard = CancelGuard { state: state.clone(), req_id: req_id.clone() };

        yield Ok::<Event, Infallible>(Event::default().data(chunk_json(
            &id, &model, created, json!({"role": "assistant", "content": ""}), None, None,
        )));

        loop {
            match tokio::time::timeout(gen, rx.recv()).await {
                Ok(Some(StreamEvent::Delta(d))) => {
                    if !d.is_empty() { got_content = true; }
                    acc.push_str(&d);
                    yield Ok(Event::default().data(chunk_json(
                        &id, &model, created, json!({"content": d}), None, None,
                    )));
                }
                Ok(Some(StreamEvent::Done { conversation_id, full })) => {
                    if !full.trim().is_empty() { got_content = true; if full.len() >= acc.len() { acc = full.clone(); } }
                    // E3：零内容结束（解析漂移 + DOM 兜底失败）→ 发 error 帧，不静默 stop（退款由 gateway 完成）。
                    if !got_content {
                        state.publish(LiveEvent::RequestError { req_id: id.clone(), code: "empty_response".into() });
                        yield Ok(Event::default().data(
                            json!({"error": {"message": "worker returned empty response", "type": "empty_response"}}).to_string(),
                        ));
                        break;
                    }
                    pin.persist(conversation_id).await;
                    let completion_tokens = crate::billing::tokens::estimate_tokens(&acc); // A2
                    state.publish(LiveEvent::RequestDone {
                        req_id: id.clone(), status: "stop".into(),
                        tokens: Some(prompt_tokens.saturating_add(completion_tokens)),
                        latency_ms: started.elapsed().as_millis() as u64,
                    });
                    yield Ok(Event::default().data(chunk_json(
                        &id, &model, created, json!({}), Some("stop"),
                        Some(json!({
                            "prompt_tokens": prompt_tokens,
                            "completion_tokens": completion_tokens,
                            "total_tokens": prompt_tokens.saturating_add(completion_tokens)
                        })),
                    )));
                    break;
                }
                Ok(Some(StreamEvent::Error(e))) => {
                    state.publish(LiveEvent::RequestError { req_id: id.clone(), code: "worker_error".into() });
                    yield Ok(Event::default().data(
                        json!({"error": {"message": e, "type": "worker_error"}}).to_string(),
                    ));
                    break;
                }
                Ok(None) => {
                    state.publish(LiveEvent::RequestError { req_id: id.clone(), code: "disconnected".into() });
                    // P0#1：与其它失败分支一致，先 yield error 帧再 [DONE]；
                    // 否则发送端被 drop（worker/中转断开）时客户端只见 [DONE] → 把截断当成功。
                    yield Ok(Event::default().data(
                        json!({"error": {"message": "worker disconnected", "type": "disconnected"}}).to_string(),
                    ));
                    break;
                }
                Err(_) => {
                    state.publish(LiveEvent::RequestError { req_id: id.clone(), code: "timeout".into() });
                    yield Ok(Event::default().data(
                        json!({"error": {"message": "generation timeout", "type": "timeout"}}).to_string(),
                    ));
                    break;
                }
            }
        }
        yield Ok(Event::default().data("[DONE]"));
    };
    let sse = Sse::new(s).keep_alive(KeepAlive::default());
    ([("x-sub2org-session", session_id)], sse).into_response()
}

#[allow(clippy::too_many_arguments)]
async fn json_response(
    state: Arc<AppState>,
    lease: Lease,
    mut rx: UnboundedReceiver<StreamEvent>,
    req_id: String,
    model: String,
    gen: Duration,
    pin: SessionPin,
    session_id: String,
    rate: RateGuard,
    started: Instant,
    prompt_tokens: u32,
) -> Response {
    let _lease = lease;
    let _rate = rate;
    // 计费不在此结算：charge 在 pending 里，由 gateway 的 done/error 驱动（治断连即免费 #1）。
    let _guard = CancelGuard {
        state: state.clone(),
        req_id: req_id.clone(),
    };
    let created = now_unix();
    let mut full = String::new();
    loop {
        match tokio::time::timeout(gen, rx.recv()).await {
            Ok(Some(StreamEvent::Delta(d))) => full.push_str(&d),
            Ok(Some(StreamEvent::Done { full: f, conversation_id })) => {
                if !f.is_empty() {
                    full = f;
                }
                // E3：流正常结束但零内容（SSE 解析漂移 + DOM 兜底也失败）→ 显式失败，
                // 不再静默返回空 200（退款由 gateway 完成）。
                if full.trim().is_empty() {
                    state.publish(LiveEvent::RequestError { req_id: req_id.clone(), code: "empty_response".into() });
                    return err(StatusCode::BAD_GATEWAY, "worker returned empty response");
                }
                pin.persist(conversation_id).await;
                state.publish(LiveEvent::RequestDone {
                    req_id: req_id.clone(), status: "stop".into(),
                    tokens: Some(prompt_tokens.saturating_add(crate::billing::tokens::estimate_tokens(&full))),
                    latency_ms: started.elapsed().as_millis() as u64,
                });
                break;
            }
            Ok(Some(StreamEvent::Error(e))) => {
                state.publish(LiveEvent::RequestError { req_id: req_id.clone(), code: "worker_error".into() });
                return err(StatusCode::BAD_GATEWAY, &e);
            }
            Ok(None) => {
                // P0#1：发送端被 drop（worker/中转断开）→ 显式 502，不把已收的部分 full 当 200 返回。
                state.publish(LiveEvent::RequestError { req_id: req_id.clone(), code: "disconnected".into() });
                return err(StatusCode::BAD_GATEWAY, "worker disconnected");
            }
            Err(_) => {
                state.publish(LiveEvent::RequestError { req_id: req_id.clone(), code: "timeout".into() });
                return err(StatusCode::GATEWAY_TIMEOUT, "generation timeout");
            }
        }
    }
    let completion_tokens = crate::billing::tokens::estimate_tokens(&full); // A2
    (
        [("x-sub2org-session", session_id)],
        Json(completion_json(&req_id, &model, created, &full, prompt_tokens, completion_tokens)),
    )
        .into_response()
}

pub async fn list_models(State(state): State<Arc<AppState>>) -> Json<Value> {
    // config.models 为来源；空则回退默认表（向后兼容）。
    let ids: Vec<String> = if state.config.models.is_empty() {
        vec!["gpt-4o".to_string(), "claude".to_string()]
    } else {
        state.config.models.clone()
    };
    let data: Vec<Value> = ids
        .into_iter()
        .map(|id| json!({"id": id, "object": "model", "owned_by": "sub2org"}))
        .collect();
    Json(json!({ "object": "list", "data": data }))
}

/// GET /v1/pricing — 公开定价（按次/按模型固定价 + 充值换算）。
pub async fn pricing(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "billing_enabled": state.account.billing_enabled(),
        "default_cost": state.config.default_cost,
        "model_cost": state.config.model_cost,
        "cost_multiplier": state.config.cost_multiplier,
        "model_multiplier": state.config.model_multiplier,
        "credits_per_cny": state.config.payment.as_ref().map(|p| p.credits_per_cny),
        // B5：仅在启用注册验证码时下发公开 site key（控制台据此渲染 Turnstile widget）。
        "turnstile_site_key": state.config.turnstile.as_ref().filter(|t| t.on_register).map(|t| t.site_key.clone()),
    }))
}

// ---- helpers ----

fn header(h: &HeaderMap, name: &str) -> Option<String> {
    h.get(name).and_then(|v| v.to_str().ok()).map(|s| s.trim().to_string())
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn chunk_json(id: &str, model: &str, created: u64, delta: Value, finish: Option<&str>, usage: Option<Value>) -> String {
    let mut v = json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]
    });
    if let Some(u) = usage {
        v["usage"] = u; // A2：仅末帧带 usage（OpenAI 约定）
    }
    v.to_string()
}

fn completion_json(id: &str, model: &str, created: u64, content: &str, prompt_tokens: u32, completion_tokens: u32) -> Value {
    json!({
        "id": id,
        "object": "chat.completion",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": content},
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens,
            "total_tokens": prompt_tokens.saturating_add(completion_tokens)
        }
    })
}

fn err(status: StatusCode, msg: &str) -> Response {
    (
        status,
        Json(json!({"error": {"message": msg, "type": "sub2org_error"}})),
    )
        .into_response()
}

fn content_to_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(arr) => arr
            .iter()
            .filter_map(|p| {
                if p.get("type").and_then(|t| t.as_str()) == Some("text") {
                    p.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(""),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// A2：估算请求 prompt 的 token（基于完整 messages，含每条框架开销 + 回复引导）。仅用于 usage 上报。
fn estimate_prompt_tokens(msgs: &[ChatMessage]) -> u32 {
    use crate::billing::tokens::estimate_tokens;
    let mut total: u32 = 2; // 回复引导
    for m in msgs {
        total = total
            .saturating_add(4) // role + 分隔框架开销
            .saturating_add(estimate_tokens(&m.role))
            .saturating_add(estimate_tokens(&content_to_text(&m.content)));
    }
    total
}

/// 新会话：把整段对话渲染成单条 prompt。
fn render_prompt(msgs: &[ChatMessage]) -> String {
    msgs.iter()
        .filter_map(|m| {
            let text = content_to_text(&m.content);
            if text.trim().is_empty() {
                None
            } else {
                Some(format!("{}: {}", m.role, text))
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// 续接：取最后一条 user 消息文本。
fn last_user_message(msgs: &[ChatMessage]) -> Option<String> {
    msgs.iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| content_to_text(&m.content))
        .filter(|s| !s.trim().is_empty())
}

// ---- 图片/多模态 ----

/// 从最后一条 user 消息的 content 数组提取 image_url，归一为 `data:` URI 列表。
/// base64 data URI 直接透传（校验大小）；https URL 由服务端 fetch（SSRF 守卫）后转 data URI。
async fn collect_images(
    cfg: &crate::config::ImageConfig,
    sem: &tokio::sync::Semaphore,
    msgs: &[ChatMessage],
) -> Result<Vec<String>, String> {
    // 仅取最后一条 user 消息的图；更早消息里的图会被丢弃（多轮历史图由网页会话自身保留），告警便于排查。
    let last_idx = msgs.iter().rposition(|m| m.role == "user");
    if msgs.iter().enumerate().any(|(i, m)| Some(i) != last_idx && content_has_image(&m.content)) {
        tracing::warn!(category = "dispatch", "图片仅取最后一条 user 消息；更早消息中的图片被忽略");
    }
    let arr = match last_idx.map(|i| &msgs[i].content) {
        Some(Value::Array(a)) => a,
        _ => return Ok(Vec::new()),
    };
    let mut out: Vec<String> = Vec::new();
    let mut total: usize = 0;
    let total_cap = cfg.max_bytes.saturating_mul(cfg.max_count.max(1));
    for part in arr {
        if part.get("type").and_then(|t| t.as_str()) != Some("image_url") {
            continue;
        }
        let url = part
            .get("image_url")
            .and_then(|u| u.get("url"))
            .and_then(|u| u.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        let url = match url {
            Some(u) => u,
            None => continue,
        };
        if out.len() >= cfg.max_count {
            return Err(format!("too many images (max {})", cfg.max_count));
        }
        let (data_uri, decoded) = if url.starts_with("data:") {
            let b64 = url.split_once(',').map(|(_, b)| b).unwrap_or("");
            if b64.is_empty() {
                return Err("empty data uri".into());
            }
            let approx = b64.len() / 4 * 3; // 估算解码大小（忽略 padding 的轻微高估可接受）
            if approx > cfg.max_bytes {
                return Err(format!("image too large (max {} bytes)", cfg.max_bytes));
            }
            (url.to_string(), approx)
        } else if url.starts_with("http://") || url.starts_with("https://") {
            if !cfg.fetch_url {
                return Err("url images disabled (only base64 data URI accepted)".into());
            }
            let uri = fetch_to_data_uri(url, cfg, sem).await?;
            let d = uri.split_once(',').map(|(_, b)| b.len() / 4 * 3).unwrap_or(0);
            (uri, d)
        } else {
            return Err("unsupported image url scheme".into());
        };
        // per-request 总字节上限：防多图累计内存放大。
        total = total.saturating_add(decoded);
        if total > total_cap {
            return Err("images total too large".into());
        }
        out.push(data_uri);
    }
    Ok(out)
}

/// content（数组）是否含 image_url 部分。
fn content_has_image(v: &Value) -> bool {
    matches!(v, Value::Array(a) if a.iter().any(|p| p.get("type").and_then(|t| t.as_str()) == Some("image_url")))
}

/// 服务端 fetch https 图片 → `data:<mime>;base64,...`。
/// SSRF 加固：①解析一次得 IP 并逐一校验 ②把校验过的 IP **钉定**到连接（resolve_to_addrs，消除 reqwest 二次解析的 DNS rebinding TOCTOU）
/// ③不跟随重定向 ④**流式**读取边累计边截断（content-length 可缺失/伪造，唯一可靠上限）⑤全局并发信号量。
async fn fetch_to_data_uri(
    url: &str,
    cfg: &crate::config::ImageConfig,
    sem: &tokio::sync::Semaphore,
) -> Result<String, String> {
    let _permit = sem.acquire().await.map_err(|_| "image fetch semaphore closed".to_string())?;
    let (host, port) = parse_host_port(url)?;
    let vetted = resolve_vetted(&host, port)?; // 已逐一过 is_blocked_ip
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(cfg.fetch_timeout_secs))
        .redirect(reqwest::redirect::Policy::none()) // 不跟随重定向：防绕过守卫跳私网
        .resolve_to_addrs(&host, &vetted) // 钉定：reqwest 只连这些已校验 IP，不再二次解析
        .build()
        .map_err(|e| e.to_string())?;
    // #7：连接类瞬断（ECONNRESET/超时/握手失败等）重试一次再放弃——同站点偶发抖动不必整请求挂掉。
    let mut resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(e1) => {
            tracing::warn!(category = "image", "图片 fetch 瞬断重试一次：{e1}");
            client.get(url).send().await.map_err(|e| format!("fetch failed: {e}"))?
        }
    };
    if !resp.status().is_success() {
        return Err(format!("fetch status {}", resp.status()));
    }
    // #7：content-type 存在但非 image/* → 拒绝（多半是错误页/网盘 HTML，灌进模型无意义）；仅 header 缺失才回退 png。
    let mime = match resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(';').next().unwrap_or("").trim().to_string())
    {
        Some(m) if m.starts_with("image/") => m,
        Some(m) => return Err(format!("not an image (content-type {m})")),
        None => "image/png".to_string(),
    };
    // 流式累计：超 max_bytes 立即中止丢弃连接（不依赖 content-length）。
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("read failed: {e}"))? {
        if buf.len().saturating_add(chunk.len()) > cfg.max_bytes {
            return Err(format!("image too large (max {} bytes)", cfg.max_bytes));
        }
        buf.extend_from_slice(&chunk);
    }
    if buf.is_empty() {
        return Err("empty image body".into());
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
    Ok(format!("data:{mime};base64,{b64}"))
}

/// 从 http(s) URL 解析 (host, port)；默认端口 https=443 / http=80。处理 userinfo 与 IPv6 字面量。
fn parse_host_port(url: &str) -> Result<(String, u16), String> {
    let default_port = if url.starts_with("https://") {
        443
    } else if url.starts_with("http://") {
        80
    } else {
        return Err("unsupported scheme".into());
    };
    let after = url.splitn(2, "://").nth(1).unwrap_or("");
    let authority = after.split(['/', '?', '#']).next().unwrap_or("");
    let authority = authority.rsplit('@').next().unwrap_or(authority); // 去 userinfo
    if authority.is_empty() {
        return Err("bad url host".into());
    }
    if let Some(rest) = authority.strip_prefix('[') {
        // IPv6 字面量 [::1] 或 [::1]:port
        let end = rest.find(']').ok_or("bad ipv6 authority")?;
        let host = rest[..end].to_string();
        let port = rest[end + 1..]
            .strip_prefix(':')
            .and_then(|p| p.parse().ok())
            .unwrap_or(default_port);
        Ok((host, port))
    } else if let Some((h, p)) = authority.rsplit_once(':') {
        match p.parse::<u16>() {
            Ok(port) => Ok((h.to_string(), port)),
            Err(_) => Ok((authority.to_string(), default_port)),
        }
    } else {
        Ok((authority.to_string(), default_port))
    }
}

/// 解析 host → SocketAddr 集合，逐一过 is_blocked_ip；返回的集合用于钉定连接（防二次解析 TOCTOU）。
fn resolve_vetted(host: &str, port: u16) -> Result<Vec<SocketAddr>, String> {
    let addrs: Vec<SocketAddr> = if let Ok(ip) = host.parse::<IpAddr>() {
        vec![SocketAddr::new(ip, port)]
    } else {
        use std::net::ToSocketAddrs;
        (host, port)
            .to_socket_addrs()
            .map_err(|e| format!("dns resolve failed: {e}"))?
            .collect()
    };
    if addrs.is_empty() {
        return Err("host did not resolve".into());
    }
    if addrs.iter().any(|a| is_blocked_ip(&a.ip())) {
        return Err("blocked (private/loopback/link-local ip)".into());
    }
    Ok(addrs)
}

// ==== Anthropic Messages API（A1）====
// 复用 dispatch_and_relay 整条中继管线；仅入参解析 + 出参封装与 OpenAI 不同。
// 边界：网页 UI 中继给不了结构化 tool_use → 接受 tools 但忽略，stop_reason 恒 end_turn；
// max_tokens/temperature/stop_sequences 接受但不可强制；system 折进 prompt；usage 为估算。

#[derive(Debug, Deserialize)]
pub struct MessagesRequest {
    pub model: String,
    #[serde(default)]
    pub messages: Vec<AnthropicMessage>,
    /// string | 块数组 | null
    #[serde(default)]
    pub system: Value,
    #[serde(default)]
    pub stream: bool,
    // 以下接受但网页 UI 不可强制（忽略）：
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(default)]
    pub tools: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicMessage {
    pub role: String,
    #[serde(default)]
    pub content: Value,
}

/// Anthropic content（string | 块数组）→ 纯文本：取 type=="text" 块，其余（image/tool_use/tool_result）跳过。
fn anthropic_content_to_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(arr) => arr
            .iter()
            .filter_map(|p| {
                if p.get("type").and_then(|t| t.as_str()) == Some("text") {
                    p.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(""),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// 新会话渲染：system 折成前置行 + 各 role: text。
fn anthropic_render_prompt(system: &Value, msgs: &[AnthropicMessage]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let sys = anthropic_content_to_text(system);
    if !sys.trim().is_empty() {
        parts.push(format!("system: {sys}"));
    }
    for m in msgs {
        let text = anthropic_content_to_text(&m.content);
        if !text.trim().is_empty() {
            parts.push(format!("{}: {}", m.role, text));
        }
    }
    parts.join("\n\n")
}

/// 续接：取最后一条 user 消息文本。
fn anthropic_last_user_message(msgs: &[AnthropicMessage]) -> Option<String> {
    msgs.iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| anthropic_content_to_text(&m.content))
        .filter(|s| !s.trim().is_empty())
}

/// A2：估算 Anthropic prompt token（system + 各消息）。
fn anthropic_estimate_prompt_tokens(system: &Value, msgs: &[AnthropicMessage]) -> u32 {
    use crate::billing::tokens::estimate_tokens;
    let mut total: u32 = 2;
    total = total.saturating_add(estimate_tokens(&anthropic_content_to_text(system)));
    for m in msgs {
        total = total
            .saturating_add(4)
            .saturating_add(estimate_tokens(&m.role))
            .saturating_add(estimate_tokens(&anthropic_content_to_text(&m.content)));
    }
    total
}

/// 从最后一条 user 消息提取 Anthropic 图片块 → data URI（复用 OpenAI 路径的上限与 SSRF fetch）。
/// 块形如 {type:"image", source:{type:"base64", media_type, data} | {type:"url", url}}。
async fn collect_images_anthropic(
    cfg: &crate::config::ImageConfig,
    sem: &tokio::sync::Semaphore,
    msgs: &[AnthropicMessage],
) -> Result<Vec<String>, String> {
    let Some(m) = msgs.iter().rev().find(|m| m.role == "user") else {
        return Ok(Vec::new());
    };
    let Value::Array(arr) = &m.content else {
        return Ok(Vec::new());
    };
    let mut uris: Vec<String> = Vec::new();
    for block in arr {
        if block.get("type").and_then(|t| t.as_str()) != Some("image") {
            continue;
        }
        let Some(src) = block.get("source") else { continue };
        let uri = match src.get("type").and_then(|t| t.as_str()).unwrap_or("") {
            "base64" => {
                let mt = src.get("media_type").and_then(|t| t.as_str()).unwrap_or("image/png");
                let data = src.get("data").and_then(|t| t.as_str()).unwrap_or("");
                if data.is_empty() {
                    return Err("empty base64 image data".into());
                }
                format!("data:{mt};base64,{data}")
            }
            "url" => {
                let url = src.get("url").and_then(|t| t.as_str()).unwrap_or("");
                if !cfg.fetch_url {
                    return Err("image url fetch disabled".into());
                }
                fetch_to_data_uri(url, cfg, sem).await?
            }
            _ => continue,
        };
        if uri.len() / 4 * 3 > cfg.max_bytes {
            return Err(format!("image too large (max {} bytes)", cfg.max_bytes));
        }
        uris.push(uri);
        if uris.len() >= cfg.max_count {
            break;
        }
    }
    Ok(uris)
}

/// POST /v1/messages — Anthropic Messages API 入口。
pub async fn messages(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<MessagesRequest>,
) -> Response {
    let key = match auth::bearer(&headers) {
        Some(k) => k,
        None => return err_anthropic(StatusCode::UNAUTHORIZED, "authentication_error", "missing api key"),
    };
    let resolved = match state.account.store.resolve_key(&key).await {
        Some(r) => r,
        None => return err_anthropic(StatusCode::UNAUTHORIZED, "authentication_error", "invalid api key"),
    };
    if resolved.key_status != "active" {
        return err_anthropic(StatusCode::UNAUTHORIZED, "authentication_error", "api key disabled");
    }
    if resolved.user_status != "active" {
        return err_anthropic(StatusCode::FORBIDDEN, "permission_error", "user banned");
    }
    let user_id = resolved.user_id.clone();

    if !state.config.models.is_empty() && !state.config.model_supported(&body.model) {
        return err_anthropic(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            &format!("model '{}' not supported", body.model),
        );
    }
    if state.account.require_email_verify() {
        let verified = state.account.store.get_user(&user_id).await.map(|u| u.email_verified).unwrap_or(false);
        if !verified {
            return err_anthropic(StatusCode::FORBIDDEN, "permission_error", "email not verified");
        }
    }
    let rate_guard = match state.account.try_rate(&user_id, resolved.rate_limit) {
        Some(g) => g,
        None => return err_anthropic(StatusCode::TOO_MANY_REQUESTS, "rate_limit_error", "rate limit exceeded"),
    };
    let images = if state.config.image.enabled {
        match collect_images_anthropic(&state.config.image, &state.image_fetch_sem, &body.messages).await {
            Ok(imgs) => imgs,
            Err(e) => return err_anthropic(StatusCode::BAD_REQUEST, "invalid_request_error", &format!("image error: {e}")),
        }
    } else {
        Vec::new()
    };

    let prompt_new = anthropic_render_prompt(&body.system, &body.messages);
    let prompt_continue = anthropic_last_user_message(&body.messages).unwrap_or_else(|| prompt_new.clone());
    let prompt_tokens = anthropic_estimate_prompt_tokens(&body.system, &body.messages);
    dispatch_and_relay(
        state, headers, user_id, resolved.group, body.model, images,
        prompt_new, prompt_continue, prompt_tokens, body.stream, rate_guard,
        ResponseFormat::Anthropic,
    )
    .await
}

fn err_anthropic(status: StatusCode, etype: &str, msg: &str) -> Response {
    (status, Json(json!({"type": "error", "error": {"type": etype, "message": msg}}))).into_response()
}

fn message_json(id: &str, model: &str, content: &str, input_tokens: u32, output_tokens: u32) -> Value {
    json!({
        "id": id,
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": [{"type": "text", "text": content}],
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "usage": {"input_tokens": input_tokens, "output_tokens": output_tokens}
    })
}

/// Anthropic 流式：message_start → content_block_start → content_block_delta* → content_block_stop
/// → message_delta → message_stop。无 [DONE]。失败分支发 Anthropic error 事件。
#[allow(clippy::too_many_arguments)]
fn messages_stream_response(
    state: Arc<AppState>,
    lease: Lease,
    mut rx: UnboundedReceiver<StreamEvent>,
    req_id: String,
    model: String,
    gen: Duration,
    pin: SessionPin,
    session_id: String,
    rate: RateGuard,
    started: Instant,
    prompt_tokens: u32,
) -> Response {
    let id = req_id.clone();
    let s = async_stream::stream! {
        let _lease = lease;
        let _rate = rate;
        let mut got_content = false;
        let mut acc = String::new();
        // 计费不在此结算：charge 在 pending 里，由 gateway 的 done/error 驱动（治断连即免费 #1）。
        let _guard = CancelGuard { state: state.clone(), req_id: req_id.clone() };

        yield Ok::<Event, Infallible>(Event::default().event("message_start").data(json!({
            "type": "message_start",
            "message": {"id": id, "type": "message", "role": "assistant", "model": model,
                        "content": [], "stop_reason": null, "stop_sequence": null,
                        "usage": {"input_tokens": prompt_tokens, "output_tokens": 0}}
        }).to_string()));
        yield Ok(Event::default().event("content_block_start").data(
            json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}).to_string()
        ));

        loop {
            match tokio::time::timeout(gen, rx.recv()).await {
                Ok(Some(StreamEvent::Delta(d))) => {
                    if !d.is_empty() { got_content = true; }
                    acc.push_str(&d);
                    yield Ok(Event::default().event("content_block_delta").data(
                        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": d}}).to_string()
                    ));
                }
                Ok(Some(StreamEvent::Done { conversation_id, full })) => {
                    if !full.trim().is_empty() { got_content = true; if full.len() >= acc.len() { acc = full.clone(); } }
                    if !got_content {
                        state.publish(LiveEvent::RequestError { req_id: id.clone(), code: "empty_response".into() });
                        yield Ok(Event::default().event("error").data(
                            json!({"type": "error", "error": {"type": "api_error", "message": "worker returned empty response"}}).to_string()
                        ));
                        break;
                    }
                    pin.persist(conversation_id).await;
                    let output_tokens = crate::billing::tokens::estimate_tokens(&acc);
                    state.publish(LiveEvent::RequestDone {
                        req_id: id.clone(), status: "stop".into(),
                        tokens: Some(prompt_tokens.saturating_add(output_tokens)),
                        latency_ms: started.elapsed().as_millis() as u64,
                    });
                    yield Ok(Event::default().event("content_block_stop").data(
                        json!({"type": "content_block_stop", "index": 0}).to_string()
                    ));
                    yield Ok(Event::default().event("message_delta").data(
                        json!({"type": "message_delta", "delta": {"stop_reason": "end_turn", "stop_sequence": null}, "usage": {"output_tokens": output_tokens}}).to_string()
                    ));
                    yield Ok(Event::default().event("message_stop").data(
                        json!({"type": "message_stop"}).to_string()
                    ));
                    break;
                }
                Ok(Some(StreamEvent::Error(e))) => {
                    state.publish(LiveEvent::RequestError { req_id: id.clone(), code: "worker_error".into() });
                    yield Ok(Event::default().event("error").data(
                        json!({"type": "error", "error": {"type": "api_error", "message": e}}).to_string()
                    ));
                    break;
                }
                Ok(None) => {
                    state.publish(LiveEvent::RequestError { req_id: id.clone(), code: "disconnected".into() });
                    yield Ok(Event::default().event("error").data(
                        json!({"type": "error", "error": {"type": "api_error", "message": "worker disconnected"}}).to_string()
                    ));
                    break;
                }
                Err(_) => {
                    state.publish(LiveEvent::RequestError { req_id: id.clone(), code: "timeout".into() });
                    yield Ok(Event::default().event("error").data(
                        json!({"type": "error", "error": {"type": "overloaded_error", "message": "generation timeout"}}).to_string()
                    ));
                    break;
                }
            }
        }
    };
    let sse = Sse::new(s).keep_alive(KeepAlive::default());
    ([("x-sub2org-session", session_id)], sse).into_response()
}

/// Anthropic 非流式：返回 message 对象。
#[allow(clippy::too_many_arguments)]
async fn messages_json_response(
    state: Arc<AppState>,
    lease: Lease,
    mut rx: UnboundedReceiver<StreamEvent>,
    req_id: String,
    model: String,
    gen: Duration,
    pin: SessionPin,
    session_id: String,
    rate: RateGuard,
    started: Instant,
    prompt_tokens: u32,
) -> Response {
    let _lease = lease;
    let _rate = rate;
    // 计费不在此结算：charge 在 pending 里，由 gateway 的 done/error 驱动（治断连即免费 #1）。
    let _guard = CancelGuard { state: state.clone(), req_id: req_id.clone() };
    let mut full = String::new();
    loop {
        match tokio::time::timeout(gen, rx.recv()).await {
            Ok(Some(StreamEvent::Delta(d))) => full.push_str(&d),
            Ok(Some(StreamEvent::Done { full: f, conversation_id })) => {
                if !f.is_empty() {
                    full = f;
                }
                if full.trim().is_empty() {
                    state.publish(LiveEvent::RequestError { req_id: req_id.clone(), code: "empty_response".into() });
                    return err_anthropic(StatusCode::BAD_GATEWAY, "api_error", "worker returned empty response");
                }
                pin.persist(conversation_id).await;
                let output_tokens = crate::billing::tokens::estimate_tokens(&full);
                state.publish(LiveEvent::RequestDone {
                    req_id: req_id.clone(), status: "stop".into(),
                    tokens: Some(prompt_tokens.saturating_add(output_tokens)),
                    latency_ms: started.elapsed().as_millis() as u64,
                });
                return (
                    [("x-sub2org-session", session_id)],
                    Json(message_json(&req_id, &model, &full, prompt_tokens, output_tokens)),
                )
                    .into_response();
            }
            Ok(Some(StreamEvent::Error(e))) => {
                state.publish(LiveEvent::RequestError { req_id: req_id.clone(), code: "worker_error".into() });
                return err_anthropic(StatusCode::BAD_GATEWAY, "api_error", &e);
            }
            Ok(None) => {
                state.publish(LiveEvent::RequestError { req_id: req_id.clone(), code: "disconnected".into() });
                return err_anthropic(StatusCode::BAD_GATEWAY, "api_error", "worker disconnected");
            }
            Err(_) => {
                state.publish(LiveEvent::RequestError { req_id: req_id.clone(), code: "timeout".into() });
                return err_anthropic(StatusCode::GATEWAY_TIMEOUT, "overloaded_error", "generation timeout");
            }
        }
    }
}

/// SSRF 守卫（解析 + 逐一校验）；供测试与早期校验用。
#[cfg(test)]
fn ssrf_guard(url: &str) -> Result<(), String> {
    let (host, port) = parse_host_port(url)?;
    resolve_vetted(&host, port).map(|_| ())
}

/// 判定内网/不可路由/特殊用途 IP（黑名单，v6 手写以避开 nightly-only API）。
fn is_blocked_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()       // 169.254/16（含云元数据 169.254.169.254）
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()    // 192.0.2/24, 198.51.100/24, 203.0.113/24
                || o[0] == 0                // 0.0.0.0/8
                || (o[0] == 100 && (o[1] & 0xc0) == 64) // 100.64/10 CGNAT
                || (o[0] == 192 && o[1] == 0 && o[2] == 0) // 192.0.0/24 IETF
                || (o[0] == 198 && (o[1] & 0xfe) == 18)    // 198.18/15 benchmarking
                || o[0] >= 240              // 240/4 reserved（含 255 广播）
        }
        IpAddr::V6(v6) => {
            let s = v6.segments();
            v6.is_loopback()
                || v6.is_unspecified()
                || (s[0] & 0xfe00) == 0xfc00 // fc00::/7 unique-local
                || (s[0] & 0xffc0) == 0xfe80 // fe80::/10 link-local
                || (s[0] == 0x2001 && s[1] == 0x0db8) // 2001:db8::/32 文档段
                || v6.to_ipv4_mapped().map(|m| is_blocked_ip(&IpAddr::V4(m))).unwrap_or(false)
                || nat64_blocked(s) // 64:ff9b::/96 解包内嵌 v4 再判
        }
    }
}

/// NAT64 64:ff9b::/96：解出内嵌 IPv4 后递归判定。
fn nat64_blocked(s: [u16; 8]) -> bool {
    if s[0] == 0x0064 && s[1] == 0xff9b && s[2] == 0 && s[3] == 0 && s[4] == 0 && s[5] == 0 {
        let v4 = Ipv4Addr::new((s[6] >> 8) as u8, (s[6] & 0xff) as u8, (s[7] >> 8) as u8, (s[7] & 0xff) as u8);
        return is_blocked_ip(&IpAddr::V4(v4));
    }
    false
}

#[cfg(test)]
mod image_tests {
    use super::*;
    use crate::config::ImageConfig;
    use std::net::Ipv4Addr;
    use tokio::sync::Semaphore;

    fn user_with_parts(parts: Value) -> Vec<ChatMessage> {
        vec![ChatMessage { role: "user".into(), content: parts }]
    }
    fn sem() -> Semaphore {
        Semaphore::new(4)
    }

    #[tokio::test]
    async fn data_uri_passthrough_and_text_only() {
        let cfg = ImageConfig::default();
        let uri = "data:image/png;base64,aGVsbG8="; // decode "hello"
        let msgs = user_with_parts(json!([
            {"type":"text","text":"what"},
            {"type":"image_url","image_url":{"url": uri}}
        ]));
        assert_eq!(collect_images(&cfg, &sem(), &msgs).await.unwrap(), vec![uri.to_string()]);
        // content 为纯字符串 → 无图
        let txt = user_with_parts(json!("hi"));
        assert!(collect_images(&cfg, &sem(), &txt).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn rejects_oversized_and_too_many() {
        let mut cfg = ImageConfig::default();
        cfg.max_bytes = 8;
        let big = format!("data:image/png;base64,{}", "A".repeat(64)); // ~48B decoded > 8
        assert!(collect_images(&cfg, &sem(), &user_with_parts(json!([{"type":"image_url","image_url":{"url":big}}]))).await.is_err());

        let mut cfg2 = ImageConfig::default();
        cfg2.max_count = 1;
        let uri = "data:image/png;base64,aGVsbG8=";
        let two = user_with_parts(json!([
            {"type":"image_url","image_url":{"url":uri}},
            {"type":"image_url","image_url":{"url":uri}}
        ]));
        assert!(collect_images(&cfg2, &sem(), &two).await.is_err());
    }

    #[tokio::test]
    async fn url_disabled_rejects_http() {
        let mut cfg = ImageConfig::default();
        cfg.fetch_url = false;
        let msgs = user_with_parts(json!([{"type":"image_url","image_url":{"url":"https://example.com/a.png"}}]));
        assert!(collect_images(&cfg, &sem(), &msgs).await.is_err());
    }

    #[test]
    fn ssrf_guard_blocks_internal() {
        for u in [
            "http://127.0.0.1/x",
            "http://10.0.0.5/x",
            "http://192.168.1.1/x",
            "http://169.254.169.254/latest/meta-data", // 云元数据
            "http://[::1]/x",
        ] {
            assert!(ssrf_guard(u).is_err(), "should block {u}");
        }
        // 公网 IP 字面量放行（无需 DNS）
        assert!(ssrf_guard("http://1.1.1.1/x").is_ok());
    }

    #[test]
    fn blocked_ip_classification() {
        assert!(is_blocked_ip(&IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(is_blocked_ip(&IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3))));
        assert!(is_blocked_ip(&IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1))));
        assert!(is_blocked_ip(&IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1)))); // CGNAT
        assert!(is_blocked_ip(&IpAddr::V4(Ipv4Addr::new(198, 18, 0, 1)))); // benchmarking
        assert!(is_blocked_ip(&IpAddr::V4(Ipv4Addr::new(240, 0, 0, 1)))); // reserved
        assert!(!is_blocked_ip(&IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_blocked_ip(&IpAddr::V4(Ipv4Addr::new(100, 128, 0, 1)))); // 100.128 不在 CGNAT
    }
}
