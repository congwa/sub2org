import { io } from "socket.io-client";
import type { Socket } from "socket.io-client";
import { toWireChunk, toWireDone, toWireError } from "../content/shared/workerProtocol";

// sub2org 服务地址（self-host 默认本地）。可在设置里改，存到 chrome.storage "apiBaseUrl"。
export const DEFAULT_API_BASE_URL = "http://localhost:3300";
const DEFAULT_WORKER_TOKEN = "dev-worker-token";

const trimSlash = (u: string) => u.replace(/\/+$/, "");

const chatgptBaseUrl = "https://chatgpt.com/";
const claudeBaseUrl = "https://claude.ai/new";

export type Provider = "chatgpt" | "claude";

/** sub2org 派发下来的一次提问。 */
export type Dispatch = {
  req_id: string;
  session_id: string;
  action: "new" | "continue";
  native_conversation_id?: string;
  provider: string;
  prompt: string;
  /** 多模态图片：每项一个 data: URI（服务端归一）。随 ask_question 透传给 content script。 */
  images?: string[];
};

export type WorkerConfig = {
  serverBaseUrl: string;
  workerToken: string;
  workerId: string;
  accountLabel: string;
  provider: Provider;
  capacity: number;
  group: string; // B2：分组标签（""=默认池）
};

let socketConnectionStatus: {
  status: "pending" | "connected" | "failed" | "disconnected";
  errorMessage?: string;
} = { status: "disconnected", errorMessage: undefined };

chrome.runtime.onInstalled.addListener(() => {
  chrome.tabs.create({ url: "src/pages/settings/index.html" });
});

// ---------- storage helpers ----------

const getLocal = <T>(key: string, fallback: T): Promise<T> =>
  new Promise((resolve) =>
    chrome.storage.local.get([key], (r) => resolve((r[key] as T) ?? fallback))
  );

const setLocal = (obj: Record<string, unknown>): Promise<void> =>
  new Promise((resolve) => chrome.storage.local.set(obj, () => resolve()));

const getApiBaseUrl = () => getLocal<string>("apiBaseUrl", DEFAULT_API_BASE_URL);
const getProvider = () => getLocal<Provider>("provider", "chatgpt");
const getWorkerToken = () => getLocal<string>("workerToken", DEFAULT_WORKER_TOKEN);
const getAccountLabel = () => getLocal<string>("accountLabel", "");
const getGroup = () => getLocal<string>("group", ""); // B2
const getCapacity = () => getLocal<number>("capacity", 1);

// worker_id 一次生成、持久化（= 这台指纹浏览器/账号的稳定身份）
const getWorkerId = async (): Promise<string> => {
  const existing = await getLocal<string>("workerId", "");
  if (existing) return existing;
  const id =
    "w-" +
    Math.random().toString(36).substring(2, 10) +
    Math.random().toString(36).substring(2, 10);
  await setLocal({ workerId: id });
  return id;
};

const getProviderNewChatUrl = async (): Promise<string> => {
  const provider = await getProvider();
  return provider === "claude" ? claudeBaseUrl : chatgptBaseUrl;
};

const getConversationUrl = (provider: Provider, convId: string): string =>
  provider === "claude"
    ? `https://claude.ai/chat/${convId}`
    : `https://chatgpt.com/c/${convId}`;

const getWorkerConfig = async (): Promise<WorkerConfig> => ({
  serverBaseUrl: await getApiBaseUrl(),
  workerToken: await getWorkerToken(),
  workerId: await getWorkerId(),
  accountLabel: await getAccountLabel(),
  provider: await getProvider(),
  capacity: await getCapacity(),
  group: await getGroup(),
});

// ---------- 规模化预配（M2 托管策略 / M4 服务端领取）----------

/**
 * M2：企业策略 / MDM / 指纹浏览器集中下发的配置（chrome.storage.managed）。
 * 策略为准——每次启动覆盖本地，保证 fleet 不漂移。无策略时静默返回。
 */
