//! worker 规模化预配：`POST /worker/claim`。
//! 扩展用共享 `worker_token` 领取一份唯一 `worker_id` + `account_label`，免逐台手填。
//! 无状态：身份只在 socket 首连时由 `upsert_worker` 落库；这里只负责免碰撞铸造。

use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::state::AppState;
use crate::store::WorkerRow;

#[derive(Debug, Deserialize)]
pub struct ClaimReq {
    /// 共享准入 token（也可走 header `Authorization: Bearer` 或 `X-Worker-Token`）。
    #[serde(default)]
    pub worker_token: Option<String>,
    /// 期望的 provider（chatgpt/claude）；缺省用 config.worker.default_provider。
    #[serde(default)]
    pub provider: Option<String>,
    /// 客户端建议的标签；缺省自动生成 wk-NNN。
    #[serde(default)]
    pub account_label: Option<String>,
    /// 幂等重领：带上已分配的 worker_id → 原样返回该身份。
    #[serde(default)]
    pub worker_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ClaimResp {
    pub worker_id: String,
    pub account_label: String,
    pub provider: String,
    pub server_base_url: String,
}

/// 取 token：body.worker_token 优先，否则 header `Authorization: Bearer <t>` / `X-Worker-Token`。
fn extract_token(headers: &HeaderMap, body: &ClaimReq) -> Option<String> {
    if let Some(t) = body.worker_token.as_deref() {
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    if let Some(v) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        if let Some(rest) = v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer ")) {
            let t = rest.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    if let Some(v) = headers.get("x-worker-token").and_then(|v| v.to_str().ok()) {
        let t = v.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    None
}

/// 在已有标签中找下一个 `<prefix>-NNN`（取现有最大序号 +1，3 位补零）。
/// 并发下标签可能撞，但注册键是 worker_id（永唯一 uuid），标签仅装饰。
pub fn next_label(existing: &[WorkerRow], prefix: &str) -> String {
    let pat = format!("{prefix}-");
    let max = existing
        .iter()
        .filter_map(|w| w.account_label.strip_prefix(&pat))
        .filter_map(|n| n.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    format!("{prefix}-{:03}", max + 1)
}

fn err(status: StatusCode, msg: &str, kind: &str) -> Response {
    (status, Json(json!({"error": {"message": msg, "type": kind}}))).into_response()
}

/// POST /worker/claim — 自校 worker_token，铸造/返回唯一 worker 身份。
pub async fn claim_worker(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<ClaimReq>,
) -> Response {
    let wcfg = &state.config.worker;
    if !wcfg.provision_enabled {
        return err(StatusCode::FORBIDDEN, "worker 预配未启用", "provision_disabled");
    }
    let token = match extract_token(&headers, &body) {
        Some(t) => t,
        None => return err(StatusCode::UNAUTHORIZED, "缺少 worker_token", "auth_error"),
    };
    if !state.config.worker_token_ok(&token) {
        return err(StatusCode::UNAUTHORIZED, "worker_token 无效", "auth_error");
    }

    let workers = state.store.list_workers().await;

    // 幂等重领：已知 worker_id 原样返回。
    if let Some(id) = body.worker_id.as_deref().filter(|s| !s.is_empty()) {
        if let Some(row) = workers.iter().find(|w| w.worker_id == id) {
            return Json(ClaimResp {
                worker_id: row.worker_id.clone(),
                account_label: row.account_label.clone(),
                provider: row.provider.clone(),
                server_base_url: state.config.public_base_url.clone(),
            })
            .into_response();
        }
    }

    let provider = body
        .provider
        .as_deref()
        .map(|p| p.to_ascii_lowercase())
        .filter(|p| p == "chatgpt" || p == "claude")
        .unwrap_or_else(|| wcfg.default_provider.clone());

    let account_label = match body.account_label.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(l) => l.to_string(),
        None => next_label(&workers, &wcfg.label_prefix),
    };

    let worker_id = format!("{}-{}", wcfg.label_prefix, Uuid::new_v4().simple());

    tracing::info!(
        category = "connection",
        worker_id = %worker_id,
        provider = %provider,
        "worker claimed identity"
    );

    Json(ClaimResp {
        worker_id,
        account_label,
        provider,
        server_base_url: state.config.public_base_url.clone(),
    })
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, label: &str) -> WorkerRow {
        WorkerRow {
            worker_id: id.to_string(),
            account_label: label.to_string(),
            provider: "chatgpt".to_string(),
            group: String::new(),
            created_at: 0,
            last_seen_at: 0,
            total_served: 0,
            disabled: false,
        }
    }

    #[test]
    fn next_label_sequences() {
        let existing = vec![row("a", "wk-001"), row("b", "wk-002")];
        assert_eq!(next_label(&existing, "wk"), "wk-003");
    }

    #[test]
    fn next_label_from_empty_and_gaps() {
        assert_eq!(next_label(&[], "wk"), "wk-001");
        // 取最大值 +1，忽略非数字 / 不同前缀
        let existing = vec![row("a", "wk-005"), row("b", "cg-099"), row("c", "wk-abc")];
        assert_eq!(next_label(&existing, "wk"), "wk-006");
    }

    #[test]
    fn extract_token_prefers_body_then_headers() {
        let mut h = HeaderMap::new();
        h.insert("authorization", "Bearer hdr-tok".parse().unwrap());
        let body = ClaimReq {
            worker_token: Some("body-tok".into()),
            provider: None,
            account_label: None,
            worker_id: None,
        };
        assert_eq!(extract_token(&h, &body).as_deref(), Some("body-tok"));

        let body2 = ClaimReq { worker_token: None, provider: None, account_label: None, worker_id: None };
        assert_eq!(extract_token(&h, &body2).as_deref(), Some("hdr-tok"));

        let mut h2 = HeaderMap::new();
        h2.insert("x-worker-token", "xwt".parse().unwrap());
        assert_eq!(extract_token(&h2, &body2).as_deref(), Some("xwt"));

        assert_eq!(extract_token(&HeaderMap::new(), &body2), None);
    }
}
