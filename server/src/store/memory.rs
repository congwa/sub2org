//! 内存 store（dev / 无 DB）。api key 与 worker token 从 config 种子化。

use async_trait::async_trait;
use dashmap::DashMap;
use uuid::Uuid;

use std::time::{SystemTime, UNIX_EPOCH};

use super::{sha256_hex, Session, Store, WorkerRow};

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub struct InMemoryStore {
    /// key_hash → api_key_id
    api_keys: DashMap<String, String>,
    worker_tokens: Vec<String>,
    sessions: DashMap<String, Session>,
    workers: DashMap<String, WorkerRow>,
}

impl InMemoryStore {
    pub fn from_config(worker_tokens: &[String]) -> Self {
        // api_key 种子化已交给 account 层；这里只保留 worker_tokens 与中转状态。
        Self {
            api_keys: DashMap::new(),
            worker_tokens: worker_tokens.to_vec(),
            sessions: DashMap::new(),
            workers: DashMap::new(),
        }
    }
}

#[async_trait]
impl Store for InMemoryStore {
    async fn api_key_id(&self, raw_key: &str) -> Option<String> {
        self.api_keys.get(&sha256_hex(raw_key)).map(|v| v.clone())
    }

    async fn worker_token_ok(&self, token: &str) -> bool {
        self.worker_tokens.iter().any(|t| t == token)
    }

    async fn create_api_key(&self, _owner: &str) -> anyhow::Result<String> {
        let raw = format!("sk-sub2org-{}", Uuid::new_v4().simple());
        let id = Uuid::new_v4().simple().to_string();
        self.api_keys.insert(sha256_hex(&raw), id);
        Ok(raw)
    }

    async fn get_session(&self, id: &str) -> Option<Session> {
        self.sessions.get(id).map(|v| v.clone())
    }

    async fn upsert_session(&self, s: &Session) -> anyhow::Result<()> {
        self.sessions.insert(s.session_id.clone(), s.clone());
        Ok(())
    }

    async fn list_sessions(&self) -> Vec<Session> {
        self.sessions.iter().map(|e| e.value().clone()).collect()
    }

    async fn upsert_worker(&self, worker_id: &str, account_label: &str, provider: &str, group: &str) -> anyhow::Result<()> {
        let n = now();
        self.workers
            .entry(worker_id.to_string())
            .and_modify(|w| {
                w.account_label = account_label.to_string();
                w.provider = provider.to_string();
                w.group = group.to_string();
                w.last_seen_at = n;
            })
            .or_insert_with(|| WorkerRow {
                worker_id: worker_id.to_string(),
                account_label: account_label.to_string(),
                provider: provider.to_string(),
                group: group.to_string(),
                created_at: n,
                last_seen_at: n,
                total_served: 0,
                disabled: false,
            });
        Ok(())
    }

    async fn touch_worker(&self, worker_id: &str, last_seen: i64) -> anyhow::Result<()> {
        if let Some(mut w) = self.workers.get_mut(worker_id) {
            w.last_seen_at = last_seen;
        }
        Ok(())
    }

    async fn incr_worker_served(&self, worker_id: &str) -> anyhow::Result<()> {
        if let Some(mut w) = self.workers.get_mut(worker_id) {
            w.total_served += 1;
        }
        Ok(())
    }

    async fn set_worker_disabled(&self, worker_id: &str, disabled: bool) -> anyhow::Result<()> {
        if let Some(mut w) = self.workers.get_mut(worker_id) {
            w.disabled = disabled;
        }
        Ok(())
    }

    async fn is_worker_disabled(&self, worker_id: &str) -> bool {
        self.workers.get(worker_id).map(|w| w.disabled).unwrap_or(false)
    }

    async fn list_workers(&self) -> Vec<WorkerRow> {
        self.workers.iter().map(|e| e.value().clone()).collect()
    }
}