async function applyManagedPolicy(): Promise<boolean> {
  if (!chrome.storage?.managed) return false;
  let managed: Record<string, unknown> = {};
  try {
    managed = await new Promise((resolve) =>
      chrome.storage.managed.get(
        ["serverBaseUrl", "apiBaseUrl", "workerToken", "provider", "accountLabel", "group"],
        (r) => resolve(r || {})
      )
    );
  } catch {
    return false; // 无策略的浏览器/profile 会抛错或返回空
  }
  if (!managed || Object.keys(managed).length === 0) return false;

  const patch: Record<string, unknown> = {};
  const server = (managed.serverBaseUrl ?? managed.apiBaseUrl) as string | undefined;
  if (server) patch.apiBaseUrl = server;
  if (managed.workerToken) patch.workerToken = managed.workerToken;
  if (managed.provider === "chatgpt" || managed.provider === "claude") {
    patch.provider = managed.provider;
  }
  if (typeof managed.accountLabel === "string") patch.accountLabel = managed.accountLabel;
  if (typeof managed.group === "string") patch.group = managed.group; // B2
  if (Object.keys(patch).length === 0) return false;

  patch.provisionSource = "managed";
  patch.managedApplied = true;
  await setLocal(patch);
  wlog("connection", "info", "applied managed policy", patch);
  return true;
}

/**
 * M4：首启用共享 worker_token 向服务端领取唯一身份（worker_id + account_label）。
 * 幂等（claimDone 标记）；失败软降级到本地自生成 id，下次启动重试。
 * 仅在有真实引导（自定义 server 或 token）且尚无标签时触发——保持手填流程向后兼容。
 */
async function ensureClaimedIdentity(): Promise<void> {
  if (await getLocal<boolean>("claimDone", false)) return;

  const serverBaseUrl = await getApiBaseUrl();
  const workerToken = await getWorkerToken();
  // 全是默认值（未配置任何东西）→ 没有可领取目标，跳过，避免无谓打 localhost。
  const unconfigured =
    serverBaseUrl === DEFAULT_API_BASE_URL && workerToken === DEFAULT_WORKER_TOKEN;
  if (!serverBaseUrl || !workerToken || unconfigured) return;

  // 已有标签（手填/managed/导入已给身份）→ 不再领取。
  if (await getAccountLabel()) {
    await setLocal({ claimDone: true });
    return;
  }

  const existingId = await getLocal<string>("workerId", "");
  try {
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), 8000);
    const resp = await fetch(`${trimSlash(serverBaseUrl)}/worker/claim`, {
      method: "POST",
      headers: { "content-type": "application/json", authorization: `Bearer ${workerToken}` },
      body: JSON.stringify({
        worker_token: workerToken,
        provider: await getProvider(),
        ...(existingId ? { worker_id: existingId } : {}),
      }),
      signal: ctrl.signal,
    });
    clearTimeout(timer);
    if (!resp.ok) {
      wlog("connection", "warn", `claim failed http ${resp.status}, 用本地 id`);
      return;
    }
    const data = await resp.json();
    if (!data?.worker_id) {
      wlog("connection", "warn", "claim 无 worker_id，用本地 id");
      return;
    }
    const patch: Record<string, unknown> = {
      workerId: data.worker_id,
      accountLabel: data.account_label ?? "",
      claimDone: true,
      provisionSource: "claim",
    };
    if (data.provider === "chatgpt" || data.provider === "claude") patch.provider = data.provider;
    await setLocal(patch);
    wlog("connection", "info", "worker claimed identity", {
      worker_id: data.worker_id,
      account_label: data.account_label,
    });
  } catch (e: any) {
    wlog("connection", "warn", `claim 出错：${e?.message ?? e}，用本地 id`);
  }
}

// ---------- connection ----------

let tabId: number | undefined;
let socket: Socket | undefined;
// #2-ext：代际计数。重连竞态下旧 socket 的迟到事件（disconnect/connect_error）可能在
// 新连接已建立后才触发，把 UI 覆盖成 disconnected。每个 handler 比对捕获的 seq + socket 引用，
// 不是当前代就直接忽略。disconnectWS 自增以作废本代。
let socketSeq = 0;

// E6：在途请求 → 它被派发到的 tab。用于丢弃「未登记 / 已完成 / 跨 tab」的流事件，
// 防止迟到 SSE 或并发跳转把上一请求的 chunk 串到下一请求。capacity=1 下每 tab 至多一条在途。
const inflightReqTab = new Map<string, number>();

