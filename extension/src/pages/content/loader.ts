// 注入到页面 JS 世界：劫持 window.fetch，解析 ChatGPT 的 text/event-stream，
// 把增量 token 实时 postMessage 给 content script（真流式）。
(function () {
  const originalFetch = window.fetch;

  // #4：清理 ChatGPT 网页端私有引用/图片标记（防泄漏进对外正文）。
  // 形如 `citeturn0search1` / `turn0image3`：可选前导关键词 + 一段 PUA 包裹内容。
  // 三遍兜底——带 PUA 定界符的整段；定界符被网页吞掉后残留的裸 token；任何残余 PUA 字符。
  // ⚠️ 精确 PUA 码点随网页演进，需真机抓一条标记 charCodeAt 复核（同 E5/DOM 脆弱面）。
  const stripCitationMarkers = (t: string): string =>
    t
      .replace(/(?:cite|filecite|navlist|video)?[\uE000-\uF8FF][\s\S]*?[\uE000-\uF8FF]/g, "")
      .replace(/turn\d+(?:search|image|news|view|video|forecast)\d+/g, "")
      .replace(/[\uE000-\uF8FF]/g, "");

  // E6：content script 在每次 ask_question 时推入当前 req_id；fetch 发起时锁定它。
  let currentReqId = "";
  window.addEventListener("message", (ev) => {
    if (ev.source !== window) return;
    const d: any = ev.data || {};
    if (typeof d.s2o_set_req === "string") currentReqId = d.s2o_set_req;
  });

  window.fetch = async function (...args) {
    const reqId = currentReqId; // 锁定本次 fetch 归属的 req_id（即使后续被新请求覆盖）
    const url = typeof args[0] === "string" ? args[0] : (args[0] as Request)?.url ?? "";
    // 只处理对话接口的流，避免捕获页面其它无关 SSE
    const isConversation = url.includes("/conversation");
    const method = (
      (args[1] as RequestInit | undefined)?.method ||
      (typeof args[0] !== "string" ? (args[0] as Request)?.method : "") ||
      "GET"
    ).toUpperCase();
    // Opt3：本轮对话请求真正发出（POST）= 注入&点击发送成功，回报「已提交」供 content 自检。
    if (isConversation && method === "POST") {
      window.postMessage({ s2o: true, req_id: reqId, event: "submitted" }, "*");
    }

    const response = await originalFetch(...args);
    const contentType = response.headers.get("content-type") || "";

    const postStatus = (payload: Record<string, unknown>) =>
      window.postMessage({ s2o: true, event: "status", ...payload }, "*");

    // 限流/掉登录检测（对话接口的错误响应）
    if (isConversation && !response.ok) {
      if (response.status === 429) {
        const ra = parseInt(response.headers.get("retry-after") || "0", 10);
        const cooling = ra > 0 ? Math.floor(Date.now() / 1000) + ra : 0;
        postStatus({ available: true, cooling_until: cooling, reason: "rate_limited" });
      } else if (response.status === 401 || response.status === 403) {
        postStatus({ available: false, cooling_until: 0, reason: "logged_out" });
      }
      return response;
    }

    if (!contentType.includes("text/event-stream") || !isConversation) {
      return response;
    }

    const clone = response.clone();
    const reader = clone.body?.getReader();
    const decoder = new TextDecoder("utf-8");

    let buffer = "";
    let full = "";
    let conversationId: string | null = null;

    const post = (payload: Record<string, unknown>) =>
      window.postMessage({ s2o: true, req_id: reqId, ...payload }, "*");

    // E3：SSE 解析为空时，从页面已渲染的「本轮新」assistant 回答兜底取文本。
    // continue 会话页含历史回答，必须排除——在流开始前先记基线（最后一条 assistant 节点 + 其文本），
    // 仅当出现新节点、或同节点内容相对基线增长时才采用，否则返回空让 server 走 empty_response，
    // 避免把上一轮陈旧答案当本轮结果返回（比返回空更糟）。
    const asstSel = '[data-message-author-role="assistant"]';
    const baseNodes = document.querySelectorAll(asstSel);
    const baseLast = baseNodes[baseNodes.length - 1] as HTMLElement | undefined;
    const baseText = (baseLast?.innerText || "").trim();
    const domFallbackText = (): string => {
      const nodes = document.querySelectorAll(asstSel);
      const last = nodes[nodes.length - 1] as HTMLElement | undefined;
      if (!last) return "";
      const text = (last.innerText || "").trim();
      if (!text) return "";
      return last !== baseLast || text !== baseText ? text : "";
    };

    async function readStream() {
      if (reader === undefined) return;
      // #4：按消息元数据闸住正文累加（accept-unless-disqualified）。
      // 默认接受——仅当能正面识别为非用户消息（工具/搜索/思考/旁注）才拒，
      // 元数据缺失/schema 漂移时不黑洞答案（漏标记可由 strip 兜，丢答案不可逆）。
      let accept = true;
      // 当前增量目标是否是 /message/content/parts 正文。默认 true：裸字符串增量(④)即便
      // 出现在任何显式 path 之前也照收（不黑洞答案，回归旧版无条件累加行为）；仅当显式 path
      // 切到非 parts（如 metadata）时才置 false。工具/搜索/思考消息由 accept 拦，不靠这个。
      let partsPathActive = true;
      const applySnapshot = (m: any) => {
        if (!m) return;
        const role = m.author?.role;
        const recipient = m.recipient;
        const ctype = m.content?.content_type;
        const thinking = m.metadata?.is_thinking_preamble_message;
        const channel = m.channel ?? m.metadata?.channel;
        accept = !(
          (role != null && role !== "assistant") ||
          (recipient != null && recipient !== "all") ||
          (ctype != null && ctype !== "text") ||
          thinking === true ||
          channel === "commentary" ||
          channel === "analysis"
        );
        partsPathActive = true; // 切到新消息：新消息的下一段内容预期就是 parts 正文
      };
      // parts 正文追加：append 或裸增量（无 o）；replace 不收（避免整段重放翻倍）。
      const isPartsAppend = (p: any, o: any): boolean =>
        typeof p === "string" &&
        p.includes("/message/content/parts") &&
        (o === undefined || o === "append");
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

            let eventName: string | null = null;
            const dataLines: string[] = [];
            for (const line of part.split(/\r?\n/)) {
              if (line.startsWith("event:")) eventName = line.slice(6).trim();
              else if (line.startsWith("data:")) dataLines.push(line.slice(5).trim());
            }
            const dataStr = dataLines.join("\n");
            if (!dataStr || dataStr === "[DONE]") continue;

            let obj: any;
            try {
              obj = JSON.parse(dataStr);
            } catch {
              continue;
            }

            if (obj.conversation_id) conversationId = obj.conversation_id;
            else if (obj.v && typeof obj.v === "object" && obj.v.conversation_id)
              conversationId = obj.v.conversation_id;

            const take = (s: string) => {
              if (!accept || !partsPathActive) return;
              full += s;
              post({ event: "chunk", delta: s });
            };

            // ChatGPT 增量四态：①消息快照对象 ②patch 数组 ③顶层单 patch(带 p/o) ④裸字符串。
            if (eventName === "delta" || obj.v !== undefined) {
              if (obj.v && typeof obj.v === "object" && obj.v.message) {
                // ① 完整快照（首帧或中途 o:"add"/p:"" 新消息）→ 重算是否接受。
                applySnapshot(obj.v.message);
              } else if (Array.isArray(obj.v)) {
                // ② patch 数组。
                for (const patch of obj.v) {
                  if (!patch) continue;
                  if (patch.v && typeof patch.v === "object" && patch.v.message) {
                    applySnapshot(patch.v.message);
                  } else if (typeof patch.p === "string") {
                    partsPathActive = isPartsAppend(patch.p, patch.o);
                    if (typeof patch.v === "string") take(patch.v);
                  } else if (typeof patch.v === "string") {
                    take(patch.v); // 续上一路径
                  }
                }
              } else if (typeof obj.p === "string") {
                // ③ 顶层单 patch（ChatGPT 最常见的流式增量形态）：先按 path 更新路径态。
                partsPathActive = isPartsAppend(obj.p, obj.o);
                if (typeof obj.v === "string") take(obj.v);
              } else if (typeof obj.v === "string") {
                // ④ 裸字符串：续上一条 parts 路径。
                take(obj.v);
              }
            }
          }
        }
        // E3：流结束但零内容 → DOM 兜底取已渲染回答，避免返回空响应。
        if (!full.trim()) {
          const dom = domFallbackText();
          if (dom) {
            full = stripCitationMarkers(dom);
            post({ event: "chunk", delta: full });
            post({ event: "log", category: "dom", level: "warn", message: "ChatGPT SSE empty → used DOM fallback" });
          }
        } else {
          // #4：聚合结果统一清理引用/图片私有标记（流式 chunk 可能短暂带标记，done 干净）。
          full = stripCitationMarkers(full);
        }
        post({ event: "done", full, conversation_id: conversationId });
      } catch (err) {
        post({ event: "error", message: String(err) });
      }
    }

    readStream();
    return response;
  };
})();
