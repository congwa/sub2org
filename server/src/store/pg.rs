//! Postgres store（生产）。sqlx 运行期查询 + 启动时跑 migrations，无需编译期 DB。

use async_trait::async_trait;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use super::{sha256_hex, Session, Store, WorkerRow};

pub struct PgStore {
    pool: PgPool,
    worker_tokens: Vec<String>,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl PgStore {
    pub async fn connect(database_url: &str, worker_tokens: &[String]) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        // 运行全部迁移（含 users/api_keys/billing 的 0004）。api_key 种子化已交给 account 层。
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self {
            pool,
            worker_tokens: worker_tokens.to_vec(),
        })
    }
}

#[async_trait]
impl Store for PgStore {
    async fn api_key_id(&self, raw_key: &str) -> Option<String> {
        let row: Option<(String,)> = sqlx::query_as("SELECT id FROM api_keys WHERE key_hash = $1")
            .bind(sha256_hex(raw_key))
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten();
        row.map(|r| r.0)
    }

    async fn worker_token_ok(&self, token: &str) -> bool {
        // M2：worker 准入 token 仍由 config 提供（worker 注册表 = M3）
        self.worker_tokens.iter().any(|t| t == token)
    }

    async fn create_api_key(&self, owner: &str) -> anyhow::Result<String> {
        let raw = format!("sk-sub2org-{}", Uuid::new_v4().simple());
        let id = Uuid::new_v4().simple().to_string();
        sqlx::query(
            "INSERT INTO api_keys (id, key_hash, owner, created_at) VALUES ($1,$2,$3,$4)",
        )
        .bind(&id)
        .bind(sha256_hex(&raw))
        .bind(owner)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(raw)
    }

    async fn get_session(&self, id: &str) -> Option<Session> {
        sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE session_id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten()
    }

    async fn upsert_session(&self, s: &Session) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO sessions
               (session_id, api_key_id, worker_id, provider, native_conversation_id, created_at, last_used_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7)
             ON CONFLICT (session_id) DO UPDATE SET
               worker_id = EXCLUDED.worker_id,
               provider = EXCLUDED.provider,
               native_conversation_id = COALESCE(EXCLUDED.native_conversation_id, sessions.native_conversation_id),
               last_used_at = EXCLUDED.last_used_at",
        )
        .bind(&s.session_id)
        .bind(&s.api_key_id)
        .bind(&s.worker_id)
        .bind(&s.provider)
        .bind(&s.native_conversation_id)
        .bind(s.created_at)
        .bind(s.last_used_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_sessions(&self) -> Vec<Session> {
        sqlx::query_as::<_, Session>("SELECT * FROM sessions ORDER BY last_used_at DESC LIMIT 500")
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default()
    }

    async fn upsert_worker(&self, worker_id: &str, account_label: &str, provider: &str, group: &str) -> anyhow::Result<()> {
        let n = now();
        sqlx::query(
            "INSERT INTO workers (worker_id, account_label, provider, worker_group, created_at, last_seen_at)
             VALUES ($1,$2,$3,$4,$5,$5)
             ON CONFLICT (worker_id) DO UPDATE SET
               account_label = EXCLUDED.account_label,
               provider = EXCLUDED.provider,
               worker_group = EXCLUDED.worker_group,
               last_seen_at = EXCLUDED.last_seen_at",
        )
        .bind(worker_id)
        .bind(account_label)
        .bind(provider)
        .bind(group)
        .bind(n)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn touch_worker(&self, worker_id: &str, last_seen: i64) -> anyhow::Result<()> {
        sqlx::query("UPDATE workers SET last_seen_at = $2 WHERE worker_id = $1")
            .bind(worker_id)
            .bind(last_seen)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn incr_worker_served(&self, worker_id: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE workers SET total_served = total_served + 1 WHERE worker_id = $1")
            .bind(worker_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn set_worker_disabled(&self, worker_id: &str, disabled: bool) -> anyhow::Result<()> {
        sqlx::query("UPDATE workers SET disabled = $2 WHERE worker_id = $1")
            .bind(worker_id)
            .bind(disabled)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn is_worker_disabled(&self, worker_id: &str) -> bool {
        let row: Option<(bool,)> = sqlx::query_as("SELECT disabled FROM workers WHERE worker_id = $1")
            .bind(worker_id)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten();
        row.map(|r| r.0).unwrap_or(false)
    }

    async fn list_workers(&self) -> Vec<WorkerRow> {
        sqlx::query_as::<_, WorkerRow>("SELECT * FROM workers ORDER BY last_seen_at DESC")
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default()
    }

    async fn ping(&self) -> bool {
        sqlx::query("SELECT 1").execute(&self.pool).await.is_ok()
    }
}