const isInflight = (reqId?: string, senderTabId?: number): boolean => {
  if (!reqId) return false;
  const expect = inflightReqTab.get(reqId);
  if (expect === undefined) {
    wlog("dispatch", "debug", "drop stream event for unknown/finished req", undefined, reqId);
    return false;
  }
  if (senderTabId !== undefined && senderTabId !== expect) {
    wlog("dispatch", "warn", `drop cross-tab stream event (tab ${senderTabId}≠${expect})`, undefined, reqId);
    return false;
  }
  return true;
};

// ---- worker 健康状态（M3）----
const HEARTBEAT_ALARM = "sub2org_heartbeat";
const RECOVER_ALARM = "sub2org_recover"; // 掉登录后周期探测，人工登录后自动重新入池
const DEFAULT_COOLDOWN_SECS = 3600;
let workerAvailable = true; // false = 掉登录/被风控
let coolingUntil = 0; // 限流冷却到期 unix 秒

const nowSecs = () => Math.floor(Date.now() / 1000);

const sendHeartbeat = () => {
  if (!socket) return;
  if (coolingUntil && nowSecs() >= coolingUntil) coolingUntil = 0; // 到点自动恢复
  socket.emit("heartbeat", {
    free_slots: 1,
    available: workerAvailable,
    cooling_until: coolingUntil,
  });
};

// 上报即时状态（掉登录 / 限流 / 恢复）
const reportStatus = (available: boolean, cooling_until = 0, reason = "") => {
  workerAvailable = available;
  coolingUntil = cooling_until;
  socket?.emit("status", { available, cooling_until, reason });
  wlog("rate_limit", "warn", `status available=${available} cooling_until=${cooling_until} ${reason}`);
  // 掉登录 → 起周期探测（人工登录后自动恢复）；恢复/可用 → 停探测。冷却由心跳自恢复，不在此处理。
  if (!available && reason === "logged_out") {
    chrome.alarms.create(RECOVER_ALARM, { periodInMinutes: 2 });
  } else if (available) {
    chrome.alarms.clear(RECOVER_ALARM);
  }
};

chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name === HEARTBEAT_ALARM) sendHeartbeat();
  else if (alarm.name === RECOVER_ALARM) probeLoginRecovery();
});

// ---- 日志（控制台 + 批量经 socket 上报 sub2org）----
type LogEntry = {
  ts_ms: number;
  category: string;
  level: string;
  message: string;
  req_id?: string;
  fields?: unknown;
};
const LOG_MIN_RANK = 2; // info（trace0 debug1 info2 warn3 error4）
const levelRank = (l: string): number =>
  ({ trace: 0, debug: 1, info: 2, warn: 3, error: 4 } as Record<string, number>)[l] ?? 2;
let logBuffer: LogEntry[] = [];

const wlog = (category: string, level: string, message: string, fields?: unknown, req_id?: string) => {
  console.log(`[${level}][${category}] ${message}`, fields ?? "");
  if (levelRank(level) < LOG_MIN_RANK) return;
  logBuffer.push({ ts_ms: Date.now(), category, level, message, req_id, fields });
  if (logBuffer.length > 200) logBuffer.splice(0, logBuffer.length - 200); // 上限保护
  if (logBuffer.length >= 20) flushLogs();
};

const flushLogs = () => {
  if (!socket || logBuffer.length === 0) return;
  const entries = logBuffer;
  logBuffer = [];
  socket.emit("log", { entries });
};
setInterval(flushLogs, 3000);

const broadcastConnectionStatus = () => {
  chrome.tabs.query({}, (tabs) => {
    tabs.forEach((tab) => {
      if (tab.id) {
        chrome.tabs.sendMessage(
          tab.id,
          { type: "get_connection_status", content: socketConnectionStatus },
          () => chrome.runtime.lastError
        );
      }
    });
  });
};

