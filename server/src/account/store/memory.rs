//! 内存账户存储（dev / 无 DB）。原子性靠 DashMap 单 entry 独占。

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use dashmap::DashMap;
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

#[derive(Default)]
pub struct InMemoryAccountStore {
    users: DashMap<String, User>,
    username_idx: DashMap<String, String>, // username -> id
    keys: DashMap<String, ApiKeyRow>,      // key_id -> row
    key_hash_idx: DashMap<String, String>, // key_hash -> key_id
    txs: Mutex<Vec<Tx>>,
    tx_id: AtomicI64,
    codes: DashMap<String, RedeemCode>,
    orders: DashMap<String, PaymentOrder>,
    audit: Mutex<Vec<AuditRow>>,
    audit_id: AtomicI64,
}

impl InMemoryAccountStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn push_tx(&self, user_id: &str, kind: &str, amount: i64, balance_after: i64, req_id: Option<&str>, model: Option<&str>, note: Option<&str>) {
        let id = self.tx_id.fetch_add(1, Ordering::SeqCst) + 1;
        self.txs.lock().unwrap().push(Tx {
            id,
            user_id: user_id.to_string(),
            kind: kind.to_string(),
            amount,
            balance_after,
            req_id: req_id.map(|s| s.to_string()),
            model: model.map(|s| s.to_string()),
            note: note.map(|s| s.to_string()),
            created_at: now(),
        });
    }
}

#[async_trait]
impl AccountStore for InMemoryAccountStore {
    async fn create_user(&self, username: &str, password_hash: &str, role: &str, initial_balance: i64) -> anyhow::Result<String> {
        if self.username_idx.contains_key(username) {
            anyhow::bail!("username already exists");
        }
        let id = Uuid::new_v4().simple().to_string();
        let user = User {
            id: id.clone(),
            username: username.to_string(),
            password_hash: password_hash.to_string(),
            email: None,
            role: role.to_string(),
            status: "active".to_string(),
            balance: initial_balance,
            rate_limit: 0,
            created_at: now(),
            email_verified: false,
            totp_secret: None,
            totp_enabled: false,
            token_version: 0,
        };
        self.username_idx.insert(username.to_string(), id.clone());
        self.users.insert(id.clone(), user);
        Ok(id)
    }

    async fn get_user(&self, id: &str) -> Option<User> {
        self.users.get(id).map(|u| u.clone())
    }
    async fn get_user_by_username(&self, username: &str) -> Option<User> {
        let id = self.username_idx.get(username)?.clone();
        self.users.get(&id).map(|u| u.clone())
    }
    async fn list_users(&self) -> Vec<User> {
        self.users.iter().map(|e| e.value().clone()).collect()
    }
    async fn set_user_status(&self, id: &str, status: &str) -> anyhow::Result<()> {
        if let Some(mut u) = self.users.get_mut(id) {
            u.status = status.to_string();
        }
        Ok(())
    }
    async fn set_user_rate_limit(&self, id: &str, n: i32) -> anyhow::Result<()> {
        if let Some(mut u) = self.users.get_mut(id) {
            u.rate_limit = n;
        }
        Ok(())
    }
    async fn count_admins(&self) -> i64 {
        self.users.iter().filter(|e| e.value().role == "admin").count() as i64
    }

    async fn create_api_key(&self, user_id: &str, name: &str) -> anyhow::Result<(String, String)> {
        let raw = format!("sk-sub2org-{}", Uuid::new_v4().simple());
        let id = Uuid::new_v4().simple().to_string();
        let row = ApiKeyRow {
            id: id.clone(),
            user_id: user_id.to_string(),
            name: name.to_string(),
            status: "active".to_string(),
            created_at: now(),
            group: String::new(),
        };
        self.key_hash_idx.insert(sha256_hex(&raw), id.clone());
        self.keys.insert(id.clone(), row);
        Ok((raw, id))
    }

