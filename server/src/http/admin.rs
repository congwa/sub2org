//! 运维接口（M3）：worker 在线/统计（合并内存+DB）、禁用/启用、会话查询、签发 api key。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::account::AdminGuard;
use crate::logging::{level_rank, LogQuery};
use crate::state::AppState;
use crate::store::Session;

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 记一条管理审计（fire-and-forget；失败不阻断业务）。
async fn audit(state: &Arc<AppState>, admin: &AdminGuard, action: &str, target: &str, detail: Value) {
    state.account.store.append_audit(&admin.actor, action, target, detail).await;
}

/// GET /admin/workers — 合并内存实时态 + DB 已知 worker（含离线），一眼看全所有机器。
pub async fn list_workers(_admin: AdminGuard, State(state): State<Arc<AppState>>) -> Json<Value> {
    let now = now_secs();
    let live = state.registry.snapshot(now);
    let live_ids: HashMap<String, ()> =
        live.iter().map(|w| (w.worker_id.clone(), ())).collect();

    let mut out: Vec<Value> = live
        .iter()
        .map(|w| {
            json!({
                "worker_id": w.worker_id,
                "provider": w.provider,
                "group": w.group,
                "account_label": w.account_label,
                "online": true,
                "status": w.status,
                "in_flight": w.in_flight,
                "free": w.free,
                "capacity": w.capacity,
                "cooldown_until": w.cooldown_until,
                "last_seen_at": w.last_seen,
                "total_served": w.total_served,
            })
        })
        .collect();

    // DB 里已知但当前不在线的机器
    for row in state.store.list_workers().await {
        if live_ids.contains_key(&row.worker_id) {
            continue;
        }
        out.push(json!({
            "worker_id": row.worker_id,
            "provider": row.provider,
            "group": row.group,
            "account_label": row.account_label,
            "online": false,
            "status": if row.disabled { "disabled" } else { "offline" },
            "in_flight": 0,
            "free": 0,
            "capacity": 1,
            "cooldown_until": 0,
            "last_seen_at": row.last_seen_at,
            "total_served": row.total_served,
            "disabled": row.disabled,
        }));
    }

    let count = out.len();
    Json(json!({ "workers": out, "count": count }))
}

/// POST /admin/workers/:id/disable
pub async fn disable_worker(
    admin: AdminGuard,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let _ = state.store.set_worker_disabled(&id, true).await;
    state.registry.set_disabled(&id, true);
    state.publish(crate::metrics::LiveEvent::WorkerStatus {
        worker_id: id.clone(),
        status: "disabled".to_string(),
    });
    audit(&state, &admin, "disable_worker", &id, json!({})).await;
    Json(json!({ "worker_id": id, "disabled": true })).into_response()
}

/// POST /admin/workers/:id/enable
pub async fn enable_worker(
    admin: AdminGuard,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let _ = state.store.set_worker_disabled(&id, false).await;
    state.registry.set_disabled(&id, false);
    state.registry.reset_breaker(&id); // B4：手动启用同时清熔断
    if let Some(s) = state.registry.status_of(&id, now_secs()) {
        state.publish(crate::metrics::LiveEvent::WorkerStatus {
            worker_id: id.clone(),
            status: s.to_string(),
        });
    }
    audit(&state, &admin, "enable_worker", &id, json!({})).await;
    Json(json!({ "worker_id": id, "disabled": false })).into_response()
}

fn session_json(s: &Session) -> Value {
    json!({
        "session_id": s.session_id,
        "api_key_id": s.api_key_id,
        "worker_id": s.worker_id,
        "provider": s.provider,
        "native_conversation_id": s.native_conversation_id,
        "created_at": s.created_at,
        "last_used_at": s.last_used_at,
    })
}

/// GET /admin/sessions
pub async fn list_sessions(_admin: AdminGuard, State(state): State<Arc<AppState>>) -> Json<Value> {
    let sessions: Vec<Value> = state
        .store
        .list_sessions()
        .await
        .iter()
        .map(session_json)
        .collect();
    Json(json!({ "sessions": sessions, "count": sessions.len() }))
}

