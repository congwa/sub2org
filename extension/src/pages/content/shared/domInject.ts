// E5：健壮的 prompt 注入工具。
// 旧实现「直接 innerHTML/value 赋值 + 固定延时点死选择器」对 React/ProseMirror 受控组件无效，
// 且选择器一改版就静默失效。这里改为：模拟真实输入事件 + 多重按钮 fallback + 失败立即回报。

/** 等选择器出现（MutationObserver）；超时 resolve(null)。 */
export const waitForElement = (selector: string, timeout = 10000): Promise<Element | null> =>
  new Promise((resolve) => {
    const el = document.querySelector(selector);
    if (el) return resolve(el);
    const observer = new MutationObserver(() => {
      const found = document.querySelector(selector);
      if (found) {
        observer.disconnect();
        resolve(found);
      }
    });
    observer.observe(document.body, { childList: true, subtree: true });
    setTimeout(() => {
      observer.disconnect();
      resolve(null);
    }, timeout);
  });

/** 按优先级顺序等待多个选择器中任一出现（选择器自愈/多版本适配）；超时 resolve(null)。 */
export const waitForAny = (selectors: string[], timeout = 10000): Promise<Element | null> =>
  new Promise((resolve) => {
    const find = (): Element | null => {
      for (const s of selectors) {
        const el = document.querySelector(s);
        if (el) return el;
      }
      return null;
    };
    const immediate = find();
    if (immediate) return resolve(immediate);
    const observer = new MutationObserver(() => {
      const el = find();
      if (el) {
        observer.disconnect();
        resolve(el);
      }
    });
    observer.observe(document.body, { childList: true, subtree: true });
    setTimeout(() => {
      observer.disconnect();
      resolve(null);
    }, timeout);
  });

/**
 * 注入自检：等待页面世界 loader 发来的「已提交」确定信号（本轮对话请求真正发出）。
 * 命中 resolve(true)；超时 resolve(false)（=发送没触发，应让请求快速失败而非干等服务端超时）。
 */
export const awaitSubmitted = (reqId: string, timeout = 10000): Promise<boolean> =>
  new Promise((resolve) => {
    let done = false;
    const finish = (ok: boolean) => {
      if (done) return;
      done = true;
      window.removeEventListener('message', listener);
      clearTimeout(timer);
      resolve(ok);
    };
    const listener = (ev: MessageEvent) => {
      if (ev.source !== window) return;
      const d: any = ev.data || {};
      if (d.s2o && d.event === 'submitted' && (!d.req_id || d.req_id === reqId)) finish(true);
    };
    window.addEventListener('message', listener);
    const timer = setTimeout(() => finish(false), timeout);
  });

/** 轮询多选择器，等到出现且未 disabled 的按钮再 resolve；超时返回当前命中（可能 disabled）或 null。 */
export const waitForEnabledButton = (
  selectors: string[],
  timeout = 6000
): Promise<HTMLButtonElement | null> =>
  new Promise((resolve) => {
    const start = Date.now();
    const find = (): HTMLButtonElement | null => {
      for (const s of selectors) {
        const b = document.querySelector(s) as HTMLButtonElement | null;
        if (b) return b;
      }
      return null;
    };
    const tick = () => {
      const b = find();
      if (b && !b.disabled) return resolve(b);
      if (Date.now() - start > timeout) return resolve(b);
      setTimeout(tick, 150);
    };
    tick();
  });

const editorText = (editor: HTMLElement): string =>
  (editor.innerText || (editor as HTMLTextAreaElement).value || '').trim();

/**
 * 把 text 写进 contenteditable / textarea：
 * 1) execCommand('insertText')——首选，能让 ProseMirror/React 感知值变化；
 * 2) 失败则「真正写入 DOM」再派发 input：textarea 用原生 value setter（绕过 React 受控追踪），
 *    contenteditable 用 Selection/Range 插入文本节点。
 * 注意：单纯 dispatch 合成 InputEvent（isTrusted=false）不会让浏览器自动插字，故必须先落 DOM。
 * 返回写入后是否非空。
 */
export const typeIntoEditor = (editor: HTMLElement, text: string): boolean => {
  editor.focus();
  let ok = false;
  try {
    ok = document.execCommand('insertText', false, text);
  } catch {
    ok = false;
  }
  if (!ok || !editorText(editor)) {
    const tag = editor.tagName;
    if (tag === 'TEXTAREA' || tag === 'INPUT') {
      // 原生 value setter + input 事件：React 受控组件的标准写入手法。
      const proto = tag === 'TEXTAREA' ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
      const setter = Object.getOwnPropertyDescriptor(proto, 'value')?.set;
      if (setter) setter.call(editor, text);
      else (editor as HTMLTextAreaElement).value = text;
    } else {
      // contenteditable：清空后插入文本节点。
      const sel = window.getSelection();
      const range = document.createRange();
      range.selectNodeContents(editor);
      range.deleteContents();
      const node = document.createTextNode(text);
      range.insertNode(node);
      range.setStartAfter(node);
      range.collapse(true);
      sel?.removeAllRanges();
      sel?.addRange(range);
    }
    editor.dispatchEvent(new InputEvent('input', { inputType: 'insertText', data: text, bubbles: true }));
  }
  return editorText(editor).length > 0;
};

