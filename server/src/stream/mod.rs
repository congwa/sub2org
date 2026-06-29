//! req_id → 用户 SSE 通道。worker 的 chunk/done/error 经此 fan-out 给 HTTP 侧。

/// 一次请求生命周期内，从 worker 流回的事件。
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// 增量 token。
    Delta(String),
    /// 完成：全文 + 网页原生 conversation id（用于会话亲和钉定）。
    Done {
        full: String,
        conversation_id: Option<String>,
    },
    /// worker 侧错误。
    Error(String),
}
