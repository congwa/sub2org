//! GET /admin/events — admin 实时事件 SSE。订阅 AppState 的 broadcast 通道。
//! 前端用 fetch-stream（能带 Authorization），故仍用 AdminGuard bearer 鉴权。

use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::account::AdminGuard;
use crate::state::AppState;

pub async fn admin_events(_admin: AdminGuard, State(state): State<Arc<AppState>>) -> Response {
    let rx = state.events.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|res| async move {
        match res {
            Ok(ev) => serde_json::to_string(&ev)
                .ok()
                .map(|json| Ok::<Event, Infallible>(Event::default().data(json))),
            // 慢订阅者 lag → 跳过丢失事件，不杀连接
            Err(_) => None,
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}