/** 找发送按钮并点击；都不可用则向编辑器派发 Enter。返回采用的提交方式。 */
export const submitPrompt = async (
  editor: HTMLElement,
  selectors: string[]
): Promise<'click' | 'enter'> => {
  const btn = await waitForEnabledButton(selectors);
  if (btn && !btn.disabled) {
    btn.click();
    return 'click';
  }
  editor.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', code: 'Enter', bubbles: true }));
  return 'enter';
};

/** DOM 操作日志（经 background → 服务端日志流）。 */
export const logDom = (reqId: string, level: string, message: string) =>
  chrome.runtime.sendMessage({
    type: 'worker_log',
    content: { category: 'dom', level, message, req_id: reqId },
  });

/**
 * 注入失败：记日志 + 让当前请求立即失败（经 background stream_error → 服务端 502），
 * 不再让客户端干等到 generation timeout。
 */
export const reportInjectFailure = (reqId: string, message: string) => {
  logDom(reqId, 'error', message);
  chrome.runtime.sendMessage({ type: 'stream_error', content: { req_id: reqId, message } });
};

// ---- 图片注入 ----

/** `data:` URI → File。data URI 的 fetch 无 CORS。 */
const dataUriToFile = async (uri: string, idx: number): Promise<File | null> => {
  try {
    const blob = await (await fetch(uri)).blob();
    const ext = (blob.type.split('/')[1] || 'png').split('+')[0];
    return new File([blob], `image-${idx}.${ext}`, { type: blob.type || 'image/png' });
  } catch {
    return null;
  }
};

/**
 * 等出现 want 张「新」预览（不在 beforeSet 中的元素）。用元素引用差集而非计数，
 * 避免 continue 会话页里历史 blob: 图污染基线导致误判/永久超时。超时 false。
 */
const waitForNewPreviews = (
  selector: string,
  beforeSet: Set<Element>,
  want: number,
  timeout: number
): Promise<boolean> =>
  new Promise((resolve) => {
    const start = Date.now();
    const tick = () => {
      const fresh = Array.from(document.querySelectorAll(selector)).filter((el) => !beforeSet.has(el)).length;
      if (fresh >= want) return resolve(true);
      if (Date.now() - start > timeout) return resolve(false);
      setTimeout(tick, 200);
    };
    tick();
  });

/**
 * 把图片注入对话框。Chrome 下合成 ClipboardEvent 的 clipboardData 基本读不到 → **文件 input 为主路径**
 * （input.files = DataTransfer.files + change），无 input 时才退合成 paste。用「新预览元素差集」确认上传完成。
 * 返回是否全部就绪；否则 false（调用方据此快速失败，不静默丢图）。
 */
export const injectImages = async (
  editor: HTMLElement,
  dataUris: string[],
  uploadSelectors: string[],
  previewSelector: string
): Promise<boolean> => {
  if (!dataUris.length) return true;
  const files: File[] = [];
  for (let i = 0; i < dataUris.length; i++) {
    const f = await dataUriToFile(dataUris[i], i);
    if (!f) return false;
    files.push(f);
  }
  // 基线：记录已存在的预览元素引用（排除历史图），用差集判定新附件。
  const beforeSet = new Set<Element>(Array.from(document.querySelectorAll(previewSelector)));

  const dt = new DataTransfer();
  for (const f of files) dt.items.add(f);

  // 主：文件 input（优先 accept 含 image 的那个）。
  const input = (await waitForAny(uploadSelectors, 4000)) as HTMLInputElement | null;
  if (input) {
    try {
      input.files = dt.files;
    } catch {
      /* 个别实现 files 不可写 */
    }
    input.dispatchEvent(new Event('change', { bubbles: true }));
    input.dispatchEvent(new Event('input', { bubbles: true }));
  } else {
    // 兜底：合成 paste（best-effort，多数 Chrome 下无效）。
    editor.focus();
    try {
      editor.dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true }));
    } catch {
      /* ignore */
    }
  }

  // 确认：出现 files.length 张「新」预览（上传完成）才算成功（大图给足 20s）。
  return waitForNewPreviews(previewSelector, beforeSet, files.length, 20000);
};
