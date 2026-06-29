//! 持久化抽象：会话亲和钉定、api key、worker 准入。
//! 两种后端：InMemory（dev，无 DB）/ Postgres（生产，sqlx 运行期查询，无需编译期 DB）。

mod memory;
mod pg;

pub use memory::InMemoryStore;
pub use pg::PgStore;

use async_trait::async_trait;
use sha2::{Digest, Sha256};

/// 一个会话：session_id 钉住「某 worker + 某 provider + 网页原生 conversation」。
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Session {
    pub session_id: String,
    pub api_key_id: String,
    pub worker_id: String,
    pub provider: String,
    pub native_conversation_id: Option<String>,
    pub created_at: i64,
    pub last_used_at: i64,
}

/// worker 的持久化身份/统计（在线状态不在这里，在内存 registry）。
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct WorkerRow {
    pub worker_id: String,
    pub account_label: String,
    pub provider: String,
    /// B2 分组标签（列名 worker_group，避开 SQL 保留字；旧 SELECT 缺列时默认 ""）。
    #[sqlx(rename = "worker_group", default)]
    pub group: String,
    pub created_at: i64,
    pub last_seen_at: i64,
    pub total_served: i64,
    pub disabled: bool,
}

#[async_trait]
pub trait Store: Send + Sync {
    /// 校验 raw api key，返回其内部 id（用于关联会话）；None=无效。
    async fn api_key_id(&self, raw_key: &str) -> Option<String>;
    /// 校验 worker 准入 token。
    async fn worker_token_ok(&self, token: &str) -> bool;
    /// 新建一个 api key，返回 raw key（仅此一次可见）。
    async fn create_api_key(&self, owner: &str) -> anyhow::Result<String>;
    /// 取会话。
    async fn get_session(&self, id: &str) -> Option<Session>;
    /// 写入/更新会话（钉定或刷新 native_conversation_id / last_used）。
    async fn upsert_session(&self, s: &Session) -> anyhow::Result<()>;
    /// admin：列出会话。
    async fn list_sessions(&self) -> Vec<Session>;

    // ---- worker 自注册 / 统计（M3）----
    /// worker 首连入库（已存在则更新 label/provider/last_seen）。
    async fn upsert_worker(&self, worker_id: &str, account_label: &str, provider: &str, group: &str) -> anyhow::Result<()>;
    /// 刷新 last_seen。
    async fn touch_worker(&self, worker_id: &str, last_seen: i64) -> anyhow::Result<()>;
    /// 累加服务计数。
    async fn incr_worker_served(&self, worker_id: &str) -> anyhow::Result<()>;
    /// admin 禁用/启用（排空开关）。
    async fn set_worker_disabled(&self, worker_id: &str, disabled: bool) -> anyhow::Result<()>;
    /// 握手时判断是否被禁用。
    async fn is_worker_disabled(&self, worker_id: &str) -> bool;
    /// admin：列出所有已知 worker（含离线）。
    async fn list_workers(&self) -> Vec<WorkerRow>;

    /// 就绪探针：默认 true；Postgres 后端 ping 数据库。
    async fn ping(&self) -> bool {
        true
    }
}

pub fn sha256_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}