async function connectWS() {
  if (socket) return; // #2-ext 幂等：已有 socket（在连/已连/socket.io 自动重连中）直接返回，杜绝双 socket
  const mySeq = ++socketSeq; // 幂等检查之后再自增，避免误杀正在连的 socket
  try {
    socketConnectionStatus = { status: "pending", errorMessage: "" };
    broadcastConnectionStatus();

    const cfg = await getWorkerConfig();
    const mySocket = io(cfg.serverBaseUrl, {
      transports: ["websocket"],
      reconnection: true,
      auth: {
        worker_token: cfg.workerToken,
        worker_id: cfg.workerId,
        provider: cfg.provider,
        group: cfg.group, // B2：分组标签
        // E8：account_label 永不为空——未配则由 provider+workerId 末段派生，保证后台可辨识。
        account_label: cfg.accountLabel || `${cfg.provider}-${cfg.workerId.slice(-6)}`,
        capacity: 1, // M3：单机固定 1，靠加机器扩展
      },
    });
    socket = mySocket;
    // 当前代际守卫：本 handler 只在仍是「这一代 + 这个 socket」时才生效。
    const fresh = () => mySeq === socketSeq && socket === mySocket;

    mySocket.on("connect", () => {
      if (!fresh()) return;
      socketConnectionStatus = { status: "connected", errorMessage: "" };
      broadcastConnectionStatus();
      wlog("connection", "info", "connected to sub2org", { worker_id: cfg.workerId });
      sendHeartbeat(); // 连上即报一次
      flushLogs(); // 连上后把缓冲日志推上去
      chrome.alarms.create(HEARTBEAT_ALARM, { periodInMinutes: 0.5 });
    });

    mySocket.on("ready", (d) => {
      if (!fresh()) return;
      wlog("connection", "info", "ready", d);
    });
    mySocket.on("disconnect", () => {
      if (!fresh()) return;
      wlog("connection", "warn", "disconnected from sub2org");
      socketConnectionStatus = { status: "disconnected", errorMessage: undefined };
      broadcastConnectionStatus();
    });
    mySocket.on("connect_error", (err) => {
      if (!fresh()) return;
      handleErrorOnConnect(err);
    });
    mySocket.on("connect_failed", (err) => {
      if (!fresh()) return;
      handleErrorOnConnect(err);
    });

    mySocket.on("dispatch", (msg: Dispatch) => {
      if (!fresh()) return;
      handleDispatch(msg);
    });
    mySocket.on("cancel", (msg: { req_id: string }) => {
      if (!fresh()) return;
      console.log("[BG] cancel (not yet handled)", msg.req_id);
    });
  } catch (e) {
    handleErrorOnConnect(e);
  }
}

const disconnectWS = () => {
  socket?.off(); // 先摘掉所有监听，杜绝迟到事件回灌旧 handler
  socket?.disconnect();
  socket = undefined;
  socketSeq++; // 作废本代所有未及清理的回调
  chrome.alarms.clear(HEARTBEAT_ALARM);
  chrome.alarms.clear(RECOVER_ALARM);
  socketConnectionStatus = { status: "disconnected", errorMessage: undefined };
  broadcastConnectionStatus();
};

// worker tab 是否已离开登录页（= 会话已恢复）。https 且非 login URL 才算。
const looksLoggedIn = (url?: string) => !!url && /^https?:/.test(url) && !isLoginUrl(url);

// 掉登录后周期探测：worker 自身无法登录，仅在人工重新登录后检测到并自动重新入池。
// 关键：绝不冲掉「正在该 tab 前台进行的人工登录」——仅在后台 tab 上重载以拾取会话。
async function probeLoginRecovery() {
  if (workerAvailable) {
    chrome.alarms.clear(RECOVER_ALARM);
    return;
  }
  if (!socket || inflightReqTab.size > 0) return; // 无连接或有在途请求时不打断
  try {
    if (tabId === undefined) {
      // 无 tab（被关掉）→ 建一个后台 tab，下周期再判（先让其加载）。
      const tab = await chrome.tabs.create({ url: await getProviderNewChatUrl(), active: false });
      tabId = tab.id;
      return;
    }
    const t = await chrome.tabs.get(tabId);
    // 已离开登录页（人工已在该 tab 登录完成，或会话已恢复）→ 直接恢复，不重载。
    if (looksLoggedIn(t.url)) {
      reportStatus(true, 0, "recovered");
      wlog("connection", "info", "worker 重新登录，已恢复上线");
      return;
    }
    // 仍在登录页且该 tab 正被前台查看 → 运维可能正在此登录，本周期不动，避免冲掉登录流程。
    if (t.active) return;
    // 后台登录页：安全重载以拾取 profile 级新会话（运维在别处登录的情形）。
    await chrome.tabs.update(tabId, { url: await getProviderNewChatUrl() });
    await waitForTabComplete(tabId);
    await new Promise((r) => setTimeout(r, 2000)); // settle：等可能的重定向到登录页落定，避免外壳页早判
    const t2 = await chrome.tabs.get(tabId);
    if (looksLoggedIn(t2.url)) {
      reportStatus(true, 0, "recovered"); // 清 RECOVER_ALARM 并重新可派发
      wlog("connection", "info", "worker 会话已恢复，重新上线");
    }
  } catch {
    tabId = undefined; // tab 没了 → 下个周期重建
  }
}

