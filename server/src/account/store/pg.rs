//! Postgres 账户存储。余额变更用事务保证 balance 与流水一致。

use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use crate::store::sha256_hex;

use super::{
    AccountStore, ApiKeyRow, AuditFilter, AuditRow, BalanceErr, ModelUsageRow, PaymentOrder,
    RedeemCode, RedeemErr, ResolvedKey, Tx, UsageRow, User, UserUsageRow,
};

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn gen_code() -> String {
    use rand::Rng;
    const CH: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut r = rand::thread_rng();
    let s: String = (0..12).map(|_| CH[r.gen_range(0..CH.len())] as char).collect();
    format!("SUB2ORG-{}-{}", &s[0..6], &s[6..12])
}

pub struct PgAccountStore {
    pool: PgPool,
}

impl PgAccountStore {
    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new().max_connections(8).connect(database_url).await?;
        Ok(Self { pool })
    }

    async fn insert_tx(
        ex: impl sqlx::PgExecutor<'_>,
        user_id: &str,
        kind: &str,
        amount: i64,
        balance_after: i64,
        req_id: Option<&str>,
        model: Option<&str>,
        note: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO billing_tx (user_id, kind, amount, balance_after, req_id, model, note, created_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        )
        .bind(user_id)
        .bind(kind)
        .bind(amount)
        .bind(balance_after)
        .bind(req_id)
        .bind(model)
        .bind(note)
        .bind(now())
        .execute(ex)
        .await?;
        Ok(())
    }
}

