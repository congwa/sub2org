// 注入到 claude.ai 页面世界：劫持 fetch，把 Claude 的 SSE 增量实时流式 postMessage。
// Claude SSE: data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"hello"}}
(function () {
  const originalFetch = window.fetch;

  // E6：content script 在每次 ask_question 时推入当前 req_id；fetch 发起时锁定它。
  let currentReqId = '';
  window.addEventListener('message', (ev) => {
    if (ev.source !== window) return;
    const d: any = ev.data || {};
    if (typeof d.s2o_set_req === 'string') currentReqId = d.s2o_set_req;
  });

  window.fetch = async function (...args) {
    const reqId = currentReqId; // 锁定本次 fetch 归属的 req_id
    const url = typeof args[0] === 'string' ? args[0] : (args[0] as Request)?.url ?? '';
    const isConversation = url.includes('/completion') || url.includes('/chat_conversations');
    const method = (
      (args[1] as RequestInit | undefined)?.method ||
      (typeof args[0] !== 'string' ? (args[0] as Request)?.method : '') ||
      'GET'
    ).toUpperCase();
    // Opt3：completion POST 发出 = 注入&点击发送成功，回报「已提交」供 content 自检。
    if (method === 'POST' && url.includes('/completion')) {
      window.postMessage({ s2o: true, req_id: reqId, event: 'submitted' }, '*');
    }

    const response = await originalFetch(...args);
    const contentType = response.headers.get('content-type') || '';
    if (!contentType.includes('text/event-stream') || !isConversation) {
      return response;
    }

    const clone = response.clone();
    const reader = clone.body?.getReader();
    const decoder = new TextDecoder('utf-8');

    let buffer = '';
    let full = '';
    let conversationId: string | null = null;

    const post = (payload: Record<string, unknown>) =>
      window.postMessage({ s2o: true, req_id: reqId, ...payload }, '*');

    // E3：SSE 解析为空时，从页面已渲染的「本轮新」Claude 回答兜底取文本。
    // continue 会话页含历史回答，故先记基线，仅采用新增/增长的内容，否则返回空让 server 走 empty_response。
    const asstSel = '.font-claude-message, div.standard-markdown';
    const baseNodes = document.querySelectorAll(asstSel);
    const baseLast = baseNodes[baseNodes.length - 1] as HTMLElement | undefined;
    const baseText = (baseLast?.innerText || '').trim();
    const domFallbackText = (): string => {
      const nodes = document.querySelectorAll(asstSel);
      const last = nodes[nodes.length - 1] as HTMLElement | undefined;
      if (!last) return '';
      const text = (last.innerText || '').trim();
      if (!text) return '';
      return last !== baseLast || text !== baseText ? text : '';
    };

    // 从 URL 里抓 conversation uuid（/chat_conversations/<uuid>/completion）
    const m = url.match(/chat_conversations\/([0-9a-f-]{36})/i);
    if (m) conversationId = m[1];

    async function readStream() {
      if (!reader) return;
      try {
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          buffer += decoder.decode(value, { stream: true });
          const parts = buffer.split(/\r?\n\r?\n/);
          buffer = parts.pop() as string;

          for (let part of parts) {
            part = part.trim();
            if (!part) continue;
            for (const line of part.split(/\r?\n/)) {
              if (!line.startsWith('data:')) continue;
              const dataStr = line.slice(5).trim();
              if (!dataStr || dataStr === '[DONE]') continue;
              try {
                const obj = JSON.parse(dataStr);
                if (
                  obj.type === 'content_block_delta' &&
                  obj.delta?.type === 'text_delta' &&
                  typeof obj.delta?.text === 'string'
                ) {
                  full += obj.delta.text;
                  post({ event: 'chunk', delta: obj.delta.text });
                }
              } catch {
                // skip
              }
            }
          }
        }
        // E3：流结束但零内容 → DOM 兜底取已渲染回答。
        if (!full.trim()) {
          const dom = domFallbackText();
          if (dom) {
            full = dom;
            post({ event: 'chunk', delta: dom });
            post({ event: 'log', category: 'dom', level: 'warn', message: 'Claude SSE empty → used DOM fallback' });
          }
        }
        post({ event: 'done', full, conversation_id: conversationId });
      } catch (err) {
        post({ event: 'error', message: String(err) });
      }
    }

    readStream();
    return response;
  };
})();
