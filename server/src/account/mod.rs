//! 账户体系门面：用户/鉴权/计费/限流，封装给 consumer / admin / dashboard 调用。

pub mod auth;
pub mod email;
pub mod ratelimit;
pub mod store;
pub mod turnstile;

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use dashmap::DashMap;

pub use auth::{AdminGuard, CurrentUser};
pub use ratelimit::RateGuard;
pub use store::{AccountStore, ApiKeyRow, BalanceErr, RedeemCode, RedeemErr, ResolvedKey, Tx, User};

use crate::config::Config;
use ratelimit::RateLimiter;
use store::InMemoryAccountStore;

// 登录爆破锁定参数。
const LOGIN_WINDOW_SECS: i64 = 600;
const LOGIN_FAIL_THRESHOLD: u32 = 10;
const LOGIN_LOCK_SECS: i64 = 300;

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// 一次请求的预扣句柄：成功 `commit`，失败 `refund`（预扣+失败退还）。
pub struct Charge {
    store: Arc<dyn AccountStore>,
    user_id: String,
    cost: i64,
    req_id: String,
    done: bool,
}

impl Charge {
    /// 成功：余额已在 reserve 时扣除，这里只标记完成。
    pub fn commit(&mut self) {
        self.done = true;
    }
    /// 失败：把预扣的额度退还。
    pub async fn refund(&mut self) {
        if !self.done {
            let _ = self.store.refund(&self.user_id, self.cost, &self.req_id).await;
            self.done = true;
        }
    }
}
impl Drop for Charge {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        // P0#3：未结算即 drop——典型是 HTTP 客户端中途断开 → axum drop 掉 async_stream → 本句柄随之 drop。
        // 自动退款，避免只扣不退。Drop 是同步、refund 是异步 → spawn 到当前 runtime（无 runtime 时仅告警）。
        tracing::error!(category = "billing", req_id = %self.req_id, "charge 未结算即 drop，自动退款 {} credits", self.cost);
        let store = self.store.clone();
        let user_id = self.user_id.clone();
        let req_id = self.req_id.clone();
        let cost = self.cost;
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let _ = store.refund(&user_id, cost, &req_id).await;
            });
        }
    }
}

pub struct AccountService {
    pub store: Arc<dyn AccountStore>,
    config: Arc<Config>,
    ratelimit: RateLimiter,
    /// 登录失败计数：username -> (窗口起点, 次数, 锁定到).
    login_fails: DashMap<String, (i64, u32, i64)>,
}

impl AccountService {
    pub fn new(store: Arc<dyn AccountStore>, config: Arc<Config>) -> Self {
        Self {
            store,
            config,
            ratelimit: RateLimiter::new(),
            login_fails: DashMap::new(),
        }
    }

    /// 该用户名是否处于登录锁定中。
    pub fn login_locked(&self, username: &str) -> bool {
        self.login_fails.get(username).map(|e| e.2 > now_secs()).unwrap_or(false)
    }

    /// 记录一次登录结果：成功清零；失败累加，超阈值临时锁定。
    pub fn note_login_result(&self, username: &str, ok: bool) {
        if ok {
            self.login_fails.remove(username);
            return;
        }
        let now = now_secs();
        let mut e = self.login_fails.entry(username.to_string()).or_insert((now, 0, 0));
        if now - e.0 > LOGIN_WINDOW_SECS {
            e.0 = now;
            e.1 = 0;
        }
        e.1 += 1;
        if e.1 >= LOGIN_FAIL_THRESHOLD {
            e.2 = now + LOGIN_LOCK_SECS;
        }
    }

    pub fn jwt_secret(&self) -> &str {
        &self.config.jwt_secret
    }
    pub fn require_email_verify(&self) -> bool {
        self.config.require_email_verify
    }

    pub fn memory(config: Arc<Config>) -> Self {
        Self::new(Arc::new(InMemoryAccountStore::new()), config)
    }