#[async_trait]
impl AccountStore for PgAccountStore {
    async fn create_user(&self, username: &str, password_hash: &str, role: &str, initial_balance: i64) -> anyhow::Result<String> {
        if self.get_user_by_username(username).await.is_some() {
            anyhow::bail!("username already exists");
        }
        let id = Uuid::new_v4().simple().to_string();
        sqlx::query(
            "INSERT INTO users (id, username, password_hash, role, status, balance, rate_limit, created_at)
             VALUES ($1,$2,$3,$4,'active',$5,0,$6)",
        )
        .bind(&id)
        .bind(username)
        .bind(password_hash)
        .bind(role)
        .bind(initial_balance)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    async fn get_user(&self, id: &str) -> Option<User> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE id=$1").bind(id).fetch_optional(&self.pool).await.ok().flatten()
    }
    async fn get_user_by_username(&self, username: &str) -> Option<User> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE username=$1").bind(username).fetch_optional(&self.pool).await.ok().flatten()
    }
    async fn list_users(&self) -> Vec<User> {
        sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY created_at DESC").fetch_all(&self.pool).await.unwrap_or_default()
    }
    async fn set_user_status(&self, id: &str, status: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE users SET status=$2 WHERE id=$1").bind(id).bind(status).execute(&self.pool).await?;
        Ok(())
    }
    async fn set_user_rate_limit(&self, id: &str, n: i32) -> anyhow::Result<()> {
        sqlx::query("UPDATE users SET rate_limit=$2 WHERE id=$1").bind(id).bind(n).execute(&self.pool).await?;
        Ok(())
    }
    async fn count_admins(&self) -> i64 {
        let r: Option<(i64,)> = sqlx::query_as("SELECT count(*) FROM users WHERE role='admin'").fetch_optional(&self.pool).await.ok().flatten();
        r.map(|x| x.0).unwrap_or(0)
    }

    async fn create_api_key(&self, user_id: &str, name: &str) -> anyhow::Result<(String, String)> {
        let raw = format!("sk-sub2org-{}", Uuid::new_v4().simple());
        let id = Uuid::new_v4().simple().to_string();
        sqlx::query(
            "INSERT INTO api_keys (id, key_hash, user_id, name, status, created_at)
             VALUES ($1,$2,$3,$4,'active',$5)",
        )
        .bind(&id)
        .bind(sha256_hex(&raw))
        .bind(user_id)
        .bind(name)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok((raw, id))
    }

    async fn seed_api_key(&self, user_id: &str, raw_key: &str, name: &str) -> anyhow::Result<()> {
        let id = Uuid::new_v4().simple().to_string();
        sqlx::query(
            "INSERT INTO api_keys (id, key_hash, user_id, name, status, created_at)
             VALUES ($1,$2,$3,$4,'active',$5)
             ON CONFLICT (key_hash) DO UPDATE SET user_id = EXCLUDED.user_id",
        )
        .bind(id)
        .bind(sha256_hex(raw_key))
        .bind(user_id)
        .bind(name)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn resolve_key(&self, raw_key: &str) -> Option<ResolvedKey> {
        let row: Option<(String, String, String, String, i32, String)> = sqlx::query_as(
            "SELECT k.id, k.user_id, k.status, u.status, u.rate_limit, k.group_tag
             FROM api_keys k JOIN users u ON u.id = k.user_id
             WHERE k.key_hash = $1",
        )
        .bind(sha256_hex(raw_key))
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten();
        row.map(|(key_id, user_id, key_status, user_status, rate_limit, group)| ResolvedKey {
            key_id,
            user_id,
            key_status,
            user_status,
            rate_limit,
            group,
        })
    }

    async fn list_keys(&self, user_id: &str) -> Vec<ApiKeyRow> {
        sqlx::query_as::<_, ApiKeyRow>(
            "SELECT id, user_id, name, status, created_at, group_tag FROM api_keys WHERE user_id=$1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
    }
    async fn key_owner(&self, key_id: &str) -> Option<String> {
        let r: Option<(Option<String>,)> = sqlx::query_as("SELECT user_id FROM api_keys WHERE id=$1").bind(key_id).fetch_optional(&self.pool).await.ok().flatten();
        r.and_then(|x| x.0)
    }
    async fn set_key_status(&self, key_id: &str, status: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE api_keys SET status=$2 WHERE id=$1").bind(key_id).bind(status).execute(&self.pool).await?;
        Ok(())
    }
    async fn set_key_group(&self, key_id: &str, group: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE api_keys SET group_tag=$2 WHERE id=$1").bind(key_id).bind(group).execute(&self.pool).await?;
        Ok(())
    }
    async fn delete_key(&self, key_id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM api_keys WHERE id=$1").bind(key_id).execute(&self.pool).await?;
        Ok(())
    }

    async fn balance(&self, user_id: &str) -> i64 {
        let r: Option<(i64,)> = sqlx::query_as("SELECT balance FROM users WHERE id=$1").bind(user_id).fetch_optional(&self.pool).await.ok().flatten();
        r.map(|x| x.0).unwrap_or(0)
    }

    async fn reserve(&self, user_id: &str, cost: i64, req_id: &str, model: &str) -> Result<i64, BalanceErr> {
        let mut t = self.pool.begin().await.map_err(|e| BalanceErr::Other(e.to_string()))?;
        let row: Option<(i64,)> = sqlx::query_as(
            "UPDATE users SET balance = balance - $2 WHERE id=$1 AND status='active' AND balance >= $2 RETURNING balance",
        )
        .bind(user_id)
        .bind(cost)
        .fetch_optional(&mut *t)
        .await
        .map_err(|e| BalanceErr::Other(e.to_string()))?;
        let bal = match row {
            Some(r) => r.0,
            None => {
                let _ = t.rollback().await;
                return Err(BalanceErr::Insufficient);
            }
        };
        Self::insert_tx(&mut *t, user_id, "charge", -cost, bal, Some(req_id), Some(model), None)
            .await
            .map_err(|e| BalanceErr::Other(e.to_string()))?;
        t.commit().await.map_err(|e| BalanceErr::Other(e.to_string()))?;
        Ok(bal)
    }

    async fn refund(&self, user_id: &str, amount: i64, req_id: &str) -> anyhow::Result<i64> {
        let mut t = self.pool.begin().await?;
        let (bal,): (i64,) = sqlx::query_as("UPDATE users SET balance = balance + $2 WHERE id=$1 RETURNING balance")
            .bind(user_id).bind(amount).fetch_one(&mut *t).await?;
        Self::insert_tx(&mut *t, user_id, "refund", amount, bal, Some(req_id), None, None).await?;
        t.commit().await?;
        Ok(bal)
    }

    async fn recharge(&self, user_id: &str, amount: i64, note: &str) -> anyhow::Result<i64> {
        let mut t = self.pool.begin().await?;
        let (bal,): (i64,) = sqlx::query_as("UPDATE users SET balance = balance + $2 WHERE id=$1 RETURNING balance")
            .bind(user_id).bind(amount).fetch_one(&mut *t).await?;
        Self::insert_tx(&mut *t, user_id, "recharge", amount, bal, None, None, Some(note)).await?;
        t.commit().await?;
        Ok(bal)
    }

    async fn list_tx(&self, user_id: &str, limit: i64) -> Vec<Tx> {
        sqlx::query_as::<_, Tx>("SELECT * FROM billing_tx WHERE user_id=$1 ORDER BY id DESC LIMIT $2")
            .bind(user_id).bind(limit).fetch_all(&self.pool).await.unwrap_or_default()
    }

    async fn create_redeem_codes(&self, amount: i64, count: usize) -> anyhow::Result<Vec<String>> {
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            let code = gen_code();
            sqlx::query("INSERT INTO redeem_codes (code, amount, created_at) VALUES ($1,$2,$3)")
                .bind(&code).bind(amount).bind(now()).execute(&self.pool).await?;
            out.push(code);
        }
        Ok(out)
    }
    async fn list_redeem_codes(&self) -> Vec<RedeemCode> {
        sqlx::query_as::<_, RedeemCode>("SELECT * FROM redeem_codes ORDER BY created_at DESC LIMIT 500").fetch_all(&self.pool).await.unwrap_or_default()
    }
    async fn redeem(&self, user_id: &str, code: &str) -> Result<i64, RedeemErr> {
        let mut t = self.pool.begin().await.map_err(|e| RedeemErr::Other(e.to_string()))?;
        let row: Option<(i64,)> = sqlx::query_as(
            "UPDATE redeem_codes SET used_by=$1, used_at=$2 WHERE code=$3 AND used_by IS NULL RETURNING amount",
        )
        .bind(user_id).bind(now()).bind(code)
        .fetch_optional(&mut *t).await.map_err(|e| RedeemErr::Other(e.to_string()))?;
        let amount = match row {
            Some(r) => r.0,
            None => {
                let _ = t.rollback().await;
                // 区分不存在 vs 已用
                let exists: Option<(String,)> = sqlx::query_as("SELECT code FROM redeem_codes WHERE code=$1").bind(code).fetch_optional(&self.pool).await.ok().flatten();
                return Err(if exists.is_some() { RedeemErr::Used } else { RedeemErr::NotFound });
            }
        };
        let (bal,): (i64,) = sqlx::query_as("UPDATE users SET balance = balance + $2 WHERE id=$1 RETURNING balance")
            .bind(user_id).bind(amount).fetch_one(&mut *t).await.map_err(|e| RedeemErr::Other(e.to_string()))?;
        Self::insert_tx(&mut *t, user_id, "redeem", amount, bal, None, None, Some(code)).await.map_err(|e| RedeemErr::Other(e.to_string()))?;
        t.commit().await.map_err(|e| RedeemErr::Other(e.to_string()))?;
        Ok(bal)
    }

    // ---- 账号恢复（邮箱）----
    async fn get_user_by_email(&self, email: &str) -> Option<User> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE email=$1").bind(email).fetch_optional(&self.pool).await.ok().flatten()
    }
    async fn set_email(&self, user_id: &str, email: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE users SET email=$2 WHERE id=$1").bind(user_id).bind(email).execute(&self.pool).await?;
        Ok(())
    }
    async fn set_password_hash(&self, user_id: &str, hash: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE users SET password_hash=$2 WHERE id=$1").bind(user_id).bind(hash).execute(&self.pool).await?;
        Ok(())
    }
    async fn set_email_verified(&self, user_id: &str, verified: bool) -> anyhow::Result<()> {
        sqlx::query("UPDATE users SET email_verified=$2 WHERE id=$1").bind(user_id).bind(verified).execute(&self.pool).await?;
        Ok(())
    }

    // ---- 在线支付订单 ----
    async fn create_payment_order(&self, order_id: &str, user_id: &str, amount: i64, provider: &str) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO payment_orders (order_id, user_id, amount, provider, status, created_at)
             VALUES ($1,$2,$3,$4,'pending',$5)",
        )
        .bind(order_id).bind(user_id).bind(amount).bind(provider).bind(now())
        .execute(&self.pool).await?;
        Ok(())
    }
    async fn get_payment_order(&self, order_id: &str) -> Option<PaymentOrder> {
        sqlx::query_as::<_, PaymentOrder>("SELECT * FROM payment_orders WHERE order_id=$1").bind(order_id).fetch_optional(&self.pool).await.ok().flatten()
    }
    async fn mark_payment_paid(&self, order_id: &str) -> anyhow::Result<Option<i64>> {
        let mut t = self.pool.begin().await?;
        // 仅 pending → paid 才入账（幂等）。
        let row: Option<(String, i64)> = sqlx::query_as(
            "UPDATE payment_orders SET status='paid', paid_at=$2 WHERE order_id=$1 AND status='pending' RETURNING user_id, amount",
        )
        .bind(order_id).bind(now())
        .fetch_optional(&mut *t).await?;
        let (user_id, amount) = match row {
            Some(r) => r,
            None => {
                let _ = t.rollback().await;
                return Ok(None);
            }
        };
        let (bal,): (i64,) = sqlx::query_as("UPDATE users SET balance = balance + $2 WHERE id=$1 RETURNING balance")
            .bind(&user_id).bind(amount).fetch_one(&mut *t).await?;
        Self::insert_tx(&mut *t, &user_id, "recharge", amount, bal, None, None, Some(&format!("epay:{order_id}"))).await?;
        t.commit().await?;
        Ok(Some(bal))
    }
    async fn list_payment_orders(&self, user_id: &str, limit: i64) -> Vec<PaymentOrder> {
        sqlx::query_as::<_, PaymentOrder>("SELECT * FROM payment_orders WHERE user_id=$1 ORDER BY created_at DESC LIMIT $2")
            .bind(user_id).bind(limit).fetch_all(&self.pool).await.unwrap_or_default()
    }

    async fn usage_summary(&self, since: i64, until: i64) -> Vec<UsageRow> {
        let rows: Vec<(String, i64, i64)> = sqlx::query_as(
            "SELECT kind, COALESCE(SUM(amount),0)::BIGINT, COUNT(*)::BIGINT FROM billing_tx WHERE created_at BETWEEN $1 AND $2 GROUP BY kind",
        )
        .bind(since).bind(until)
        .fetch_all(&self.pool).await.unwrap_or_default();
        rows.into_iter().map(|(kind, sum, count)| UsageRow { kind, sum, count }).collect()
    }

    async fn usage_by_user(&self, since: i64, until: i64, limit: i64) -> Vec<UserUsageRow> {
        let lim = if limit > 0 { limit } else { 1000 };
        let rows: Vec<(String, Option<String>, i64, i64, i64)> = sqlx::query_as(
            "SELECT t.user_id,
                    u.username,
                    COALESCE(-SUM(CASE WHEN t.kind='charge' THEN t.amount ELSE 0 END),0)::BIGINT AS spent,
                    COALESCE( SUM(CASE WHEN t.kind='refund' THEN t.amount ELSE 0 END),0)::BIGINT AS refunded,
                    COALESCE( SUM(CASE WHEN t.kind='charge' THEN 1 ELSE 0 END),0)::BIGINT AS cnt
             FROM billing_tx t
             LEFT JOIN users u ON u.id = t.user_id
             WHERE t.created_at BETWEEN $1 AND $2
             GROUP BY t.user_id, u.username
             HAVING SUM(CASE WHEN t.kind='charge' THEN 1 ELSE 0 END) > 0
             ORDER BY spent DESC
             LIMIT $3",
        )
        .bind(since).bind(until).bind(lim)
        .fetch_all(&self.pool).await.unwrap_or_default();
        rows.into_iter()
            .map(|(user_id, username, spent, refunded, count)| UserUsageRow {
                user_id,
                username: username.unwrap_or_default(),
                spent,
                refunded,
                count,
            })
            .collect()
    }

    async fn usage_by_model(&self, since: i64, until: i64) -> Vec<ModelUsageRow> {
        let rows: Vec<(String, i64, i64)> = sqlx::query_as(
            "SELECT model,
                    COALESCE(-SUM(amount),0)::BIGINT AS spent,
                    COUNT(*)::BIGINT AS cnt
             FROM billing_tx
             WHERE kind='charge' AND model IS NOT NULL AND created_at BETWEEN $1 AND $2
             GROUP BY model
             ORDER BY spent DESC",
        )
        .bind(since).bind(until)
        .fetch_all(&self.pool).await.unwrap_or_default();
        rows.into_iter()
            .map(|(model, spent, count)| ModelUsageRow { model, spent, count })
            .collect()
    }

    async fn set_totp_secret(&self, user_id: &str, secret: Option<&str>) -> anyhow::Result<()> {
        sqlx::query("UPDATE users SET totp_secret=$2 WHERE id=$1").bind(user_id).bind(secret).execute(&self.pool).await?;
        Ok(())
    }
    async fn set_totp_enabled(&self, user_id: &str, enabled: bool) -> anyhow::Result<()> {
        sqlx::query("UPDATE users SET totp_enabled=$2 WHERE id=$1").bind(user_id).bind(enabled).execute(&self.pool).await?;
        Ok(())
    }
    async fn bump_token_version(&self, user_id: &str) -> anyhow::Result<i64> {
        let (v,): (i64,) = sqlx::query_as(
            "UPDATE users SET token_version = token_version + 1 WHERE id=$1 RETURNING token_version",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(v)
    }
    async fn append_audit(&self, actor: &str, action: &str, target: &str, detail: serde_json::Value) {
        let _ = sqlx::query(
            "INSERT INTO audit_log (actor, action, target, detail, created_at) VALUES ($1,$2,$3,$4,$5)",
        )
        .bind(actor)
        .bind(action)
        .bind(target)
        .bind(detail)
        .bind(now())
        .execute(&self.pool)
        .await;
    }
    async fn list_audit(&self, f: AuditFilter) -> Vec<AuditRow> {
        let lim = if f.limit > 0 { f.limit } else { 200 };
        sqlx::query_as::<_, AuditRow>(
            "SELECT id, actor, action, target, detail, created_at FROM audit_log
             WHERE ($1::text IS NULL OR actor = $1)
               AND ($2::text IS NULL OR action = $2)
               AND ($3::bigint IS NULL OR created_at >= $3)
             ORDER BY id DESC LIMIT $4",
        )
        .bind(f.actor)
        .bind(f.action)
        .bind(f.since)
        .bind(lim)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
    }
}
