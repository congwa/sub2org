//! worker ↔ sub2org 的 Socket.IO 消息体（serde）。
//! 对应 sub2org-worker 扩展 background 里 emit/on 的事件 payload。

use serde::{Deserialize, Serialize};

/// 握手 auth：`io(server, { auth: {...} })`。socketioxide 在 connect handler 里以 Data 提取。
#[derive(Debug, Clone, Deserialize)]
pub struct WorkerAuth {
    pub worker_token: String,
    pub worker_id: String,
    pub provider: String,
    /// B2 分组标签（旧扩展无此字段 → 默认 ""）。
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub account_label: String,
    #[serde(default = "default_capacity")]
    pub capacity: usize,
}

fn default_capacity() -> usize {
    1
}

/// S→W：派发一次提问。
#[derive(Debug, Clone, Serialize)]
pub struct Dispatch {
    pub req_id: String,
    pub session_id: String,
    /// "new" 开新窗口 | "continue" 回历史窗口
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_conversation_id: Option<String>,
    pub provider: String,
    pub prompt: String,
    /// 多模态图片：每项一个 `data:<mime>;base64,...` URI（base64 透传 / https 服务端 fetch 归一）。
    /// 空则不序列化，向后兼容旧 worker。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<String>,
}

/// S→W：取消。
#[derive(Debug, Clone, Serialize)]
pub struct Cancel {
    pub req_id: String,
}

/// W→S：心跳（带可用性/冷却，便于一次性同步状态）。
#[derive(Debug, Clone, Deserialize)]
pub struct Heartbeat {
    #[serde(default)]
    pub free_slots: usize,
    #[serde(default = "default_true")]
    pub available: bool,
    #[serde(default)]
    pub cooling_until: i64,
}

fn default_true() -> bool {
    true
}

/// W→S：即时状态上报（掉登录 / 限流冷却 / 恢复），不必等下次心跳。
#[derive(Debug, Clone, Deserialize)]
pub struct WorkerStatus {
    #[serde(default = "default_true")]
    pub available: bool,
    #[serde(default)]
    pub cooling_until: i64,
    #[serde(default)]
    pub reason: String,
}

/// W→S：流式增量。
#[derive(Debug, Clone, Deserialize)]
pub struct Chunk {
    pub req_id: String,
    pub delta: String,
}

/// W→S：完成。
#[derive(Debug, Clone, Deserialize)]
pub struct Done {
    pub req_id: String,
    #[serde(default)]
    pub full_text: String,
    #[serde(default)]
    pub native_conversation_id: Option<String>,
}

/// W→S：批量日志上报（worker_id/account 由服务端按连接身份补，不信任 payload）。
#[derive(Debug, Clone, Deserialize)]
pub struct WorkerLogEntry {
    #[serde(default)]
    pub ts_ms: i64,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub level: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub req_id: Option<String>,
    #[serde(default)]
    pub fields: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkerLogBatch {
    #[serde(default)]
    pub entries: Vec<WorkerLogEntry>,
}

/// W→S：错误。code="rate_limited" 时带 cooling_until → 标记该 worker 冷却。
#[derive(Debug, Clone, Deserialize)]
pub struct WorkerError {
    pub req_id: String,
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub cooling_until: i64,
}