/// GET /admin/sessions/:id
pub async fn get_session(_admin: AdminGuard, State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    match state.store.get_session(&id).await {
        Some(s) => Json(session_json(&s)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": {"message": "session not found"}})),
        )
            .into_response(),
    }
}

fn user_json(u: &crate::account::User) -> Value {
    json!({
        "id": u.id, "username": u.username, "role": u.role, "status": u.status,
        "balance": u.balance, "rate_limit": u.rate_limit, "created_at": u.created_at,
    })
}

/// GET /admin/users
pub async fn list_users(_admin: AdminGuard, State(state): State<Arc<AppState>>) -> Json<Value> {
    let users: Vec<Value> = state.account.store.list_users().await.iter().map(user_json).collect();
    Json(json!({ "users": users, "count": users.len() }))
}

/// GET /admin/users/:id
pub async fn get_user(_admin: AdminGuard, State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    match state.account.store.get_user(&id).await {
        Some(u) => Json(user_json(&u)).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({"error": {"message": "user not found"}}))).into_response(),
    }
}

#[derive(Deserialize)]
pub struct CreateUserBody {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub initial_balance: Option<i64>,
}

/// POST /admin/users
pub async fn create_user(admin: AdminGuard, State(state): State<Arc<AppState>>, Json(b): Json<CreateUserBody>) -> Response {
    let role = b.role.as_deref().unwrap_or("user");
    let hash = match crate::account::auth::hash_password(&b.password) {
        Ok(h) => h,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": {"message": e.to_string()}}))).into_response(),
    };
    let initial = b.initial_balance.unwrap_or(state.config.default_initial_balance);
    match state.account.store.create_user(&b.username, &hash, role, initial).await {
        Ok(id) => {
            audit(&state, &admin, "create_user", &id, json!({ "username": b.username, "role": role })).await;
            Json(json!({ "id": id, "username": b.username, "role": role })).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"error": {"message": e.to_string()}}))).into_response(),
    }
}

/// POST /admin/users/:id/ban
pub async fn ban_user(admin: AdminGuard, State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    let _ = state.account.store.set_user_status(&id, "banned").await;
    let _ = state.account.store.bump_token_version(&id).await; // 吊销其全部 JWT
    audit(&state, &admin, "ban_user", &id, json!({})).await;
    Json(json!({ "id": id, "status": "banned" })).into_response()
}

/// POST /admin/users/:id/unban
pub async fn unban_user(admin: AdminGuard, State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    let _ = state.account.store.set_user_status(&id, "active").await;
    audit(&state, &admin, "unban_user", &id, json!({})).await;
    Json(json!({ "id": id, "status": "active" })).into_response()
}

#[derive(Deserialize)]
pub struct RechargeBody {
    pub amount: i64,
    #[serde(default)]
    pub note: String,
}

/// POST /admin/users/:id/recharge
pub async fn recharge_user(admin: AdminGuard, State(state): State<Arc<AppState>>, Path(id): Path<String>, Json(b): Json<RechargeBody>) -> Response {
    match state.account.store.recharge(&id, b.amount, &b.note).await {
        Ok(bal) => {
            audit(&state, &admin, "recharge", &id, json!({ "amount": b.amount, "note": b.note, "balance": bal })).await;
            Json(json!({ "id": id, "balance": bal })).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"error": {"message": e.to_string()}}))).into_response(),
    }
}

#[derive(Deserialize)]
pub struct RateLimitBody {
    pub n: i32,
}

/// POST /admin/users/:id/rate-limit
pub async fn set_rate_limit(admin: AdminGuard, State(state): State<Arc<AppState>>, Path(id): Path<String>, Json(b): Json<RateLimitBody>) -> Response {
    let _ = state.account.store.set_user_rate_limit(&id, b.n).await;
    audit(&state, &admin, "set_rate_limit", &id, json!({ "n": b.n })).await;
    Json(json!({ "id": id, "rate_limit": b.n })).into_response()
}

/// GET /admin/users/:id/keys
pub async fn list_user_keys(_admin: AdminGuard, State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    let keys = state.account.store.list_keys(&id).await;
    Json(json!({ "keys": keys }))
}

#[derive(Deserialize)]
pub struct AdminCreateKeyBody {
    #[serde(default)]
    pub name: String,
}

