//! 鉴权：argon2 密码哈希、JWT 签发/校验，以及 Axum 提取器 CurrentUser / AdminGuard。

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{header::AUTHORIZATION, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::account::store::User;
use crate::state::AppState;

// ---- 密码哈希 ----

pub fn hash_password(pw: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(pw.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| anyhow::anyhow!("hash failed: {e}"))
}

pub fn verify_password(pw: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default().verify_password(pw.as_bytes(), &parsed).is_ok(),
        Err(_) => false,
    }
}

/// P1#4：用户不存在时也跑一次真实 argon2 校验（对固定 dummy hash），抹平
/// 「用户不存在(快) vs 密码错(慢)」的时序差，防用户名枚举。结果丢弃。
pub fn verify_dummy(pw: &str) {
    static DUMMY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let h = DUMMY.get_or_init(|| hash_password("sub2org-enumeration-guard").unwrap_or_default());
    let _ = verify_password(pw, h);
}

// ---- JWT ----

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user_id
    pub role: String,
    pub exp: usize,
    /// token 版本（主动吊销）。#[serde(default)] 让旧 token 解码为 0，与初始 token_version=0 匹配。
    #[serde(default)]
    pub tv: i64,
}

const JWT_TTL_SECS: usize = 7 * 24 * 3600;

pub fn issue_jwt(secret: &str, user_id: &str, role: &str, token_version: i64) -> anyhow::Result<String> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as usize).unwrap_or(0);
    let claims = Claims {
        sub: user_id.to_string(),
        role: role.to_string(),
        exp: now + JWT_TTL_SECS,
        tv: token_version,
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .map_err(|e| anyhow::anyhow!("jwt encode: {e}"))
}

pub fn verify_jwt(secret: &str, token: &str) -> Option<Claims> {
    decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &Validation::default())
        .ok()
        .map(|d| d.claims)
}

// ---- 一次性动作 token（找回密码 / 邮箱验证）----

#[derive(Debug, Serialize, Deserialize)]
pub struct ActionClaims {
    pub sub: String,     // user_id
    pub purpose: String, // "reset" | "verify"
    pub exp: usize,
}

/// 签发动作 token（复用 jwt_secret，短 TTL）。
pub fn issue_action_token(secret: &str, user_id: &str, purpose: &str, ttl_secs: usize) -> anyhow::Result<String> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as usize).unwrap_or(0);
    let claims = ActionClaims {
        sub: user_id.to_string(),
        purpose: purpose.to_string(),
        exp: now + ttl_secs,
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .map_err(|e| anyhow::anyhow!("action token encode: {e}"))
}

/// 校验动作 token 并要求 purpose 一致，返回 user_id。
pub fn verify_action_token(secret: &str, token: &str, expected_purpose: &str) -> Option<String> {
    let claims = decode::<ActionClaims>(token, &DecodingKey::from_secret(secret.as_bytes()), &Validation::default())
        .ok()
        .map(|d| d.claims)?;
    if claims.purpose == expected_purpose {
        Some(claims.sub)
    } else {
        None
    }
}

fn bearer(parts: &Parts) -> Option<String> {
    let v = parts.headers.get(AUTHORIZATION)?.to_str().ok()?;
    let t = v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer "))?;
    Some(t.trim().to_string())
}

fn deny(code: StatusCode, msg: &str) -> Response {
    (code, Json(json!({"error": {"message": msg, "type": "auth_error"}}))).into_response()
}

// ---- 提取器 ----

/// 用户 dashboard：要求合法 JWT 且用户未封禁。
pub struct CurrentUser(pub User);

