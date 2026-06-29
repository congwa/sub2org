//! socketioxide 网关：worker 握手鉴权 + 自注册入库、心跳/状态、流式事件接入、断连快速失败。

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use socketioxide::extract::{Data, SocketRef, TryData};
use socketioxide::SocketIo;

use crate::logging::{now_ms, LogEvent};
use crate::metrics::LiveEvent;
use crate::state::AppState;
use crate::stream::StreamEvent;
use crate::ws::protocol::{
    Chunk, Done, Heartbeat, WorkerAuth, WorkerError, WorkerLogBatch, WorkerStatus,
};

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 在默认 namespace 上挂载 worker 连接处理。
pub fn register(io: &SocketIo, state: Arc<AppState>) {
    io.ns("/", move |socket: SocketRef, auth: TryData<WorkerAuth>| {
        let state = state.clone();
        async move { on_connect(socket, auth, state).await }
    });
}

async fn on_connect(socket: SocketRef, auth: TryData<WorkerAuth>, state: Arc<AppState>) {
    let auth = match auth.0 {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!("worker 握手缺少/非法 auth，断开: {e}");
            let _ = socket.disconnect();
            return;
        }
    };

    // bootstrap token 校验（共享准入令牌）
    if !state.config.worker_token_ok(&auth.worker_token) {
        tracing::warn!(worker_id = %auth.worker_id, "worker_token 校验失败，断开");
        let _ = socket.disconnect();
        return;
    }

    // 被 admin 禁用的 worker 拒绝接入
    if state.store.is_worker_disabled(&auth.worker_id).await {
        tracing::warn!(worker_id = %auth.worker_id, "worker 已被禁用，断开");
        let _ = socket.disconnect();
        return;
    }

    // 自注册入库（首见 insert，再见更新 last_seen）
    if let Err(e) = state
        .store
        .upsert_worker(&auth.worker_id, &auth.account_label, &auth.provider, &auth.group)
        .await
    {
        tracing::error!(worker_id = %auth.worker_id, "upsert_worker 失败: {e}");
    }

    tracing::info!(
        category = "connection",
        worker_id = %auth.worker_id,
        provider = %auth.provider,
        account = %auth.account_label,
        capacity = auth.capacity,
        sid = %socket.id,
        "worker 上线"
    );

    state.registry.register(
        auth.worker_id.clone(),
        socket.id,
        auth.provider.clone(),
        auth.group.clone(),
        auth.account_label.clone(),
        auth.capacity,
        now_secs(),
    );

    let _ = socket.emit("ready", &serde_json::json!({ "worker_id": auth.worker_id }));
    state.publish(LiveEvent::WorkerStatus {
        worker_id: auth.worker_id.clone(),
        status: "idle".to_string(),
    });

    let worker_id = auth.worker_id.clone();

    // 流式增量
    {
        let st = state.clone();
        socket.on("chunk", move |Data::<Chunk>(c)| {
            st.route(&c.req_id, StreamEvent::Delta(c.delta));
        });
    }
    // 完成：路由 + 计数（内存即时，DB 异步）
    {
        let st = state.clone();
        let wid = worker_id.clone();
        socket.on("done", move |Data::<Done>(d)| {
            let st = st.clone();
            let wid = wid.clone();
            async move {
                st.route(
                    &d.req_id,
                    StreamEvent::Done {
                        full: d.full_text,
                        conversation_id: d.native_conversation_id,
                    },
                );
                st.registry.mark_served(&wid);
                st.registry.record_worker_success(&wid); // B4：成功完成即清熔断错误窗
                let _ = st.store.incr_worker_served(&wid).await;
            }
        });
    }
    // 错误：路由；rate_limited 带 cooling_until → 标记冷却
    {
        let st = state.clone();
        let wid = worker_id.clone();
        socket.on("error", move |Data::<WorkerError>(e)| {
            if e.code == "rate_limited" && e.cooling_until > 0 {
                st.registry.set_status(&wid, true, e.cooling_until);
                if let Some(s) = st.registry.status_of(&wid, now_secs()) {
                    st.publish(LiveEvent::WorkerStatus { worker_id: wid.clone(), status: s.to_string() });
                }
            }
            st.publish(LiveEvent::RequestError { req_id: e.req_id.clone(), code: e.code.clone() });
            // B4：worker 级故障码计入熔断；窗内达阈值 → 停派发并让在途请求立即失败。
            if st.config.breaker.enabled && st.config.breaker.fault_codes.iter().any(|c| c == &e.code) {
                let now = now_secs();
                if st.registry.record_worker_error(
                    &wid, now,
                    st.config.breaker.window_secs,
                    st.config.breaker.threshold,
                    st.config.breaker.open_secs,
                ) {
                    tracing::warn!(category = "connection", worker_id = %wid, "熔断触发：停止派发");
                    st.fail_worker_inflight(&wid, "worker circuit breaker tripped");
                    if let Some(s) = st.registry.status_of(&wid, now) {
                        st.publish(LiveEvent::WorkerStatus { worker_id: wid.clone(), status: s.to_string() });
                    }
                }
            }
            let msg = if e.message.is_empty() { e.code.clone() } else { e.message.clone() };
            st.route(&e.req_id, StreamEvent::Error(msg));
        });
    }
    // 即时状态上报（掉登录/限流/恢复）
    {
        let st = state.clone();
        let wid = worker_id.clone();
        socket.on("status", move |Data::<WorkerStatus>(s)| {
            tracing::info!(category = "rate_limit", worker_id = %wid, available = s.available, cooling_until = s.cooling_until, reason = %s.reason, "worker status");
            st.registry.set_status(&wid, s.available, s.cooling_until);
            if let Some(st_str) = st.registry.status_of(&wid, now_secs()) {
                st.publish(LiveEvent::WorkerStatus { worker_id: wid.clone(), status: st_str.to_string() });
            }
            // S7：worker 掉登录（available=false 且非冷却窗 → logged_out）时，让其名下在途请求立即失败，
            // 不再干等 generation 超时。冷却(rate_limited，cooling_until>now)与 available=true 不触发。
            if !s.available && s.cooling_until <= now_secs() {
                st.fail_worker_inflight(&wid, "worker logged out");
            }
        });
    }
    // 心跳：刷新 last_seen + 同步可用性/冷却
    {
        let st = state.clone();
        let wid = worker_id.clone();
        socket.on("heartbeat", move |Data::<Heartbeat>(hb)| {
            st.registry.touch(&wid, now_secs());
            st.registry.set_status(&wid, hb.available, hb.cooling_until);
            // S7 一致性：心跳也携带 available；掉登录（available=false 且非冷却）时同样让在途请求立即失败，
            // 兜住 status 事件偶发丢失的窄缝。无在途请求时为 no-op。
            if !hb.available && hb.cooling_until <= now_secs() {
                st.fail_worker_inflight(&wid, "worker logged out");
            }
        });
    }

    // worker 日志摄入：source/worker_id/account 由连接身份补，不信任 payload
    {
        let st = state.clone();
        let wid = worker_id.clone();
        let acct = auth.account_label.clone();
        socket.on("log", move |Data::<WorkerLogBatch>(b)| {
            for e in b.entries {
                st.log_sink.log(LogEvent {
                    ts_ms: if e.ts_ms > 0 { e.ts_ms } else { now_ms() },
                    source: "worker".to_string(),
                    worker_id: Some(wid.clone()),
                    account_label: if acct.is_empty() { None } else { Some(acct.clone()) },
                    category: if e.category.is_empty() { "system".to_string() } else { e.category },
                    level: if e.level.is_empty() { "info".to_string() } else { e.level },
                    req_id: e.req_id,
                    session_id: None,
                    message: e.message,
                    fields: e.fields,
                });
            }
        });
    }

    // 断开：在途请求快速失败 + 清理
    {
        let st = state.clone();
        socket.on_disconnect(move |s: SocketRef| {
            if let Some(worker_id) = st.registry.remove_by_sid(&s.id) {
                st.fail_worker_inflight(&worker_id, "worker disconnected");
                st.publish(LiveEvent::WorkerStatus { worker_id: worker_id.clone(), status: "offline".to_string() });
                tracing::info!(category = "connection", %worker_id, "worker 下线");
            }
        });
    }
}
