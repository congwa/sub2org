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
const CLAUDE_EDITOR = [
  'div[contenteditable="true"].ProseMirror',
  'div.ProseMirror[contenteditable="true"]',
  'div[contenteditable="true"]',
];
// 发送按钮选择器：不依赖英文整串 aria-label，逐级回退。
const CLAUDE_SEND = [
  'button[aria-label="Send message"]',
  'button[aria-label*="Send" i]',
  'button[data-testid="send-button"]',
];
// 图片：文件 input（优先 accept 含 image 的）+ 附件预览判定（blob: 缩略图，需真机校）。
const CLAUDE_UPLOAD = ['input[type="file"][accept*="image" i]', 'input[type="file"]'];
const IMG_PREVIEW = 'img[src^="blob:"]';

export const Claude = () => {
  const reqIdRef = useRef<string>('');

  const sendToChat = useCallback(async (content: AskContent) => {
    const editor = (await waitForAny(CLAUDE_EDITOR)) as HTMLElement | null;
    if (!editor) {
      reportInjectFailure(content.req_id, `Claude editor not found (tried: ${CLAUDE_EDITOR.join(', ')})`);
      return;
    }
    if (!typeIntoEditor(editor, content.prompt)) {
      reportInjectFailure(content.req_id, 'Claude prompt insert failed (editor empty after input)');
      return;
    }
    // 图片：发送前注入并等附件上传完成；失败即整请求快速失败（不静默丢图）。
    if (content.images?.length) {
      const ok = await injectImages(editor, content.images, CLAUDE_UPLOAD, IMG_PREVIEW);
      if (!ok) {
        reportInjectFailure(content.req_id, `Claude image injection failed (${content.images.length} imgs; preview '${IMG_PREVIEW}' not seen)`);
        return;
      }
      logDom(content.req_id, 'info', `attached ${content.images.length} image(s) to Claude`);
    }
    const how = await submitPrompt(editor, CLAUDE_SEND);
    logDom(content.req_id, how === 'click' ? 'info' : 'warn', `submitted prompt to Claude via ${how}`);
    // 注入自检：确认 completion 请求确实发出；否则快速失败。
    if (!(await awaitSubmitted(content.req_id))) {
      reportInjectFailure(content.req_id, `Claude submit not confirmed (no completion request fired; send selectors: ${CLAUDE_SEND.join(', ')})`);
    }
  }, []);

  useMessageHandler(sendToChat, reqIdRef);

  useEffect(() => {
    const newScript = document.createElement('script');
    newScript.src = chrome.runtime.getURL('loader-claude.js');
    document.body.appendChild(newScript);
    const listener = makeLoaderListener(reqIdRef);
    window.addEventListener('message', listener);
    return () => window.removeEventListener('message', listener);
  }, []);

  return <div />;
};