const handleErrorOnConnect = (err: any) => {
  console.log("[BG] connect error", err);
  socketConnectionStatus = {
    status: "disconnected",
    errorMessage: err?.message ?? String(err),
  };
  broadcastConnectionStatus();
};

// ---------- dispatch → content tab ----------

// 等待某个 tab 加载完成
const waitForTabComplete = (id: number, timeoutMs = 15000): Promise<void> =>
  new Promise((resolve) => {
    const finish = () => {
      chrome.tabs.onUpdated.removeListener(listener);
      resolve();
    };
    const listener = (updatedId: number, info: chrome.tabs.TabChangeInfo) => {
      if (updatedId === id && info.status === "complete") finish();
    };
    chrome.tabs.onUpdated.addListener(listener);
    setTimeout(finish, timeoutMs);
  });

const sendAskWithRetry = (id: number, msg: Dispatch, retries = 4, delay = 2000) => {
  chrome.tabs.sendMessage(id, { type: "ask_question", content: msg }, () => {
    const err = chrome.runtime.lastError;
    if (err && retries > 0 && err.message?.includes("Receiving end does not exist")) {
      setTimeout(() => sendAskWithRetry(id, msg, retries - 1, delay), delay);
    } else if (err) {
      wlog("error", "error", "ask_question failed", { message: err.message }, msg.req_id);
      inflightReqTab.delete(msg.req_id); // E6：投递失败 → 撤销登记
      socket?.emit("error", {
        req_id: msg.req_id,
        code: "content_unreachable",
        message: err.message,
      });
    }
  });
};

// 登录页/鉴权重定向的特征
const isLoginUrl = (url?: string) =>
  !!url && /\/auth\/login|auth0\.openai\.com|\/log[-_]?in|accounts\.google\.com|\/auth\?/i.test(url);

async function handleDispatch(msg: Dispatch) {
  wlog("dispatch", "info", `dispatch received action=${msg.action}`, { provider: msg.provider }, msg.req_id);
  const provider = (msg.provider as Provider) || (await getProvider());
  const targetUrl =
    msg.action === "continue" && msg.native_conversation_id
      ? getConversationUrl(provider, msg.native_conversation_id)
      : await getProviderNewChatUrl();

  // 确保有一个 tab 并导航到目标窗口
  try {
    if (tabId === undefined) {
      const tab = await chrome.tabs.create({ url: targetUrl, active: false });
      tabId = tab.id;
    } else {
      await chrome.tabs.get(tabId);
      await chrome.tabs.update(tabId, { url: targetUrl });
    }
  } catch {
    tabId = undefined; // tab 没了 → 重建
    return handleDispatch(msg);
  }
  if (tabId === undefined) return;
  await waitForTabComplete(tabId);

  // 掉登录检测：导航后落到登录页 → 上报不可用并让本请求失败
  try {
    const t = await chrome.tabs.get(tabId);
    if (isLoginUrl(t.url)) {
      reportStatus(false, 0, "logged_out");
      socket?.emit("error", { req_id: msg.req_id, code: "logged_out", message: "not logged in" });
      return;
    }
  } catch {
    /* ignore */
  }

  const id = tabId;
  // E6：登记本请求归属的 tab；先清掉该 tab 上的旧 req（capacity=1，上一条已结束），
  // 使其迟到 chunk 因 req_id 不再在册而被 isInflight 丢弃。
  for (const [rid, t] of inflightReqTab) if (t === id) inflightReqTab.delete(rid);
  inflightReqTab.set(msg.req_id, id);
  setTimeout(() => sendAskWithRetry(id, msg), 1500);
}

