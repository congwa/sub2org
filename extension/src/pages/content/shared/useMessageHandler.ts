import { useEffect } from 'react';
import type { MutableRefObject } from 'react';

/** sub2org → 扩展 → content 的一次提问内容。 */
export type AskContent = {
  req_id: string;
  prompt: string;
  action: 'new' | 'continue';
  native_conversation_id?: string;
  /** 多模态图片：每项一个 data: URI（服务端已归一）。 */
  images?: string[];
};

/**
 * 监听 background 转发的 ask_question。把当前 req_id 写入 reqIdRef，
 * 以便后续 loader 的流式 chunk 能带上正确的 req_id 回传。
 */
export const useMessageHandler = (
  onAskQuestion: (content: AskContent) => void,
  reqIdRef: MutableRefObject<string>
) => {
  useEffect(() => {
    const listener = (msg: any) => {
      if (msg?.type === 'ask_question' && msg.content) {
        reqIdRef.current = msg.content.req_id;
        // E6：把当前 req_id 推进页面 JS 世界，供 loader 在发起 fetch 时锁定到本次回答，
        // 即使后续有新请求覆盖 reqIdRef，进行中的流仍带正确 req_id。
        window.postMessage({ s2o_set_req: msg.content.req_id }, '*');
        onAskQuestion(msg.content as AskContent);
      }
    };
    chrome.runtime.onMessage.addListener(listener);
    return () => chrome.runtime.onMessage.removeListener(listener);
  }, [onAskQuestion, reqIdRef]);
};
