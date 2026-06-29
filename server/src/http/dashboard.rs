//! 用户 dashboard 接口：注册/登录（/auth/*）+ JWT 鉴权下的余额/流水/兑换/key 管理（/v1/*）。

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::account::store::RedeemErr;
use crate::account::{auth, email, CurrentUser};
use crate::state::AppState;

fn jerr(code: StatusCode, msg: &str) -> Response {
    (code, Json(json!({"error": {"message": msg, "type": "account_error"}}))).into_response()
}

#[derive(Deserialize)]
pub struct AuthBody {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub email: Option<String>,
    /// B5：Cloudflare Turnstile 校验 token（前端 widget 产生）；未启用验证码时忽略。
    #[serde(default)]
    pub turnstile_token: Option<String>,
}

/// POST /auth/register
pub async fn register(State(state): State<Arc<AppState>>, Json(b): Json<AuthBody>) -> Response {
    if !state.config.allow_registration {
        return jerr(StatusCode::FORBIDDEN, "registration disabled");
    }
    // B5：注册验证码（默认未配置 → 跳过）。
    if let Some(ts) = state.config.turnstile.as_ref() {
        if ts.on_register
            && !crate::account::turnstile::verify(&ts.secret_key, b.turnstile_token.as_deref().unwrap_or(""), None).await
        {
            return jerr(StatusCode::FORBIDDEN, "captcha verification failed");
        }
    }
    if b.username.trim().is_empty() || b.password.len() < 6 {
        return jerr(StatusCode::BAD_REQUEST, "username required and password >= 6 chars");
    }
    let email = b.email.as_deref().map(|e| e.trim()).filter(|e| !e.is_empty());
    match state.account.register(&b.username, &b.password, email).await {
        Ok(id) => {
            // 有邮箱且 SMTP 已配 → 发验证邮件（best-effort，不阻塞注册）。
            if let (Some(em), Some(smtp)) = (email, state.config.smtp.as_ref()) {
                if let Ok(token) = auth::issue_action_token(&state.config.jwt_secret, &id, "verify", 24 * 3600) {
                    let base = state.config.public_base_url.trim_end_matches('/');
                    let link = format!("{base}/auth/verify-email?token={token}");
                    let _ = email::send(smtp, em, "验证你的 sub2org 邮箱", format!("点击验证邮箱：{link}")).await;
                }
            }
            let jwt = state.account.login(&b.username, &b.password).await.ok();
            Json(json!({ "user_id": id, "jwt": jwt })).into_response()
        }
        Err(e) => jerr(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

/// POST /auth/login — 密码校验通过后：未开 2FA 直发 JWT；已开 2FA 返回 challenge_token 走第二步。
pub async fn login(State(state): State<Arc<AppState>>, Json(b): Json<AuthBody>) -> Response {
    if state.account.login_locked(&b.username) {
        return jerr(StatusCode::TOO_MANY_REQUESTS, "登录失败次数过多，请稍后再试");
    }
    // B5：登录验证码（默认 on_login=false → 跳过）。
    if let Some(ts) = state.config.turnstile.as_ref() {
        if ts.on_login
            && !crate::account::turnstile::verify(&ts.secret_key, b.turnstile_token.as_deref().unwrap_or(""), None).await
        {
            return jerr(StatusCode::FORBIDDEN, "captcha verification failed");
        }
    }
    match state.account.password_ok(&b.username, &b.password).await {
        Some(user) => {
            state.account.note_login_result(&b.username, true);
            if user.totp_enabled {
                match auth::issue_action_token(state.account.jwt_secret(), &user.id, "2fa", 300) {
                    Ok(challenge) => Json(json!({ "need_2fa": true, "challenge_token": challenge })).into_response(),
                    Err(e) => jerr(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
                }
            } else {
                match state.account.issue_jwt_for(&user) {
                    Ok(jwt) => Json(json!({ "jwt": jwt })).into_response(),
                    Err(e) => jerr(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
                }
            }
        }
        None => {
            state.account.note_login_result(&b.username, false);
            jerr(StatusCode::UNAUTHORIZED, "invalid credentials")
        }
    }
}

#[derive(Deserialize)]
pub struct Login2faBody {
    #[serde(default)]
    pub challenge_token: String,
    #[serde(default)]
    pub code: String,
}

/// POST /auth/login/2fa — 用 challenge_token + TOTP 码换取 JWT。
pub async fn login_2fa(State(state): State<Arc<AppState>>, Json(b): Json<Login2faBody>) -> Response {
    let uid = match auth::verify_action_token(state.account.jwt_secret(), &b.challenge_token, "2fa") {
        Some(u) => u,
        None => return jerr(StatusCode::UNAUTHORIZED, "challenge expired, please log in again"),
    };
    let user = match state.account.store.get_user(&uid).await {
        Some(u) if u.status == "active" => u,
        _ => return jerr(StatusCode::UNAUTHORIZED, "user unavailable"),
    };
    // 第二因子也走同一账号级锁定，防止 6 位码被暴力枚举（challenge_token 5 分钟可重放）。
    if state.account.login_locked(&user.username) {
        return jerr(StatusCode::TOO_MANY_REQUESTS, "验证失败次数过多，请稍后再试");
    }
    let secret = match user.totp_secret.as_deref() {
        Some(s) if user.totp_enabled => s,
        _ => return jerr(StatusCode::BAD_REQUEST, "2fa not enabled"),
    };
    if !auth::totp_check(secret, &b.code) {
        state.account.note_login_result(&user.username, false);
        return jerr(StatusCode::UNAUTHORIZED, "invalid code");
    }
    state.account.note_login_result(&user.username, true);
    match state.account.issue_jwt_for(&user) {
        Ok(jwt) => Json(json!({ "jwt": jwt })).into_response(),
        Err(e) => jerr(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct ForgotBody {
    #[serde(default)]
    pub email: String,
}

/// POST /auth/forgot — 发送重置链接。无论邮箱是否存在都返回 200（不泄露存在性）。
pub async fn forgot(State(state): State<Arc<AppState>>, Json(b): Json<ForgotBody>) -> Response {
    let smtp = match state.config.smtp.as_ref() {
        Some(s) => s,
        None => return jerr(StatusCode::SERVICE_UNAVAILABLE, "邮件服务未配置"),
    };
    if !b.email.trim().is_empty() {
        if let Some(u) = state.account.store.get_user_by_email(b.email.trim()).await {
            if let Ok(token) = auth::issue_action_token(&state.config.jwt_secret, &u.id, "reset", 3600) {
                let base = state.config.public_base_url.trim_end_matches('/');
                let link = format!("{base}/reset?token={token}");
                let _ = email::send(smtp, b.email.trim(), "重置 sub2org 密码", format!("点击重置密码（1 小时内有效）：{link}")).await;
            }
        }
    }
    Json(json!({ "ok": true })).into_response()
}

#[derive(Deserialize)]
pub struct ResetBody {
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub password: String,
}

/// POST /auth/reset — 用重置 token 设置新密码。
pub async fn reset(State(state): State<Arc<AppState>>, Json(b): Json<ResetBody>) -> Response {
    if b.password.len() < 6 {
        return jerr(StatusCode::BAD_REQUEST, "password >= 6 chars");
    }
    let uid = match auth::verify_action_token(&state.config.jwt_secret, &b.token, "reset") {
        Some(u) => u,
        None => return jerr(StatusCode::BAD_REQUEST, "invalid or expired token"),
    };
    let hash = match auth::hash_password(&b.password) {
        Ok(h) => h,
        Err(e) => return jerr(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };
    let _ = state.account.store.set_password_hash(&uid, &hash).await;
    let _ = state.account.store.bump_token_version(&uid).await; // 改密吊销旧会话
    Json(json!({ "ok": true })).into_response()
}

#[derive(Deserialize)]
pub struct VerifyQuery {
    #[serde(default)]
    pub token: String,
}

/// GET /auth/verify-email?token= — 标记邮箱已验证，跳回控制台登录页。
pub async fn verify_email(State(state): State<Arc<AppState>>, Query(q): Query<VerifyQuery>) -> Response {
    match auth::verify_action_token(&state.config.jwt_secret, &q.token, "verify") {
        Some(uid) => {
            let _ = state.account.store.set_email_verified(&uid, true).await;
            let base = state.config.public_base_url.trim_end_matches('/');
            Redirect::to(&format!("{base}/login?verified=1")).into_response()
        }
        None => jerr(StatusCode::BAD_REQUEST, "invalid or expired token"),
    }
}

/// GET /v1/balance
pub async fn balance(State(state): State<Arc<AppState>>, CurrentUser(u): CurrentUser) -> Json<Value> {
    let recent = state.account.store.list_tx(&u.id, 20).await;
    Json(json!({ "balance": u.balance, "recent_tx": recent }))
}

/// GET /v1/transactions
pub async fn transactions(State(state): State<Arc<AppState>>, CurrentUser(u): CurrentUser) -> Json<Value> {
    let tx = state.account.store.list_tx(&u.id, 200).await;
    Json(json!({ "transactions": tx, "count": tx.len() }))
}

#[derive(Deserialize)]
pub struct RedeemBody {
    #[serde(default)]
    pub code: String,
}

/// POST /v1/redeem
pub async fn redeem(State(state): State<Arc<AppState>>, CurrentUser(u): CurrentUser, Json(b): Json<RedeemBody>) -> Response {
    match state.account.store.redeem(&u.id, &b.code).await {
        Ok(bal) => Json(json!({ "balance": bal })).into_response(),
        Err(RedeemErr::NotFound) => jerr(StatusCode::NOT_FOUND, "redeem code not found"),
        Err(RedeemErr::Used) => jerr(StatusCode::CONFLICT, "redeem code already used"),
        Err(RedeemErr::Other(e)) => jerr(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

/// GET /v1/keys
pub async fn list_keys(State(state): State<Arc<AppState>>, CurrentUser(u): CurrentUser) -> Json<Value> {
    let keys = state.account.store.list_keys(&u.id).await;
    Json(json!({ "keys": keys }))
}

#[derive(Deserialize)]
pub struct CreateKeyBody {
    #[serde(default)]
    pub name: String,
}

/// POST /v1/keys
pub async fn create_key(State(state): State<Arc<AppState>>, CurrentUser(u): CurrentUser, Json(b): Json<CreateKeyBody>) -> Response {
    match state.account.store.create_api_key(&u.id, &b.name).await {
        Ok((raw, id)) => Json(json!({ "id": id, "api_key": raw, "name": b.name })).into_response(),
        Err(e) => jerr(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// 校验 key 归属当前用户。
async fn own_key(state: &Arc<AppState>, user_id: &str, key_id: &str) -> bool {
    state.account.store.key_owner(key_id).await.as_deref() == Some(user_id)
}

/// POST /v1/keys/{id}/disable
pub async fn disable_key(State(state): State<Arc<AppState>>, CurrentUser(u): CurrentUser, Path(id): Path<String>) -> Response {
    if !own_key(&state, &u.id, &id).await {
        return jerr(StatusCode::NOT_FOUND, "key not found");
    }
    let _ = state.account.store.set_key_status(&id, "disabled").await;
    Json(json!({ "id": id, "status": "disabled" })).into_response()
}

/// POST /v1/keys/{id}/enable
pub async fn enable_key(State(state): State<Arc<AppState>>, CurrentUser(u): CurrentUser, Path(id): Path<String>) -> Response {
    if !own_key(&state, &u.id, &id).await {
        return jerr(StatusCode::NOT_FOUND, "key not found");
    }
    let _ = state.account.store.set_key_status(&id, "active").await;
    Json(json!({ "id": id, "status": "active" })).into_response()
}

/// DELETE /v1/keys/{id}
pub async fn delete_key(State(state): State<Arc<AppState>>, CurrentUser(u): CurrentUser, Path(id): Path<String>) -> Response {
    if !own_key(&state, &u.id, &id).await {
        return jerr(StatusCode::NOT_FOUND, "key not found");
    }
    let _ = state.account.store.delete_key(&id).await;
    Json(json!({ "id": id, "deleted": true })).into_response()
}

// ---- 当前用户 / 安全 ----

/// GET /v1/me — 当前用户信息（含 totp_enabled；secret 永不出 JSON）。
pub async fn me(CurrentUser(u): CurrentUser) -> Json<Value> {
    Json(json!({ "user": u }))
}

/// POST /v1/logout-all — 吊销当前用户全部已签发 JWT（本机也需重新登录）。
pub async fn logout_all(State(state): State<Arc<AppState>>, CurrentUser(u): CurrentUser) -> Response {
    match state.account.store.bump_token_version(&u.id).await {
        Ok(v) => Json(json!({ "ok": true, "token_version": v })).into_response(),
        Err(e) => jerr(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct TotpCodeBody {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub password: Option<String>,
}

/// POST /v1/2fa/setup — 生成 provisional secret，返回 otpauth URI + base32；尚未启用。
pub async fn totp_setup(State(state): State<Arc<AppState>>, CurrentUser(u): CurrentUser) -> Response {
    if u.totp_enabled {
        return jerr(StatusCode::CONFLICT, "2fa already enabled");
    }
    let secret = auth::totp_gen_secret();
    if let Err(e) = state.account.store.set_totp_secret(&u.id, Some(&secret)).await {
        return jerr(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string());
    }
    let uri = auth::totp_otpauth_uri(&secret, "sub2org", &u.username);
    Json(json!({ "secret": secret, "otpauth_uri": uri })).into_response()
}

/// POST /v1/2fa/enable {code} — 校验码后正式启用（保留当前会话，需踢其它会话用 /v1/logout-all）。
pub async fn totp_enable(State(state): State<Arc<AppState>>, CurrentUser(u): CurrentUser, Json(b): Json<TotpCodeBody>) -> Response {
    let secret = match u.totp_secret.as_deref() {
        Some(s) if !u.totp_enabled => s,
        Some(_) => return jerr(StatusCode::CONFLICT, "already enabled"),
        None => return jerr(StatusCode::BAD_REQUEST, "run setup first"),
    };
    if !auth::totp_check(secret, &b.code) {
        return jerr(StatusCode::UNAUTHORIZED, "invalid code");
    }
    let _ = state.account.store.set_totp_enabled(&u.id, true).await;
    // 不 bump：用户已认证、刚验过码，保留当前会话（避免自我登出摩擦）；需踢其它会话用 /v1/logout-all。
    Json(json!({ "enabled": true })).into_response()
}

/// POST /v1/2fa/disable {code|password} — 二者其一即可关闭并清空 secret。
pub async fn totp_disable(State(state): State<Arc<AppState>>, CurrentUser(u): CurrentUser, Json(b): Json<TotpCodeBody>) -> Response {
    if !u.totp_enabled {
        return jerr(StatusCode::BAD_REQUEST, "2fa not enabled");
    }
    let ok_code = u.totp_secret.as_deref().map(|s| auth::totp_check(s, &b.code)).unwrap_or(false);
    let ok_pw = b.password.as_deref().map(|p| auth::verify_password(p, &u.password_hash)).unwrap_or(false);
    if !ok_code && !ok_pw {
        return jerr(StatusCode::UNAUTHORIZED, "code or password required");
    }
    let _ = state.account.store.set_totp_enabled(&u.id, false).await;
    let _ = state.account.store.set_totp_secret(&u.id, None).await;
    Json(json!({ "enabled": false })).into_response()
}
