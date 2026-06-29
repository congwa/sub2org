//! tracing Layer：把 server 端的 tracing 事件转成 LogEvent 投入 LogSink。
//! 只采集本 crate（target 以 "sub2org" 开头）的事件——既分类干净，又避免
//! sqlx/hyper 自身日志触发「写库→产生日志→再写库」的反馈环。

use std::fmt::Debug;

use serde_json::{Map, Value};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

use super::{now_ms, LogEvent, LogSink};

pub struct LogChannelLayer {
    sink: LogSink,
}

impl LogChannelLayer {
    pub fn new(sink: LogSink) -> Self {
        Self { sink }
    }
}

#[derive(Default)]
struct FieldVisitor {
    message: String,
    category: Option<String>,
    req_id: Option<String>,
    session_id: Option<String>,
    worker_id: Option<String>,
    account: Option<String>,
    fields: Map<String, Value>,
}

fn strip_quotes(s: &str) -> String {
    let b = s.as_bytes();
    if b.len() >= 2 && b[0] == b'"' && b[b.len() - 1] == b'"' {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

impl FieldVisitor {
    fn route(&mut self, name: &str, sval: String, jval: Value) {
        match name {
            "message" => self.message = sval,
            "category" => self.category = Some(sval),
            "req_id" => self.req_id = Some(sval),
            "session_id" => self.session_id = Some(sval),
            "worker_id" => self.worker_id = Some(sval),
            "account" | "account_label" => self.account = Some(sval),
            _ => {
                self.fields.insert(name.to_string(), jval);
            }
        }
    }
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, f: &Field, v: &dyn Debug) {
        let raw = format!("{v:?}");
        let clean = strip_quotes(&raw);
        self.route(f.name(), clean.clone(), Value::String(clean));
    }
    fn record_str(&mut self, f: &Field, v: &str) {
        self.route(f.name(), v.to_string(), Value::String(v.to_string()));
    }
    fn record_i64(&mut self, f: &Field, v: i64) {
        self.route(f.name(), v.to_string(), Value::from(v));
    }
    fn record_u64(&mut self, f: &Field, v: u64) {
        self.route(f.name(), v.to_string(), Value::from(v));
    }
    fn record_bool(&mut self, f: &Field, v: bool) {
        self.route(f.name(), v.to_string(), Value::from(v));
    }
    fn record_f64(&mut self, f: &Field, v: f64) {
        self.route(f.name(), v.to_string(), Value::from(v));
    }
}

fn category_from_target(target: &str) -> String {
    if target.contains("gateway") {
        "connection"
    } else if target.contains("scheduler") {
        "scheduler"
    } else if target.contains("consumer") {
        "http"
    } else if target.contains("health") {
        "system"
    } else {
        "system"
    }
    .to_string()
}

fn level_str(l: &Level) -> &'static str {
    match *l {
        Level::ERROR => "error",
        Level::WARN => "warn",
        Level::INFO => "info",
        Level::DEBUG => "debug",
        Level::TRACE => "trace",
    }
}

impl<S: Subscriber> Layer<S> for LogChannelLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        // 只采集本 crate 的事件（防止依赖库日志反馈环 + 分类噪音）
        if !meta.target().starts_with("sub2org") {
            return;
        }
        let mut v = FieldVisitor::default();
        event.record(&mut v);
        let category = v
            .category
            .unwrap_or_else(|| category_from_target(meta.target()));
        self.sink.log(LogEvent {
            ts_ms: now_ms(),
            source: "server".to_string(),
            worker_id: v.worker_id,
            account_label: v.account,
            category,
            level: level_str(meta.level()).to_string(),
            req_id: v.req_id,
            session_id: v.session_id,
            message: v.message,
            fields: Value::Object(v.fields),
        });
    }
}
