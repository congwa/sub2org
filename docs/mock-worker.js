// 可控 mock worker：验证 sub2org 中转 + M3 池行为（无需真实浏览器）。
//
//   npm i socket.io-client@4
//   WORKER_ID=mock-a ACCOUNT=acct-a node mock-worker.js
//
// 通过「提问里的标记词」模拟各种状况（便于脚本驱动）：
//   prompt 含 COOL   → 模拟限流：上报 cooling(now+60) + error，本请求失败
//   prompt 含 LOGOUT → 模拟掉登录：上报 available:false + error
//   prompt 含 DROP   → 模拟崩溃：直接断开 socket（触发服务端快速失败）
//   其它             → 正常逐字流式回复
const { io } = require("socket.io-client");

const SERVER = process.env.SUB2ORG || "http://localhost:3300";
const WID = process.env.WORKER_ID || "mock-1";
const ACCOUNT = process.env.ACCOUNT || "test-account";
const TOKEN = process.env.WORKER_TOKEN || "dev-worker-token";

const socket = io(SERVER, {
  transports: ["websocket"],
  auth: { worker_token: TOKEN, worker_id: WID, provider: "chatgpt", account_label: ACCOUNT, capacity: 1 },
});

let localAvail = true;
let localCooling = 0; // unix 秒
const now = () => Math.floor(Date.now() / 1000);

const heartbeat = () => {
  if (localCooling && now() >= localCooling) localCooling = 0; // 到点自恢复
  socket.emit("heartbeat", { free_slots: 1, available: localAvail, cooling_until: localCooling });
};
setInterval(heartbeat, 5000);

const sendLog = (category, level, message, req_id) =>
  socket.emit("log", { entries: [{ ts_ms: Date.now(), category, level, message, req_id }] });

socket.on("connect", () => {
  console.log(`[${WID}] connected ${socket.id}`);
  sendLog("connection", "info", `mock worker ${WID} up`);
});
socket.on("ready", (d) => console.log(`[${WID}] ready`, d));
socket.on("connect_error", (e) => console.log(`[${WID}] connect_error`, e.message));

socket.on("dispatch", (msg) => {
  const p = msg.prompt || "";
  console.log(`[${WID}] dispatch ${msg.req_id} action=${msg.action} prompt=${JSON.stringify(p)}`);
  sendLog("dom", "info", `received dispatch action=${msg.action}`, msg.req_id);

  if (p.includes("COOL")) {
    const until = now() + 60;
    localCooling = until;
    socket.emit("status", { available: true, cooling_until: until, reason: "rate_limited" });
    socket.emit("error", { req_id: msg.req_id, code: "rate_limited", cooling_until: until, message: "rate limited" });
    console.log(`[${WID}] -> COOLING until ${until}`);
    return;
  }
  if (p.includes("LOGOUT")) {
    localAvail = false;
    socket.emit("status", { available: false, cooling_until: 0, reason: "logged_out" });
    socket.emit("error", { req_id: msg.req_id, code: "logged_out", message: "logged out" });
    console.log(`[${WID}] -> LOGGED OUT`);
    return;
  }
  if (p.includes("DROP")) {
    console.log(`[${WID}] -> DROP (disconnect)`);
    socket.disconnect();
    return;
  }

  const reply = `[${WID}] echo: ${p}`;
  let i = 0;
  const t = setInterval(() => {
    if (i < reply.length) socket.emit("chunk", { req_id: msg.req_id, delta: reply[i++] });
    else {
      clearInterval(t);
      socket.emit("done", { req_id: msg.req_id, full_text: reply, native_conversation_id: `conv-${WID}` });
    }
  }, 8);
});
