//! 账户存储：users（余额挂用户）+ api_keys（归属用户 + 状态）+ billing_tx + redeem_codes。
//! 两后端：InMemory（dev）/ Postgres。api_key 解析/创建从 store::Store 迁到这里。

mod memory;
mod pg;

pub use memory::InMemoryAccountStore;
pub use pg::PgAccountStore;

use async_trait::async_trait;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct User {
    pub id: String,
    pub username: String,
    #[serde(skip)]
    pub password_hash: String,
    pub email: Option<String>,
    pub role: String,   // admin | user
    pub status: String, // active | banned
    pub balance: i64,
    pub rate_limit: i32,
    pub created_at: i64,
    #[serde(default)]
    pub email_verified: bool,
    #[serde(skip)]
    #[sqlx(default)]
    pub totp_secret: Option<String>, // base32；#[serde(skip)] 永不出 JSON
    #[serde(default)]
    pub totp_enabled: bool,
    #[serde(default)]
    pub token_version: i64, // JWT 主动吊销：bump 即令旧 token 失效
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct ApiKeyRow {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub status: String, // active | disabled
    pub created_at: i64,
    /// B2 分组标签（DB 列 group_tag；序列化为 "group" 给 console）。
    #[sqlx(rename = "group_tag", default)]
    #[serde(default)]
    pub group: String,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct Tx {
    pub id: i64,
    pub user_id: String,
    pub kind: String, // recharge | charge | refund | redeem
    pub amount: i64,
    pub balance_after: i64,
    pub req_id: Option<String>,
    pub model: Option<String>,
    pub note: Option<String>,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct RedeemCode {
    pub code: String,
    pub amount: i64,
    pub created_at: i64,
    pub used_by: Option<String>,
    pub used_at: Option<i64>,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct PaymentOrder {
    pub order_id: String,
    pub user_id: String,
    pub amount: i64,
    pub provider: String,
    pub status: String, // pending | paid
    pub created_at: i64,
    pub paid_at: Option<i64>,
}

/// 用量聚合：某时间区间内按 kind 的金额合计与笔数。
#[derive(Clone, Debug, Serialize)]
pub struct UsageRow {
    pub kind: String,
    pub sum: i64,
    pub count: i64,
}

/// 按用户聚合的消费统计（spent/refunded 为正数）。
#[derive(Clone, Debug, Serialize)]
pub struct UserUsageRow {
    pub user_id: String,
    pub username: String,
    pub spent: i64,    // -SUM(amount where charge)
    pub refunded: i64, // SUM(amount where refund)
    pub count: i64,    // charge 笔数
}

/// 按模型聚合的消费统计（仅 kind=charge 且 model 非空）。
#[derive(Clone, Debug, Serialize)]
pub struct ModelUsageRow {
    pub model: String,
    pub spent: i64, // -SUM(amount)
    pub count: i64,
}

/// 管理操作审计（append-only）。detail 为结构化上下文（JSONB）。
#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct AuditRow {
    pub id: i64,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub detail: serde_json::Value,
    pub created_at: i64,
}

/// 审计查询过滤（None = 不过滤）。
#[derive(Clone, Debug, Default)]
pub struct AuditFilter {
    pub actor: Option<String>,
    pub action: Option<String>,
    pub since: Option<i64>,
    pub limit: i64,
}

/// 消费 API 解析 api_key 的结果（已 join 用户）。
#[derive(Clone, Debug)]
pub struct ResolvedKey {
    pub key_id: String,
    pub user_id: String,
    pub key_status: String,  // active | disabled
    pub user_status: String, // active | banned
    pub rate_limit: i32,
    /// B2 分组标签（"" = 默认池）；consumer 据此把请求只投到同组 worker。
    pub group: String,
}

#[derive(Debug)]
pub enum BalanceErr {
    Insufficient,
    Other(String),
}

#[derive(Debug)]
pub enum RedeemErr {
    NotFound,
    Used,
    Other(String),
}

#[async_trait]
pub trait AccountStore: Send + Sync {
    // ---- users ----
    async fn create_user(
        &self,
        username: &str,
        password_hash: &str,
        role: &str,
        initial_balance: i64,
    ) -> anyhow::Result<String>; // 返回 user_id；用户名重复报错
    async fn get_user(&self, id: &str) -> Option<User>;
    async fn get_user_by_username(&self, username: &str) -> Option<User>;
    async fn list_users(&self) -> Vec<User>;
    async fn set_user_status(&self, id: &str, status: &str) -> anyhow::Result<()>;
    async fn set_user_rate_limit(&self, id: &str, n: i32) -> anyhow::Result<()>;
    async fn count_admins(&self) -> i64;

    // ---- api keys ----
    async fn create_api_key(&self, user_id: &str, name: &str) -> anyhow::Result<(String, String)>; // (raw, id)
    /// 把一个**指定的** raw key 归属到用户（幂等，用于 config 种子 key / bootstrap）。
    async fn seed_api_key(&self, user_id: &str, raw_key: &str, name: &str) -> anyhow::Result<()>;
    async fn resolve_key(&self, raw_key: &str) -> Option<ResolvedKey>;
    async fn list_keys(&self, user_id: &str) -> Vec<ApiKeyRow>;
    async fn key_owner(&self, key_id: &str) -> Option<String>;
    async fn set_key_status(&self, key_id: &str, status: &str) -> anyhow::Result<()>;
    /// B2：设置 api_key 的分组标签（路由隔离）。
    async fn set_key_group(&self, key_id: &str, group: &str) -> anyhow::Result<()>;
    async fn delete_key(&self, key_id: &str) -> anyhow::Result<()>;

    // ---- billing（基于 user.balance）----
    async fn balance(&self, user_id: &str) -> i64;
    /// 原子预扣：成功返回扣后余额并记 charge 流水；余额不足/封禁 → Insufficient。
    async fn reserve(&self, user_id: &str, cost: i64, req_id: &str, model: &str) -> Result<i64, BalanceErr>;
    /// 退还（失败回退）：+amount，记 refund 流水。
    async fn refund(&self, user_id: &str, amount: i64, req_id: &str) -> anyhow::Result<i64>;
    /// 充值（admin）：+amount，记 recharge 流水。
    async fn recharge(&self, user_id: &str, amount: i64, note: &str) -> anyhow::Result<i64>;
    async fn list_tx(&self, user_id: &str, limit: i64) -> Vec<Tx>;

    // ---- redeem ----
    async fn create_redeem_codes(&self, amount: i64, count: usize) -> anyhow::Result<Vec<String>>;
    async fn list_redeem_codes(&self) -> Vec<RedeemCode>;
    /// 原子兑换：标记 code 已用 + 加余额 + 记 redeem 流水。
    async fn redeem(&self, user_id: &str, code: &str) -> Result<i64, RedeemErr>;

    // ---- 账号恢复（邮箱）----
    async fn get_user_by_email(&self, email: &str) -> Option<User>;
    async fn set_email(&self, user_id: &str, email: &str) -> anyhow::Result<()>;
    async fn set_password_hash(&self, user_id: &str, hash: &str) -> anyhow::Result<()>;
    async fn set_email_verified(&self, user_id: &str, verified: bool) -> anyhow::Result<()>;

    // ---- 在线支付订单 ----
    async fn create_payment_order(&self, order_id: &str, user_id: &str, amount: i64, provider: &str) -> anyhow::Result<()>;
    async fn get_payment_order(&self, order_id: &str) -> Option<PaymentOrder>;
    /// 幂等置 paid + 入账：首次返回 Some(扣后余额)；重复回调返回 None（不重复入账）。
    async fn mark_payment_paid(&self, order_id: &str) -> anyhow::Result<Option<i64>>;
    async fn list_payment_orders(&self, user_id: &str, limit: i64) -> Vec<PaymentOrder>;

    // ---- 用量聚合（admin 收入/消耗概览）----
    /// 统计 [since, until]（unix 秒）内按 kind 的金额合计与笔数。
    async fn usage_summary(&self, since: i64, until: i64) -> Vec<UsageRow>;
    /// [since,until] 内按用户聚合的消费 Top N（按 spent 降序）。limit<=0 视为不限。
    async fn usage_by_user(&self, since: i64, until: i64, limit: i64) -> Vec<UserUsageRow>;
    /// [since,until] 内按模型聚合的消费（按 spent 降序）。
    async fn usage_by_model(&self, since: i64, until: i64) -> Vec<ModelUsageRow>;

    // ---- 2FA / TOTP ----
    /// 写入/清空 TOTP secret（base32）。None=清空（disable 用）。
    async fn set_totp_secret(&self, user_id: &str, secret: Option<&str>) -> anyhow::Result<()>;
    async fn set_totp_enabled(&self, user_id: &str, enabled: bool) -> anyhow::Result<()>;

    // ---- JWT 主动吊销 ----
    /// 自增 token 版本（吊销该用户全部已签发 JWT），返回新版本号。
    async fn bump_token_version(&self, user_id: &str) -> anyhow::Result<i64>;

    // ---- 审计（append-only 管理操作日志）----
    async fn append_audit(&self, actor: &str, action: &str, target: &str, detail: serde_json::Value);
    async fn list_audit(&self, filter: AuditFilter) -> Vec<AuditRow>;
}
