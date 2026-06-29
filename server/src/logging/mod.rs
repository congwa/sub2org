//! 日志系统：按 端(source) / 类型(category) / 级别(level) 分类入库。
//! 设计铁律：绝不阻塞中转——非阻塞投递(满了丢+计数)、批量异步入库、DB 挂了降级、
//! 内部只用 eprintln!（避免 tracing→日志→tracing 反馈环）。

pub mod layer;
pub mod store;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::sync::mpsc;

pub use store::{InMemoryLogStore, LogStore, PgLogStore};

/// 一条日志。
#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct LogEvent {
    pub ts_ms: i64,
    pub source: String, // "server" | "worker"
    pub worker_id: Option<String>,
    pub account_label: Option<String>,
    pub category: String,
    pub level: String,
    pub req_id: Option<String>,
    pub session_id: Option<String>,
    pub message: String,
    pub fields: serde_json::Value,
}

/// 查询过滤条件。
#[derive(Debug, Default)]
pub struct LogQuery {
    pub source: Option<String>,
    pub worker_id: Option<String>,
    pub category: Option<String>,
    pub min_level: Option<u8>,
    pub req_id: Option<String>,
    pub session_id: Option<String>,
    pub since_ms: Option<i64>,
    pub until_ms: Option<i64>,
    pub limit: i64,
    pub offset: i64,
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 级别排序：trace<debug<info<warn<error。未知按 info。
pub fn level_rank(level: &str) -> u8 {
    match level.to_ascii_lowercase().as_str() {
        "trace" => 0,
        "debug" => 1,
        "info" => 2,
        "warn" | "warning" => 3,
        "error" => 4,
        _ => 2,
    }
}

/// 不低于 min_rank 的级别名集合（查询时 level=ANY 用）。
pub fn levels_at_least(min_rank: u8) -> Vec<String> {
    ["trace", "debug", "info", "warn", "error"]
        .iter()
        .filter(|l| level_rank(l) >= min_rank)
        .map(|s| s.to_string())
        .collect()
}

/// 非阻塞日志投递端，可克隆共享。
#[derive(Clone)]
pub struct LogSink {
    tx: mpsc::Sender<LogEvent>,
    min_level: u8,
    dropped: Arc<AtomicU64>,
}

impl LogSink {
    /// 返回 sink + 配套 receiver（交给 flusher）。
    pub fn new(capacity: usize, min_level: u8) -> (Self, mpsc::Receiver<LogEvent>) {
        let (tx, rx) = mpsc::channel(capacity.max(16));
        (
            Self {
                tx,
                min_level,
                dropped: Arc::new(AtomicU64::new(0)),
            },
            rx,
        )
    }

    /// 投递一条日志。级别不够直接丢；通道满则计数丢弃，绝不阻塞。
    pub fn log(&self, ev: LogEvent) {
        if level_rank(&ev.level) < self.min_level {
            return;
        }
        if self.tx.try_send(ev).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

/// 批量入库任务：尽量攒批，channel 关闭即退出。
pub fn spawn_flusher(mut rx: mpsc::Receiver<LogEvent>, store: Arc<dyn LogStore>) {
    tokio::spawn(async move {
        let mut buf: Vec<LogEvent> = Vec::with_capacity(512);
        loop {
            let n = rx.recv_many(&mut buf, 500).await;
            if n == 0 {
                break; // 通道关闭
            }
            let batch = std::mem::take(&mut buf);
            if let Err(e) = store.insert_batch(&batch).await {
                eprintln!("[log] insert_batch failed: {e}");
            }
        }
    });
}

/// 定时清理超期日志。
pub fn spawn_retention(store: Arc<dyn LogStore>, retention_days: i64) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(3600));
        loop {
            tick.tick().await;
            let cutoff = now_ms() - retention_days * 86_400_000;
            if let Err(e) = store.cleanup(cutoff).await {
                eprintln!("[log] cleanup failed: {e}");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_levels() {
        assert!(level_rank("trace") < level_rank("debug"));
        assert!(level_rank("debug") < level_rank("info"));
        assert!(level_rank("info") < level_rank("warn"));
        assert!(level_rank("warn") < level_rank("error"));
        assert_eq!(level_rank("INFO"), level_rank("info"));
        assert_eq!(level_rank("unknown"), level_rank("info"));
    }

    #[test]
    fn levels_at_least_filters() {
        assert_eq!(levels_at_least(3), vec!["warn", "error"]);
        assert_eq!(levels_at_least(4), vec!["error"]);
        assert_eq!(levels_at_least(0).len(), 5);
    }
}
