//! 实时指标 + 事件类型。RequestRing（请求计数环，始终在线，不依赖日志）、
//! QueueWaitStat（排队等待 EWMA）、LiveEvent（admin SSE 广播事件）。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::Serialize;

/// 24h × 1min = 1440 桶的环形请求计数器。每桶 (epoch_minute, count)。
pub struct RequestRing {
    buckets: Box<[Mutex<(i64, u32)>]>,
}

impl Default for RequestRing {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestRing {
    pub fn new() -> Self {
        let buckets = (0..1440).map(|_| Mutex::new((0i64, 0u32))).collect::<Vec<_>>();
        Self {
            buckets: buckets.into_boxed_slice(),
        }
    }

    /// dispatch 成功时调用：O(1)，仅锁一个桶。
    pub fn incr(&self, now_secs: i64) {
        let minute = now_secs / 60;
        let idx = minute.rem_euclid(1440) as usize;
        let mut b = self.buckets[idx].lock().unwrap();
        if b.0 != minute {
            *b = (minute, 0);
        }
        b.1 += 1;
    }

    /// 最近 window_mins 分钟的每分钟计数（旧→新）。
    pub fn series(&self, now_secs: i64, window_mins: i64) -> Vec<u32> {
        let now_min = now_secs / 60;
        (0..window_mins)
            .rev()
            .map(|back| {
                let minute = now_min - back;
                let idx = minute.rem_euclid(1440) as usize;
                let b = self.buckets[idx].lock().unwrap();
                if b.0 == minute {
                    b.1
                } else {
                    0
                }
            })
            .collect()
    }

    pub fn total(&self, now_secs: i64, window_mins: i64) -> u64 {
        self.series(now_secs, window_mins).iter().map(|&c| c as u64).sum()
    }
}

/// 排队等待滚动均值（毫秒），指数滑动平均。
#[derive(Default)]
pub struct QueueWaitStat {
    ewma_ms: AtomicU64,
    count: AtomicU64,
}

impl QueueWaitStat {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn record(&self, wait_ms: u64) {
        let n = self.count.fetch_add(1, Ordering::Relaxed);
        let prev = self.ewma_ms.load(Ordering::Relaxed);
        let next = if n == 0 { wait_ms } else { prev - prev / 8 + wait_ms / 8 };
        self.ewma_ms.store(next, Ordering::Relaxed);
    }
    pub fn avg_ms(&self) -> Option<u64> {
        if self.count.load(Ordering::Relaxed) == 0 {
            None
        } else {
            Some(self.ewma_ms.load(Ordering::Relaxed))
        }
    }
}

/// admin 实时事件（广播给 /admin/events 订阅者）。
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LiveEvent {
    RequestDispatched {
        req_id: String,
        model: String,
        worker_id: String,
        session_id: String,
        action: String,
    },
    RequestDone {
        req_id: String,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        tokens: Option<u32>,
        latency_ms: u64,
    },
    RequestError {
        req_id: String,
        code: String,
    },
    WorkerStatus {
        worker_id: String,
        status: String,
    },
    BalanceLow {
        user_id: String,
        balance: i64,
        threshold: i64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_counts_and_series() {
        let r = RequestRing::new();
        let now = 1_000_000_000i64; // 某个固定秒
        r.incr(now);
        r.incr(now);
        r.incr(now - 60); // 上一分钟
        assert_eq!(r.total(now, 2), 3);
        let s = r.series(now, 2);
        assert_eq!(s, vec![1, 2]); // 旧→新
    }

    #[test]
    fn queue_wait_ewma() {
        let q = QueueWaitStat::new();
        assert_eq!(q.avg_ms(), None);
        q.record(100);
        assert_eq!(q.avg_ms(), Some(100));
        q.record(100);
        assert_eq!(q.avg_ms(), Some(100));
    }
}
