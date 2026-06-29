//! 消费方鉴权（M1 明文比对 config.api_keys；后续移到 Postgres 哈希）。

use axum::http::HeaderMap;

/// 从 `Authorization: Bearer xxx` 取出 key；回退 `x-api-key`（A1：Anthropic SDK 用该头）。
pub fn bearer(headers: &HeaderMap) -> Option<String> {
    if let Some(v) = headers.get(axum::http::header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        if let Some(token) = v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer ")) {
            return Some(token.trim().to_string());
        }
    }
    // 回退：Anthropic 客户端常发 x-api-key（无 Bearer 前缀）。
    headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
