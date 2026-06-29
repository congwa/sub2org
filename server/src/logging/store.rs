//! 日志存储后端：内存环形缓冲（dev/无 DB）/ Postgres（批插/查询/清理）。

use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, QueryBuilder, Postgres};

use super::{levels_at_least, LogEvent, LogQuery};

#[async_trait]
pub trait LogStore: Send + Sync {
    async fn insert_batch(&self, events: &[LogEvent]) -> anyhow::Result<()>;
    async fn query(&self, f: &LogQuery) -> Vec<LogEvent>;
    async fn cleanup(&self, cutoff_ms: i64) -> anyhow::Result<()>;
}

/// 内存环形缓冲：保留最近 cap 条，供无 DB 时查询。
pub struct InMemoryLogStore {
    buf: Mutex<VecDeque<LogEvent>>,
    cap: usize,
}

impl InMemoryLogStore {
    pub fn new(cap: usize) -> Self {
        Self {
            buf: Mutex::new(VecDeque::with_capacity(cap.min(1024))),
            cap: cap.max(1),
        }
    }
}

fn matches(e: &LogEvent, f: &LogQuery) -> bool {
    if let Some(s) = &f.source {
        if &e.source != s {
            return false;
        }
    }
    if let Some(w) = &f.worker_id {
        if e.worker_id.as_deref() != Some(w.as_str()) {
            return false;
        }
    }
    if let Some(c) = &f.category {
        if &e.category != c {
            return false;
        }
    }
    if let Some(r) = &f.req_id {
        if e.req_id.as_deref() != Some(r.as_str()) {
            return false;
        }
    }
    if let Some(s) = &f.session_id {
        if e.session_id.as_deref() != Some(s.as_str()) {
            return false;
        }
    }
    if let Some(min) = f.min_level {
        if super::level_rank(&e.level) < min {
            return false;
        }
    }
    if let Some(since) = f.since_ms {
        if e.ts_ms < since {
            return false;
        }
    }
    if let Some(until) = f.until_ms {
        if e.ts_ms > until {
            return false;
        }
    }
    true
}

#[async_trait]
impl LogStore for InMemoryLogStore {
    async fn insert_batch(&self, events: &[LogEvent]) -> anyhow::Result<()> {
        let mut buf = self.buf.lock().unwrap();
        for e in events {
            if buf.len() >= self.cap {
                buf.pop_front();
            }
            buf.push_back(e.clone());
        }
        Ok(())
    }

    async fn query(&self, f: &LogQuery) -> Vec<LogEvent> {
        let buf = self.buf.lock().unwrap();
        let mut out: Vec<LogEvent> = buf.iter().filter(|e| matches(e, f)).cloned().collect();
        out.sort_by(|a, b| b.ts_ms.cmp(&a.ts_ms)); // ts 降序
        let off = f.offset.max(0) as usize;
        let lim = f.limit.max(0) as usize;
        out.into_iter().skip(off).take(lim).collect()
    }

    async fn cleanup(&self, cutoff_ms: i64) -> anyhow::Result<()> {
        let mut buf = self.buf.lock().unwrap();
        buf.retain(|e| e.ts_ms >= cutoff_ms);
        Ok(())
    }
}

/// Postgres 日志存储（自建连接池）。
pub struct PgLogStore {
    pool: PgPool,
}

impl PgLogStore {
    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }
}

#[async_trait]
impl LogStore for PgLogStore {
    async fn insert_batch(&self, events: &[LogEvent]) -> anyhow::Result<()> {
        if events.is_empty() {
            return Ok(());
        }
        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO logs (ts_ms, source, worker_id, account_label, category, level, req_id, session_id, message, fields) ",
        );
        qb.push_values(events, |mut b, e| {
            b.push_bind(e.ts_ms)
                .push_bind(e.source.clone())
                .push_bind(e.worker_id.clone())
                .push_bind(e.account_label.clone())
                .push_bind(e.category.clone())
                .push_bind(e.level.clone())
                .push_bind(e.req_id.clone())
                .push_bind(e.session_id.clone())
                .push_bind(e.message.clone())
                .push_bind(e.fields.clone());
        });
        qb.build().execute(&self.pool).await?;
        Ok(())
    }

    async fn query(&self, f: &LogQuery) -> Vec<LogEvent> {
        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT ts_ms, source, worker_id, account_label, category, level, req_id, session_id, message, fields FROM logs WHERE 1=1",
        );
        if let Some(s) = &f.source {
            qb.push(" AND source = ").push_bind(s.clone());
        }
        if let Some(w) = &f.worker_id {
            qb.push(" AND worker_id = ").push_bind(w.clone());
        }
        if let Some(c) = &f.category {
            qb.push(" AND category = ").push_bind(c.clone());
        }
        if let Some(r) = &f.req_id {
            qb.push(" AND req_id = ").push_bind(r.clone());
        }
        if let Some(s) = &f.session_id {
            qb.push(" AND session_id = ").push_bind(s.clone());
        }
        if let Some(min) = f.min_level {
            qb.push(" AND level = ANY(").push_bind(levels_at_least(min)).push(")");
        }
        if let Some(since) = f.since_ms {
            qb.push(" AND ts_ms >= ").push_bind(since);
        }
        if let Some(until) = f.until_ms {
            qb.push(" AND ts_ms <= ").push_bind(until);
        }
        qb.push(" ORDER BY ts_ms DESC LIMIT ")
            .push_bind(f.limit)
            .push(" OFFSET ")
            .push_bind(f.offset);

        qb.build_query_as::<LogEvent>()
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default()
    }

    async fn cleanup(&self, cutoff_ms: i64) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM logs WHERE ts_ms < $1")
            .bind(cutoff_ms)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
