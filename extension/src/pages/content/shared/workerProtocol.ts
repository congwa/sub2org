// worker ↔ sub2org 的 Socket.IO 线协议类型 + chrome 消息封套（P2#8）。
// ⚠️ 与 server/src/ws/protocol.rs 一一对应：改任一端（字段名/事件名）必须同步改另一端
//    （server ws/protocol.rs ↔ 本文件 ↔ background 的 socket.emit/on）。否则字段漂移会静默丢数据。

// ---- W→S 线协议（socket.emit 的 payload，字段名必须与 protocol.rs 完全一致）----
export interface WireChunk {
  req_id: string;
  delta: string;
}
export interface WireDone {
  req_id: string;
  full_text: string;
  native_conversation_id: string | null;
}
export interface WireError {
  req_id: string;
  code: string;
  message: string;
  /// 与 protocol.rs WorkerError 对齐：仅 code="rate_limited" 时有意义；content_error 恒 0
  /// （限流冷却实际走 status/heartbeat 事件，不走本路径，但类型保持与服务端结构一致）。
  cooling_until: number;
}
export interface WireStatus {
  available: boolean;
  cooling_until: number;
  reason: string;
}
export interface WireHeartbeat {
  free_slots: number;
  available: boolean;
  cooling_until: number;
}

// ---- chrome 消息封套（content scripts → background；历史上是 duck-typed {type, content}）----
// content 用 loader 侧字段名（full / conversation_id），到了 background 才归一为线协议名。
export interface StreamChunkContent {
  req_id: string;
  delta?: string;
}
export interface StreamDoneContent {
  req_id: string;
  full?: string;
  conversation_id?: string | null;
}
export interface StreamErrorContent {
  req_id: string;
  message?: string;
}

// ---- 封套 content → 线协议 的【唯一归一点】----
// 消除 full/full_text、conversation_id/native_conversation_id 在 loader→streamForward→background→server
// 多层重命名的隐式漂移（[35]）：字段映射集中在此，改名只动一处且有类型约束。
export const toWireChunk = (c: StreamChunkContent): WireChunk => ({
  req_id: c.req_id,
  delta: c.delta ?? '',
});
export const toWireDone = (c: StreamDoneContent): WireDone => ({
  req_id: c.req_id,
  full_text: c.full ?? '',
  native_conversation_id: c.conversation_id ?? null,
});
export const toWireError = (c: StreamErrorContent, code = 'content_error'): WireError => ({
  req_id: c.req_id,
  code,
  message: c.message ?? '',
  cooling_until: 0, // content_error 无冷却；与服务端 serde default 一致
});
