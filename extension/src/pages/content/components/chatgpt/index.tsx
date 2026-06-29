import { useCallback, useEffect, useRef } from 'react';
import { useMessageHandler } from '../../shared/useMessageHandler';
import type { AskContent } from '../../shared/useMessageHandler';
import { makeLoaderListener } from '../../shared/streamForward';
import {
  waitForAny,
  typeIntoEditor,
  submitPrompt,
  awaitSubmitted,
  injectImages,
  logDom,
  reportInjectFailure,
} from '../../shared/domInject';

// 编辑器选择器链（多版本适配）。
const CHATGPT_EDITOR = [
  '#prompt-textarea',
  'div[contenteditable="true"]#prompt-textarea',
  'textarea[name="prompt-textarea"]',
  'div.ProseMirror[contenteditable="true"]',
  'div[contenteditable="true"]',
];
// 发送按钮选择器，按优先级回退（不依赖单一 id / 英文整串）。
const CHATGPT_SEND = [
  '#composer-submit-button',
  'button[data-testid="send-button"]',
  'button[aria-label*="Send" i]',
];
// 图片：文件 input 选择器（优先 accept 含 image 的，避开代码解释器/文档 input）+ 附件预览判定（blob: 缩略图，需真机校）。
const CHATGPT_UPLOAD = ['input[type="file"][accept*="image" i]', 'input[type="file"]'];
const IMG_PREVIEW = 'img[src^="blob:"]';

export const ChatGPT = () => {
  const reqIdRef = useRef<string>('');

  const sendToChat = useCallback(async (content: AskContent) => {
    const editor = (await waitForAny(CHATGPT_EDITOR)) as HTMLElement | null;
    if (!editor) {
      reportInjectFailure(content.req_id, `ChatGPT editor not found (tried: ${CHATGPT_EDITOR.join(', ')})`);
      return;
    }
    if (!typeIntoEditor(editor, content.prompt)) {
      reportInjectFailure(content.req_id, 'ChatGPT prompt insert failed (editor empty after input)');
      return;
    }
    // 图片：发送前注入并等附件上传完成；失败即整请求快速失败（不静默丢图）。
    if (content.images?.length) {
      const ok = await injectImages(editor, content.images, CHATGPT_UPLOAD, IMG_PREVIEW);
      if (!ok) {
        reportInjectFailure(content.req_id, `ChatGPT image injection failed (${content.images.length} imgs; preview '${IMG_PREVIEW}' not seen)`);
        return;
      }
      logDom(content.req_id, 'info', `attached ${content.images.length} image(s) to ChatGPT`);
    }
    const how = await submitPrompt(editor, CHATGPT_SEND);
    logDom(content.req_id, how === 'click' ? 'info' : 'warn', `submitted prompt to ChatGPT via ${how}`);
    // 注入自检：确认对话请求确实发出；否则快速失败，不让客户端干等服务端 generation 超时。
    if (!(await awaitSubmitted(content.req_id))) {
      reportInjectFailure(content.req_id, `ChatGPT submit not confirmed (no conversation request fired; send selectors: ${CHATGPT_SEND.join(', ')})`);
    }
  }, []);

  useMessageHandler(sendToChat, reqIdRef);

  useEffect(() => {
    const newScript = document.createElement('script');
    newScript.src = chrome.runtime.getURL('loader.js');
    document.body.appendChild(newScript);
    const listener = makeLoaderListener(reqIdRef);
    window.addEventListener('message', listener);
    return () => window.removeEventListener('message', listener);
  }, []);

  return <div />;
};