    // ---- 计费 / 限流 ----
    pub fn billing_enabled(&self) -> bool {
        self.config.billing_enabled
    }
    pub fn cost_for_model(&self, model: &str) -> i64 {
        self.config.cost_for_model(model)
    }
    /// B3：最终扣费 = 基础价 × model倍率 × 全局倍率 × key倍率（v1 key_mult 恒 1.0）。四舍五入、≥0。
    pub fn cost_for(&self, model: &str, key_mult: f64) -> i64 {
        let base = self.config.cost_for_model(model) as f64;
        let m = self.config.model_multiplier_for(model) * self.config.cost_multiplier * key_mult;
        (base * m).round().max(0.0) as i64
    }
    pub fn try_rate(&self, user_id: &str, limit: i32) -> Option<RateGuard> {
        self.ratelimit.try_acquire(user_id, limit)
    }

    /// 原子预扣，返回 (Charge 句柄, 扣后余额)。Charge 随请求流生命周期 commit/refund。
    pub async fn reserve_charge(
        &self,
        user_id: &str,
        cost: i64,
        req_id: &str,
        model: &str,
    ) -> Result<(Charge, i64), BalanceErr> {
        let balance_after = self.store.reserve(user_id, cost, req_id, model).await?;
        Ok((
            Charge {
                store: self.store.clone(),
                user_id: user_id.to_string(),
                cost,
                req_id: req_id.to_string(),
                done: false,
            },
            balance_after,
        ))
    }

    /// 启用告警时的余额低阈值（用于 reserve 后判断是否发 balance_low）。
    pub fn balance_low_threshold(&self) -> Option<i64> {
        self.config.alert.as_ref().filter(|a| a.enabled).map(|a| a.balance_low_threshold)
    }

    // ---- 鉴权 / 用户 ----
    pub async fn register(&self, username: &str, password: &str, email: Option<&str>) -> anyhow::Result<String> {
        let hash = auth::hash_password(password)?;
        let id = self
            .store
            .create_user(username, &hash, "user", self.config.default_initial_balance)
            .await?;
        if let Some(e) = email {
            if !e.is_empty() {
                let _ = self.store.set_email(&id, e).await;
            }
        }
        Ok(id)
    }

    /// 用户名密码登录，返回 JWT（供 register 自动登录；2FA 用户走 handler 的两步流程）。
    pub async fn login(&self, username: &str, password: &str) -> anyhow::Result<String> {
        let user = match self.store.get_user_by_username(username).await {
            Some(u) => u,
            None => {
                auth::verify_dummy(password); // P1#4：抹平时序，防用户名枚举
                anyhow::bail!("invalid credentials");
            }
        };
        if !auth::verify_password(password, &user.password_hash) {
            anyhow::bail!("invalid credentials");
        }
        if user.status != "active" {
            anyhow::bail!("user banned");
        }
        self.issue_jwt_for(&user)
    }

    /// 仅校验密码 + 状态，返回 User（不签发 JWT）。供 2FA 两步登录用。
    pub async fn password_ok(&self, username: &str, password: &str) -> Option<User> {
        let user = match self.store.get_user_by_username(username).await {
            Some(u) => u,
            None => {
                auth::verify_dummy(password); // P1#4：抹平时序，防用户名枚举
                return None;
            }
        };
        if !auth::verify_password(password, &user.password_hash) {
            return None;
        }
        if user.status != "active" {
            return None;
        }
        Some(user)
    }

    /// 按用户当前 token_version 签发 JWT。
    pub fn issue_jwt_for(&self, u: &User) -> anyhow::Result<String> {
        auth::issue_jwt(&self.config.jwt_secret, &u.id, &u.role, u.token_version)
    }

    /// 首启 bootstrap：无 admin 则按 config 建一个，返回 (是否新建, 用户名, 明文密码或空)。
    pub async fn ensure_admin(&self) -> anyhow::Result<Option<(String, String)>> {
        if self.store.count_admins().await > 0 {
            return Ok(None);
        }
        let username = if self.config.admin_username.is_empty() {
            "admin".to_string()
        } else {
            self.config.admin_username.clone()
        };
        let (password, generated) = if self.config.admin_password.is_empty() {
            (gen_password(), true)
        } else {
            (self.config.admin_password.clone(), false)
        };
        let hash = auth::hash_password(&password)?;
        self.store.create_user(&username, &hash, "admin", 0).await?;
        Ok(if generated {
            Some((username, password))
        } else {
            Some((username, String::new()))
        })
    }
}

fn gen_password() -> String {
    use rand::Rng;
    const CH: &[u8] = b"abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut r = rand::thread_rng();
    (0..16).map(|_| CH[r.gen_range(0..CH.len())] as char).collect()
}