// ---------- content → sub2org (streaming) ----------

type AgentMessage = { type: string; content?: any };

chrome.runtime.onMessage.addListener((msg: AgentMessage, _sender, sendResponse) => {
  // #3：仅 get_connection_status 需回包——同步应答即可；其余分支全是 fire-and-forget。
  // 旧实现无条件 return true 但多数分支从不 sendResponse → 通道挂到超时 → "message channel closed" 报错。
  if (msg.type === "get_connection_status") {
    sendResponse({ type: "get_connection_status", content: socketConnectionStatus });
    return false; // 已同步回包，无需保持通道开启
  }
  (async () => {
    const senderTabId = _sender.tab?.id;
    switch (msg.type) {
      case "stream_chunk": {
        const reqId = msg.content?.req_id;
        if (!isInflight(reqId, senderTabId)) break; // E6：丢弃串台/迟到 chunk
        socket?.emit("chunk", toWireChunk({ req_id: reqId, delta: msg.content.delta }));
        break;
      }
      case "stream_done": {
        const reqId = msg.content?.req_id;
        if (!isInflight(reqId, senderTabId)) break;
        socket?.emit("done", toWireDone({
          req_id: reqId,
          full: msg.content.full,
          conversation_id: msg.content.conversation_id,
        }));
        inflightReqTab.delete(reqId); // E6：请求结束 → 注销
        break;
      }
      case "worker_log": {
        const c = msg.content || {};
        wlog(c.category || "system", c.level || "info", c.message || "", c.fields, c.req_id);
        break;
      }
      case "worker_status": {
        // loader 检测到限流/掉登录 → 折算并上报
        const c = msg.content || {};
        let cooling = Number(c.cooling_until) || 0;
        if (c.reason === "rate_limited" && !cooling) {
          cooling = nowSecs() + DEFAULT_COOLDOWN_SECS;
        }
        reportStatus(c.available !== false, cooling, c.reason ?? "");
        break;
      }
      case "stream_error": {
        const reqId = msg.content?.req_id;
        if (!isInflight(reqId, senderTabId)) break;
        socket?.emit("error", toWireError({ req_id: reqId, message: msg.content.message }));
        inflightReqTab.delete(reqId); // E6：请求结束 → 注销
        break;
      }
      case "get_worker_config":
        if (senderTabId) {
          chrome.tabs.sendMessage(senderTabId, {
            type: "set_worker_config",
            content: await getWorkerConfig(),
          });
        }
        break;
      case "set_worker_config": {
        const c = msg.content as Partial<WorkerConfig>;
        const patch: Record<string, unknown> = {};
        if (c.serverBaseUrl !== undefined) patch.apiBaseUrl = c.serverBaseUrl;
        if (c.workerToken !== undefined) patch.workerToken = c.workerToken;
        if (c.accountLabel !== undefined) patch.accountLabel = c.accountLabel;
        if (c.provider !== undefined) patch.provider = c.provider;
        if (c.capacity !== undefined) patch.capacity = c.capacity;
        if (c.group !== undefined) patch.group = c.group; // B2
        patch.provisionSource = "manual";
        await setLocal(patch);
        if (socket) {
          disconnectWS();
          connectWS();
        }
        break;
      }
      case "set_provider":
        await setLocal({ provider: msg.content as Provider });
        break;
      case "get_provider":
        if (senderTabId) {
          chrome.tabs.sendMessage(senderTabId, {
            type: "set_provider",
            content: await getProvider(),
          });
        }
        break;
      case "connect":
        connectWS();
        break;
      case "disconnect":
        disconnectWS();
        break;
    }
  })();
  return false; // #3：fire-and-forget 分支，立即关闭通道（不再无条件 return true）
});

// 启动：先吃托管策略 → 领取身份(若有引导) → 确保有稳定 id → 连接。
(async () => {
  await applyManagedPolicy(); // M2：策略下发的 server/token 作为后续步骤的输入
  await ensureClaimedIdentity(); // M4：有引导且未领取 → 集中签发身份
  await getWorkerId(); // 已领则返回领到的 id，否则本地兜底生成
  await connectWS();
})();