    async fn seed_api_key(&self, user_id: &str, raw_key: &str, name: &str) -> anyhow::Result<()> {
        let hash = sha256_hex(raw_key);
        if self.key_hash_idx.contains_key(&hash) {
            return Ok(());
        }
        let id = Uuid::new_v4().simple().to_string();
        self.keys.insert(
            id.clone(),
            ApiKeyRow {
                id: id.clone(),
                user_id: user_id.to_string(),
                name: name.to_string(),
                status: "active".to_string(),
                created_at: now(),
                group: String::new(),
            },
        );
        self.key_hash_idx.insert(hash, id);
        Ok(())
    }

    async fn resolve_key(&self, raw_key: &str) -> Option<ResolvedKey> {
        let key_id = self.key_hash_idx.get(&sha256_hex(raw_key))?.clone();
        let row = self.keys.get(&key_id)?.clone();
        let user = self.users.get(&row.user_id)?;
        Some(ResolvedKey {
            key_id,
            user_id: row.user_id.clone(),
            key_status: row.status.clone(),
            user_status: user.status.clone(),
            rate_limit: user.rate_limit,
            group: row.group.clone(),
        })
    }

    async fn list_keys(&self, user_id: &str) -> Vec<ApiKeyRow> {
        self.keys.iter().filter(|e| e.value().user_id == user_id).map(|e| e.value().clone()).collect()
    }
    async fn key_owner(&self, key_id: &str) -> Option<String> {
        self.keys.get(key_id).map(|r| r.user_id.clone())
    }
    async fn set_key_status(&self, key_id: &str, status: &str) -> anyhow::Result<()> {
        if let Some(mut r) = self.keys.get_mut(key_id) {
            r.status = status.to_string();
        }
        Ok(())
    }
    async fn set_key_group(&self, key_id: &str, group: &str) -> anyhow::Result<()> {
        if let Some(mut r) = self.keys.get_mut(key_id) {
            r.group = group.to_string();
        }
        Ok(())
    }
    async fn delete_key(&self, key_id: &str) -> anyhow::Result<()> {
        self.keys.remove(key_id);
        self.key_hash_idx.retain(|_, v| v != key_id);
        Ok(())
    }

    async fn balance(&self, user_id: &str) -> i64 {
        self.users.get(user_id).map(|u| u.balance).unwrap_or(0)
    }

    async fn reserve(&self, user_id: &str, cost: i64, req_id: &str, model: &str) -> Result<i64, BalanceErr> {
        let bal = {
            let mut u = self.users.get_mut(user_id).ok_or(BalanceErr::Insufficient)?;
            if u.status != "active" || u.balance < cost {
                return Err(BalanceErr::Insufficient);
            }
            u.balance -= cost;
            u.balance
        };
        self.push_tx(user_id, "charge", -cost, bal, Some(req_id), Some(model), None);
        Ok(bal)
    }

    async fn refund(&self, user_id: &str, amount: i64, req_id: &str) -> anyhow::Result<i64> {
        let bal = {
            let mut u = self.users.get_mut(user_id).ok_or_else(|| anyhow::anyhow!("user not found"))?;
            u.balance += amount;
            u.balance
        };
        self.push_tx(user_id, "refund", amount, bal, Some(req_id), None, None);
        Ok(bal)
    }

    async fn recharge(&self, user_id: &str, amount: i64, note: &str) -> anyhow::Result<i64> {
        let bal = {
            let mut u = self.users.get_mut(user_id).ok_or_else(|| anyhow::anyhow!("user not found"))?;
            u.balance += amount;
            u.balance
        };
        self.push_tx(user_id, "recharge", amount, bal, None, None, Some(note));
        Ok(bal)
    }

    async fn list_tx(&self, user_id: &str, limit: i64) -> Vec<Tx> {
        let mut v: Vec<Tx> = self.txs.lock().unwrap().iter().filter(|t| t.user_id == user_id).cloned().collect();
        v.sort_by(|a, b| b.id.cmp(&a.id));
        v.truncate(limit.max(0) as usize);
        v
    }