impl FromRequestParts<Arc<AppState>> for CurrentUser {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppState>) -> Result<Self, Self::Rejection> {
        let token = bearer(parts).ok_or_else(|| deny(StatusCode::UNAUTHORIZED, "missing bearer token"))?;
        let claims = verify_jwt(&state.config.jwt_secret, &token)
            .ok_or_else(|| deny(StatusCode::UNAUTHORIZED, "invalid or expired token"))?;
        let user = state
            .account
            .store
            .get_user(&claims.sub)
            .await
            .ok_or_else(|| deny(StatusCode::UNAUTHORIZED, "user not found"))?;
        if claims.tv != user.token_version {
            return Err(deny(StatusCode::UNAUTHORIZED, "token revoked"));
        }
        if user.status != "active" {
            return Err(deny(StatusCode::FORBIDDEN, "user banned"));
        }
        Ok(CurrentUser(user))
    }
}

/// admin：X-Admin-Token 命中 config，或 admin 角色 JWT；config 未配 admin_token 则放行（内网）。
/// 携带 actor 身份供审计日志记录。
pub struct AdminGuard {
    pub actor: String,
}

impl FromRequestParts<Arc<AppState>> for AdminGuard {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppState>) -> Result<Self, Self::Rejection> {
        let cfg = &state.config;
        // P0#2：admin_token 为空不再无条件放行；仅 allow_open_admin=true（内网/dev）才免鉴权直连。
        // 否则继续走下面的 admin 角色 JWT（控制台登录），都不过则拒。
        if cfg.admin_token.is_empty() {
            if cfg.allow_open_admin {
                return Ok(AdminGuard { actor: "internal".into() });
            }
        } else if let Some(t) = parts.headers.get("x-admin-token").and_then(|v| v.to_str().ok()) {
            // P1#4：常量时间比较，防计时枚举。
            if crate::config::ct_eq(t.as_bytes(), cfg.admin_token.as_bytes()) {
                return Ok(AdminGuard { actor: "token".into() });
            }
        }
        // admin 角色 JWT：校验 token 版本（吊销）+ 状态，并以用户名作为 actor。
        if let Some(tok) = bearer(parts) {
            if let Some(c) = verify_jwt(&cfg.jwt_secret, &tok) {
                if c.role == "admin" {
                    if let Some(u) = state.account.store.get_user(&c.sub).await {
                        if c.tv == u.token_version && u.status == "active" {
                            return Ok(AdminGuard { actor: format!("jwt:{}", u.username) });
                        }
                    }
                }
            }
        }
        Err(deny(StatusCode::UNAUTHORIZED, "admin auth required"))
    }
}

// ---- TOTP（两步验证）----

use totp_rs::{Algorithm, Secret, TOTP};

const TOTP_STEP: u64 = 30;
const TOTP_DIGITS: usize = 6;
const TOTP_SKEW: u8 = 1; // ±1 步 = ±30s 容差

/// 生成新 secret（base32，存库用）。
pub fn totp_gen_secret() -> String {
    Secret::generate_secret().to_encoded().to_string()
}

/// 用 base32 secret 构造 TOTP 实例。
fn totp_from_b32(secret_b32: &str) -> Option<TOTP> {
    let bytes = Secret::Encoded(secret_b32.to_string()).to_bytes().ok()?;
    TOTP::new(Algorithm::SHA1, TOTP_DIGITS, TOTP_SKEW, TOTP_STEP, bytes).ok()
}

/// 校验 6 位码（按当前系统时间，含 skew 窗口）。
pub fn totp_check(secret_b32: &str, code: &str) -> bool {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    totp_from_b32(secret_b32).map(|t| t.check(code.trim(), now)).unwrap_or(false)
}

/// RFC3986 component 编码（只保留 unreserved；其余百分号转义）。防止用户名中的 `?&#=:/` 破坏 URI。
fn pct(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => o.push(b as char),
            _ => o.push_str(&format!("%{b:02X}")),
        }
    }
    o
}

/// 构造 otpauth:// URI（手动拼，避免 otpauth 特性引入 url/idna）。
pub fn totp_otpauth_uri(secret_b32: &str, issuer: &str, account: &str) -> String {
    let (iss, acc) = (pct(issuer), pct(account));
    format!(
        "otpauth://totp/{iss}:{acc}?secret={secret_b32}&issuer={iss}&algorithm=SHA1&digits={TOTP_DIGITS}&period={TOTP_STEP}"
    )
}