/// POST /admin/users/:id/keys
pub async fn create_user_key(admin: AdminGuard, State(state): State<Arc<AppState>>, Path(id): Path<String>, Json(b): Json<AdminCreateKeyBody>) -> Response {
    match state.account.store.create_api_key(&id, &b.name).await {
        Ok((raw, kid)) => {
            audit(&state, &admin, "create_key", &id, json!({ "key_id": kid })).await; // 不记原始 key
            Json(json!({ "id": kid, "api_key": raw, "user_id": id })).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": {"message": e.to_string()}}))).into_response(),
    }
}

/// POST /admin/api-keys/:id/disable
pub async fn admin_disable_key(admin: AdminGuard, State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    let _ = state.account.store.set_key_status(&id, "disabled").await;
    audit(&state, &admin, "disable_key", &id, json!({})).await;
    Json(json!({ "id": id, "status": "disabled" })).into_response()
}

#[derive(serde::Deserialize)]
pub struct KeyGroupBody {
    #[serde(default)]
    pub group: String,
}

/// B2：设置 api_key 的分组标签（路由隔离）。
pub async fn set_key_group(
    admin: AdminGuard,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(b): Json<KeyGroupBody>,
) -> Response {
    let _ = state.account.store.set_key_group(&id, b.group.trim()).await;
    audit(&state, &admin, "set_key_group", &id, json!({ "group": b.group })).await;
    Json(json!({ "id": id, "group": b.group.trim() })).into_response()
}

/// POST /admin/api-keys/:id/enable
pub async fn admin_enable_key(admin: AdminGuard, State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    let _ = state.account.store.set_key_status(&id, "active").await;
    audit(&state, &admin, "enable_key", &id, json!({})).await;
    Json(json!({ "id": id, "status": "active" })).into_response()
}

#[derive(Deserialize)]
pub struct CreateCodesBody {
    pub amount: i64,
    #[serde(default = "one")]
    pub count: usize,
}
fn one() -> usize {
    1
}

/// POST /admin/redeem-codes
pub async fn create_redeem_codes(admin: AdminGuard, State(state): State<Arc<AppState>>, Json(b): Json<CreateCodesBody>) -> Response {
    match state.account.store.create_redeem_codes(b.amount, b.count.clamp(1, 1000)).await {
        Ok(codes) => {
            audit(&state, &admin, "create_redeem_codes", "", json!({ "amount": b.amount, "count": codes.len() })).await; // 不记 code 值
            Json(json!({ "codes": codes, "amount": b.amount })).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": {"message": e.to_string()}}))).into_response(),
    }
}

/// GET /admin/redeem-codes
pub async fn list_redeem_codes(_admin: AdminGuard, State(state): State<Arc<AppState>>) -> Json<Value> {
    let codes = state.account.store.list_redeem_codes().await;
    Json(json!({ "codes": codes, "count": codes.len() }))
}

#[derive(Debug, Deserialize)]
pub struct LogQueryParams {
    pub source: Option<String>,
    pub worker_id: Option<String>,
    pub category: Option<String>,
    pub level: Option<String>,
    pub req_id: Option<String>,
    pub session_id: Option<String>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// GET /admin/logs — 按 端/类型/级别/时间/worker/req 过滤查询日志。
pub async fn list_logs(
    _admin: AdminGuard,
    State(state): State<Arc<AppState>>,
    Query(p): Query<LogQueryParams>,
) -> Json<Value> {
    let q = LogQuery {
        source: p.source,
        worker_id: p.worker_id,
        category: p.category,
        min_level: p.level.as_deref().map(level_rank),
        req_id: p.req_id,
        session_id: p.session_id,
        since_ms: p.since,
        until_ms: p.until,
        limit: p.limit.unwrap_or(200).clamp(1, 1000),
        offset: p.offset.unwrap_or(0).max(0),
    };
    let logs = state.log_store.query(&q).await;
    Json(json!({
        "logs": logs,
        "count": logs.len(),
        "dropped": state.log_sink.dropped(),
    }))
}

#[derive(Debug, Deserialize)]
pub struct UsageQuery {
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub group_by: Option<String>, // "user" | "model" | 缺省(=按 kind 概览)
    pub limit: Option<i64>,       // group_by=user 时的 Top N，默认 20
}

/// GET /admin/stats/usage — 区间内收入/消耗（默认近 24h）。
/// `?group_by=user|model` 给出 Top 用户 / 按模型聚合；缺省给按 kind 概览。
pub async fn usage_stats(_admin: AdminGuard, State(state): State<Arc<AppState>>, Query(q): Query<UsageQuery>) -> Json<Value> {
    let until = q.until.unwrap_or_else(now_secs);
    let since = q.since.unwrap_or(until - 86_400);

    match q.group_by.as_deref() {
        Some("user") => {
            let limit = q.limit.unwrap_or(20).clamp(1, 500);
            let rows = state.account.store.usage_by_user(since, until, limit).await;
            let by_user: Vec<Value> = rows
                .iter()
                .map(|r| json!({
                    "user_id": r.user_id, "username": r.username,
                    "spent": r.spent, "refunded": r.refunded, "count": r.count,
                }))
                .collect();
            return Json(json!({ "since": since, "until": until, "by_user": by_user }));
        }
        Some("model") => {
            let rows = state.account.store.usage_by_model(since, until).await;
            let by_model: Vec<Value> = rows
                .iter()
                .map(|r| json!({ "model": r.model, "spent": r.spent, "count": r.count }))
                .collect();
            return Json(json!({ "since": since, "until": until, "by_model": by_model }));
        }
        _ => {}
    }

    let rows = state.account.store.usage_summary(since, until).await;
    let mut charge = 0i64; // 负数
    let mut refund = 0i64;
    let mut recharge = 0i64;
    let mut redeem = 0i64;
    let by_kind: Vec<Value> = rows
        .iter()
        .map(|r| {
            match r.kind.as_str() {
                "charge" => charge = r.sum,
                "refund" => refund = r.sum,
                "recharge" => recharge = r.sum,
                "redeem" => redeem = r.sum,
                _ => {}
            }
            json!({ "kind": r.kind, "sum": r.sum, "count": r.count })
        })
        .collect();
    Json(json!({
        "since": since,
        "until": until,
        "by_kind": by_kind,
        "spent_credits": -charge,                 // 用户被扣总额（正数）
        "refunded_credits": refund,
        "recharged_credits": recharge,
        "redeemed_credits": redeem,
        "net_consumed_credits": -charge - refund, // 净消耗 = 扣费 - 退款
    }))
}

#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    pub actor: Option<String>,
    pub action: Option<String>,
    pub since: Option<i64>,
    pub limit: Option<i64>,
}

/// GET /admin/audit — 管理操作审计（AdminGuard；读端不记审计）。
pub async fn list_audit(_admin: AdminGuard, State(state): State<Arc<AppState>>, Query(q): Query<AuditQuery>) -> Json<Value> {
    let rows = state
        .account
        .store
        .list_audit(crate::account::store::AuditFilter {
            actor: q.actor.filter(|s| !s.is_empty()),
            action: q.action.filter(|s| !s.is_empty()),
            since: q.since,
            limit: q.limit.unwrap_or(200).clamp(1, 1000),
        })
        .await;
    let count = rows.len();
    Json(json!({ "audit": rows, "count": count }))
}

/// POST /admin/users/:id/2fa/reset — 清除用户 2FA（丢设备救援；记审计）。
pub async fn reset_2fa(admin: AdminGuard, State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    let _ = state.account.store.set_totp_enabled(&id, false).await;
    let _ = state.account.store.set_totp_secret(&id, None).await;
    audit(&state, &admin, "reset_2fa", &id, json!({})).await;
    Json(json!({ "id": id, "totp_enabled": false })).into_response()
}

/// GET /healthz
pub async fn healthz() -> &'static str {
    "ok"
}

/// GET /livez — 进程存活探针（容器/反代用）。
pub async fn livez() -> &'static str {
    "ok"
}

/// GET /readyz — 就绪探针：Postgres 模式会 ping DB。
pub async fn readyz(State(state): State<Arc<AppState>>) -> Response {
    if state.store.ping().await {
        (StatusCode::OK, "ready").into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "db unavailable").into_response()
    }
}
