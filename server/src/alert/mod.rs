//! 告警钩子：订阅 LiveEvent 广播，按窗口聚合后经 webhook / SMTP 投递。
//! best-effort，永不阻塞 relay；min_interval 去重防刷；Lagged 容忍丢弃。

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::sync::broadcast::error::RecvError;

use crate::config::AlertConfig;
use crate::metrics::LiveEvent;
use crate::state::AppState;

/// 一条要投递的告警（webhook JSON body）。
#[derive(Clone, Debug, Serialize)]
pub struct Alert {
    pub kind: String,     // worker_offline | worker_logged_out | error_rate | balance_low
    pub severity: String, // warning | critical
    pub summary: String,
    pub detail: serde_json::Value,
    pub ts: i64,
}

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// 去重 / 最小间隔节流：每个 key 记上次发送时刻。
struct Throttle {
    last: HashMap<String, i64>,
    min_interval: i64,
}
impl Throttle {
    fn new(min_interval: i64) -> Self {
        Self { last: HashMap::new(), min_interval }
    }
    /// true=可发；false=静默窗内跳过。
    fn allow(&mut self, key: &str, now: i64) -> bool {
        match self.last.get(key) {
            Some(&t) if now - t < self.min_interval => false,
            _ => {
                self.last.insert(key.to_string(), now);
                true
            }
        }
    }
}

/// 错误率滑动窗（最近 window 秒内 error/total）。
struct ErrWindow {
    window_secs: i64,
    events: VecDeque<(i64, bool)>, // (ts, is_error)
}
impl ErrWindow {
    fn new(window_secs: i64) -> Self {
        Self { window_secs, events: VecDeque::new() }
    }
    fn record(&mut self, now: i64, is_error: bool) {
        self.events.push_back((now, is_error));
        let cutoff = now - self.window_secs;
        while matches!(self.events.front(), Some(&(t, _)) if t < cutoff) {
            self.events.pop_front();
        }
    }
    /// (rate 0..1, total)；total < min 时 None，避免小样本误报。
    fn rate(&self, min_total: usize) -> Option<(f64, usize)> {
        let total = self.events.len();
        if total < min_total {
            return None;
        }
        let errs = self.events.iter().filter(|(_, e)| *e).count();
        Some((errs as f64 / total as f64, total))
    }
}

const ERR_WINDOW_SECS: i64 = 60;
const ERR_MIN_SAMPLE: usize = 20;

/// 启动告警任务（未配置 / 关闭则直接返回，不 spawn）。
pub fn spawn(state: Arc<AppState>) {
    let cfg = match state.config.alert.clone() {
        Some(c) if c.enabled => c,
        _ => return,
    };
    // reqwest 用 no-provider：显式安装 ring（复用项目已有的 ring，不引 aws-lc-rs）。
    let _ = rustls::crypto::ring::default_provider().install_default();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();
    let mut rx = state.events.subscribe();

    tokio::spawn(async move {
        let mut throttle = Throttle::new(cfg.min_interval_secs.max(1));
        let mut errwin = ErrWindow::new(ERR_WINDOW_SECS);

        loop {
            match rx.recv().await {
                Ok(ev) => {
                    let now = now_secs();
                    let alert: Option<Alert> = match &ev {
                        LiveEvent::WorkerStatus { worker_id, status } if status == "offline" => {
                            let key = format!("worker_offline:{worker_id}");
                            throttle.allow(&key, now).then(|| Alert {
                                kind: "worker_offline".into(),
                                severity: "warning".into(),
                                summary: format!("worker {worker_id} offline"),
                                detail: serde_json::json!({ "worker_id": worker_id }),
                                ts: now,
                            })
                        }
                        LiveEvent::WorkerStatus { worker_id, status } if status == "logged_out" => {
                            let key = format!("logged_out:{worker_id}");
                            throttle.allow(&key, now).then(|| Alert {
                                kind: "worker_logged_out".into(),
                                severity: "warning".into(),
                                summary: format!("worker {worker_id} 掉登录，需人工重新登录 ChatGPT/Claude"),
                                detail: serde_json::json!({ "worker_id": worker_id }),
                                ts: now,
                            })
                        }
                        LiveEvent::BalanceLow { user_id, balance, threshold } => {
                            let key = format!("balance_low:{user_id}");
                            throttle.allow(&key, now).then(|| Alert {
                                kind: "balance_low".into(),
                                severity: "warning".into(),
                                summary: format!("user {user_id} balance {balance} < {threshold}"),
                                detail: serde_json::json!({ "user_id": user_id, "balance": balance, "threshold": threshold }),
                                ts: now,
                            })
                        }
                        LiveEvent::RequestError { .. } | LiveEvent::RequestDone { .. } => {
                            let is_err = matches!(ev, LiveEvent::RequestError { .. });
                            errwin.record(now, is_err);
                            match errwin.rate(ERR_MIN_SAMPLE) {
                                Some((rate, total)) if rate >= cfg.error_rate_threshold => {
                                    throttle.allow("error_rate", now).then(|| Alert {
                                        kind: "error_rate".into(),
                                        severity: "critical".into(),
                                        summary: format!("error rate {:.0}% over {}s ({} reqs)", rate * 100.0, ERR_WINDOW_SECS, total),
                                        detail: serde_json::json!({ "rate": rate, "window_secs": ERR_WINDOW_SECS, "total": total }),
                                        ts: now,
                                    })
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    };
                    if let Some(a) = alert {
                        deliver(&client, &cfg, &state, a).await;
                    }
                }
                Err(RecvError::Lagged(_)) => continue, // 慢订阅丢事件，容忍
                Err(RecvError::Closed) => break,       // 关停
            }
        }
    });
}

async fn deliver(client: &reqwest::Client, cfg: &AlertConfig, state: &Arc<AppState>, alert: Alert) {
    if let Some(url) = cfg.webhook_url.as_deref().filter(|u| !u.is_empty()) {
        let mut req = client.post(url).json(&alert);
        if let Some(secret) = cfg.webhook_secret.as_deref().filter(|s| !s.is_empty()) {
            req = req.header("X-Sub2org-Alert-Token", secret);
        }
        if let Err(e) = req.send().await {
            tracing::warn!(category = "alert", error = %e, kind = %alert.kind, "webhook 投递失败");
        }
    }
    if let (Some(to), Some(smtp)) = (
        cfg.email_to.as_deref().filter(|s| !s.is_empty()),
        state.config.smtp.as_ref(),
    ) {
        let subject = format!("[sub2org][{}] {}", alert.severity, alert.kind);
        let body = format!("{}\n\n{}", alert.summary, serde_json::to_string_pretty(&alert.detail).unwrap_or_default());
        if let Err(e) = crate::account::email::send(smtp, to, &subject, body).await {
            tracing::warn!(category = "alert", error = %e, "alert 邮件失败");
        }
    }
}
