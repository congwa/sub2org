import type { MutableRefObject } from 'react';

/**
 * 构造一个 window message 监听器：把页面世界 loader 发来的
 * { s2o:true, event:'chunk'|'done'|'error', ... } 透传给 background，
 * 并带上当前 req_id。
 */
export const makeLoaderListener =
  (reqIdRef: MutableRefObject<string>) => (event: MessageEvent) => {
    if (event.source !== window) return;
    const d: any = event.data || {};
    if (!d.s2o) return;
    // E6：优先用 loader 在发起 fetch 时锁定并随帧带回的 req_id；缺省才回退到可变 ref。
    const req_id = typeof d.req_id === 'string' && d.req_id ? d.req_id : reqIdRef.current;
    if (d.event === 'chunk') {
      chrome.runtime.sendMessage({
        type: 'stream_chunk',
        content: { req_id, delta: d.delta },
      });
    } else if (d.event === 'done') {
      chrome.runtime.sendMessage({
        type: 'stream_done',
        content: { req_id, full: d.full, conversation_id: d.conversation_id },
      });
    } else if (d.event === 'error') {
      chrome.runtime.sendMessage({
        type: 'stream_error',
        content: { req_id, message: d.message },
      });
    } else if (d.event === 'log') {
      // E3：loader（页面世界，无 chrome.runtime）借转发通道上报日志（如走了 DOM 兜底）。
      chrome.runtime.sendMessage({
        type: 'worker_log',
        content: {
          category: d.category ?? 'dom',
          level: d.level ?? 'info',
          message: d.message ?? '',
          req_id,
        },
      });
    } else if (d.event === 'status') {
      // worker 级状态（限流/掉登录），与 req 无关
      chrome.runtime.sendMessage({
        type: 'worker_status',
        content: {
          available: d.available,
          cooling_until: d.cooling_until ?? 0,
          reason: d.reason ?? '',
        },
      });
    }
  };