    async fn create_redeem_codes(&self, amount: i64, count: usize) -> anyhow::Result<Vec<String>> {
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            let code = gen_code();
            self.codes.insert(code.clone(), RedeemCode { code: code.clone(), amount, created_at: now(), used_by: None, used_at: None });
            out.push(code);
        }
        Ok(out)
    }
    async fn list_redeem_codes(&self) -> Vec<RedeemCode> {
        self.codes.iter().map(|e| e.value().clone()).collect()
    }
    async fn redeem(&self, user_id: &str, code: &str) -> Result<i64, RedeemErr> {
        let amount = {
            let mut c = self.codes.get_mut(code).ok_or(RedeemErr::NotFound)?;
            if c.used_by.is_some() {
                return Err(RedeemErr::Used);
            }
            c.used_by = Some(user_id.to_string());
            c.used_at = Some(now());
            c.amount
        };
        let bal = {
            let mut u = self.users.get_mut(user_id).ok_or(RedeemErr::Other("user not found".into()))?;
            u.balance += amount;
            u.balance
        };
        self.push_tx(user_id, "redeem", amount, bal, None, None, Some(code));
        Ok(bal)
    }

    // ---- 账号恢复（邮箱）----
    async fn get_user_by_email(&self, email: &str) -> Option<User> {
        self.users
            .iter()
            .find(|e| e.value().email.as_deref() == Some(email))
            .map(|e| e.value().clone())
    }
    async fn set_email(&self, user_id: &str, email: &str) -> anyhow::Result<()> {
        if let Some(mut u) = self.users.get_mut(user_id) {
            u.email = Some(email.to_string());
        }
        Ok(())
    }
    async fn set_password_hash(&self, user_id: &str, hash: &str) -> anyhow::Result<()> {
        if let Some(mut u) = self.users.get_mut(user_id) {
            u.password_hash = hash.to_string();
        }
        Ok(())
    }
    async fn set_email_verified(&self, user_id: &str, verified: bool) -> anyhow::Result<()> {
        if let Some(mut u) = self.users.get_mut(user_id) {
            u.email_verified = verified;
        }
        Ok(())
    }

    async fn set_totp_secret(&self, user_id: &str, secret: Option<&str>) -> anyhow::Result<()> {
        if let Some(mut u) = self.users.get_mut(user_id) {
            u.totp_secret = secret.map(|s| s.to_string());
        }
        Ok(())
    }
    async fn set_totp_enabled(&self, user_id: &str, enabled: bool) -> anyhow::Result<()> {
        if let Some(mut u) = self.users.get_mut(user_id) {
            u.totp_enabled = enabled;
        }
        Ok(())
    }
    async fn bump_token_version(&self, user_id: &str) -> anyhow::Result<i64> {
        if let Some(mut u) = self.users.get_mut(user_id) {
            u.token_version += 1;
            Ok(u.token_version)
        } else {
            anyhow::bail!("user not found")
        }
    }
    async fn append_audit(&self, actor: &str, action: &str, target: &str, detail: serde_json::Value) {
        let id = self.audit_id.fetch_add(1, Ordering::SeqCst) + 1;
        self.audit.lock().unwrap().push(AuditRow {
            id,
            actor: actor.into(),
            action: action.into(),
            target: target.into(),
            detail,
            created_at: now(),
        });
    }
    async fn list_audit(&self, f: AuditFilter) -> Vec<AuditRow> {
        let mut v: Vec<AuditRow> = self
            .audit
            .lock()
            .unwrap()
            .iter()
            .filter(|r| f.actor.as_ref().is_none_or(|a| &r.actor == a))
            .filter(|r| f.action.as_ref().is_none_or(|a| &r.action == a))
            .filter(|r| f.since.is_none_or(|s| r.created_at >= s))
            .cloned()
            .collect();
        v.sort_by(|a, b| b.id.cmp(&a.id));
        if f.limit > 0 {
            v.truncate(f.limit as usize);
        }
        v
    }

    // ---- 在线支付订单 ----
    async fn create_payment_order(&self, order_id: &str, user_id: &str, amount: i64, provider: &str) -> anyhow::Result<()> {
        self.orders.insert(
            order_id.to_string(),
            PaymentOrder {
                order_id: order_id.to_string(),
                user_id: user_id.to_string(),
                amount,
                provider: provider.to_string(),
                status: "pending".to_string(),
                created_at: now(),
                paid_at: None,
            },
        );
        Ok(())
    }
    async fn get_payment_order(&self, order_id: &str) -> Option<PaymentOrder> {
        self.orders.get(order_id).map(|o| o.clone())
    }
    async fn mark_payment_paid(&self, order_id: &str) -> anyhow::Result<Option<i64>> {
        // 幂等：仅当 pending → paid 才入账。
        let (user_id, amount) = {
            let mut o = match self.orders.get_mut(order_id) {
                Some(o) => o,
                None => return Ok(None),
            };
            if o.status != "pending" {
                return Ok(None);
            }
            o.status = "paid".to_string();
            o.paid_at = Some(now());
            (o.user_id.clone(), o.amount)
        };
        let bal = {
            let mut u = self.users.get_mut(&user_id).ok_or_else(|| anyhow::anyhow!("user not found"))?;
            u.balance += amount;
            u.balance
        };
        self.push_tx(&user_id, "recharge", amount, bal, None, None, Some(&format!("epay:{order_id}")));
        Ok(Some(bal))
    }
    async fn list_payment_orders(&self, user_id: &str, limit: i64) -> Vec<PaymentOrder> {
        let mut v: Vec<PaymentOrder> = self.orders.iter().filter(|e| e.value().user_id == user_id).map(|e| e.value().clone()).collect();
        v.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        v.truncate(limit.max(0) as usize);
        v
    }

    async fn usage_summary(&self, since: i64, until: i64) -> Vec<UsageRow> {
        use std::collections::HashMap;
        let mut agg: HashMap<String, (i64, i64)> = HashMap::new();
        for t in self.txs.lock().unwrap().iter() {
            if t.created_at < since || t.created_at > until {
                continue;
            }
            let e = agg.entry(t.kind.clone()).or_insert((0, 0));
            e.0 += t.amount;
            e.1 += 1;
        }
        agg.into_iter().map(|(kind, (sum, count))| UsageRow { kind, sum, count }).collect()
    }

    async fn usage_by_user(&self, since: i64, until: i64, limit: i64) -> Vec<UserUsageRow> {
        use std::collections::HashMap;
        // user_id -> (charge_amount_sum(负), refund_sum, charge_count)
        let mut agg: HashMap<String, (i64, i64, i64)> = HashMap::new();
        {
            let txs = self.txs.lock().unwrap();
            for t in txs.iter() {
                if t.created_at < since || t.created_at > until {
                    continue;
                }
                let e = agg.entry(t.user_id.clone()).or_insert((0, 0, 0));
                match t.kind.as_str() {
                    "charge" => {
                        e.0 += t.amount;
                        e.2 += 1;
                    }
                    "refund" => e.1 += t.amount,
                    _ => {}
                }
            }
        } // 释放 txs 锁后再查 users
        let mut rows: Vec<UserUsageRow> = agg
            .into_iter()
            .filter(|(_, (_, _, count))| *count > 0)
            .map(|(user_id, (charge_sum, refund, count))| {
                let username = self.users.get(&user_id).map(|u| u.username.clone()).unwrap_or_default();
                UserUsageRow { user_id, username, spent: -charge_sum, refunded: refund, count }
            })
            .collect();
        rows.sort_by(|a, b| b.spent.cmp(&a.spent));
        if limit > 0 {
            rows.truncate(limit as usize);
        }
        rows
    }

    async fn usage_by_model(&self, since: i64, until: i64) -> Vec<ModelUsageRow> {
        use std::collections::HashMap;
        let mut agg: HashMap<String, (i64, i64)> = HashMap::new();
        for t in self.txs.lock().unwrap().iter() {
            if t.created_at < since || t.created_at > until || t.kind != "charge" {
                continue;
            }
            if let Some(m) = &t.model {
                let e = agg.entry(m.clone()).or_insert((0, 0));
                e.0 += t.amount;
                e.1 += 1;
            }
        }
        let mut rows: Vec<ModelUsageRow> = agg
            .into_iter()
            .map(|(model, (sum, count))| ModelUsageRow { model, spent: -sum, count })
            .collect();
        rows.sort_by(|a, b| b.spent.cmp(&a.spent));
        rows
    }
}
