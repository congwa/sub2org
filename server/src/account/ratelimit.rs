//! 每用户在途并发限流（rate_limit=0 表示不限）。

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use dashmap::DashMap;

#[derive(Default)]
pub struct RateLimiter {
    inflight: DashMap<String, Arc<AtomicU32>>,
}

/// 持有期间占用一个并发名额，drop 时释放。
pub struct RateGuard {
    counter: Arc<AtomicU32>,
}
impl Drop for RateGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}

impl RateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// 占用一个名额；limit<=0 不限。超限返回 None。
    pub fn try_acquire(&self, user_id: &str, limit: i32) -> Option<RateGuard> {
        let counter = self
            .inflight
            .entry(user_id.to_string())
            .or_insert_with(|| Arc::new(AtomicU32::new(0)))
            .clone();
        if limit > 0 {
            loop {
                let cur = counter.load(Ordering::SeqCst);
                if cur as i32 >= limit {
                    return None;
                }
                if counter
                    .compare_exchange(cur, cur + 1, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    break;
                }
            }
        } else {
            counter.fetch_add(1, Ordering::SeqCst);
        }
        Some(RateGuard { counter })
    }
}
