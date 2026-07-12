# sub2org — 中转控制中心（Rust）

把多台「指纹浏览器 + sub2org-worker 扩展 + 登录态 ChatGPT/Claude 账号」组成一个 **worker 池**，
对外暴露 OpenAI 兼容 API。用户请求进来 → 调度一个空闲 worker → 让它在网页 AI 里提问 →
把**逐字流式**结果转发回用户。

**📖 使用文档（部署/接 worker/调 API/运维/排查）：[`docs/USAGE.md`](docs/USAGE.md)**

## 当前线上拓扑

本仓库 `website/` 的 `sub2.org` release 运行在 SFO `108.62.160.202`，由
`sub2org-website.service` 监听 `127.0.0.1:19440`。公网请求先到 aiyunus OpenResty，
再经 WireGuard 转发到 SFO `10.88.0.1:18080`。aiyunus 不保存 website release、源码、
Node 运行时或业务数据。机器事实以
[`vps/ops/sfo`](/Users/wang/code/xiaoji/vps/ops/sfo/README.md) 和
[`vps/ops/aiyunus`](/Users/wang/code/xiaoji/vps/ops/aiyunus/README.md) 为准。

## 总体架构

```
                       多个终端用户 (OpenAI SDK / curl)
                              │  POST /v1/chat/completions
                              │  Authorization: Bearer sk-sub2org-...
                              │  X-Sub2org-Session: <id?>     ← 会话亲和（续接历史窗口）
                              ▼
   ┌─ sub2org / server (Rust，单实例) ──────────────────────────────────────────
   │  http::consumer    OpenAI 兼容入口 + 鉴权 + 会话解析
   │  session           session_id ⇄ (worker, 账号, 网页 conversation_id)
   │  pool::scheduler   挑空闲 worker / 排队 / 亲和路由 / 限流轮换 / 故障转移
   │  ws::gateway       socketioxide：worker 自注册 / 心跳 / dispatch / 流式回传
   │  stream            worker chunk → 用户 SSE 帧 (chat.completion.chunk)
   │  logging           按 端/类型/级别 入库 → /admin/logs
   │  store             session / worker / apikey / logs（内存 或 Postgres）
   └───────▲────────────────────────────────────────────────────────────────────
           │  Socket.IO（worker 主动连入，auth 带 worker_token；chunk 逐字回流）
   ┌───────┼──────────────────────────────┐
   ▼       ▼                              ▼
[云机器A] [云机器B]   ...    每台 = 指纹浏览器 + extension(sub2org-worker) + 1 个账号
指纹浏览器 指纹浏览器          多个 tab = 多个网页 conversation（新窗口 / 历史窗口）
   │
   ▼
ChatGPT 网页 / Claude 网页    ← loader 劫持 window.fetch，逐字解析 SSE 流式回传
```

**请求流转**：用户 `POST` → 鉴权 → 解析/新建 `session` → 调度挑 worker（带 session 则定向续接）
→ `ws::gateway` emit **dispatch** → worker 在网页里提问 → loader 逐字 **chunk** 回传
→ server 包成 `chat.completion.chunk` SSE 流式吐给用户 → **done** 时把 session 钉定落库。

> 核心范式：**room（一对一）→ worker 池 + session 亲和**；请求按 `req_id` 关联（不再按 roomId），
> 一个 worker 可承载多个网页 conversation 窗口。单 sub2org 实例 + 多 worker 机器即可扩展（无需 Redis）。



## 运行

```bash
cd sub2org/server
cargo run            # 读取 ./config.toml，默认监听 0.0.0.0:3300
cargo test           # 单元测试
```

配置见 `config.toml`：`bind`、`worker_tokens`（worker 准入）、`api_keys`（消费方）、
超时、`failover_on_offline`、`database_url`、`[provider_map]`（model 名前缀 → provider）。

启用 Postgres（持久化会话亲和）：起一个 PG，在 `config.toml` 设 `database_url`，
启动时自动跑 `migrations/`。不设则用内存 store（重启丢会话）。

会话亲和用法：

```bash
# 新会话：从响应头拿到 x-sub2org-session
curl -i -X POST localhost:3300/v1/chat/completions \
  -H 'authorization: Bearer sk-sub2org-dev' -H 'content-type: application/json' \
  -d '{"model":"gpt-4o","messages":[{"role":"user","content":"hi"}]}'
# 续接：带上同一个 session，会回到那台机器+账号+历史聊天窗
curl -X POST localhost:3300/v1/chat/completions \
  -H 'authorization: Bearer sk-sub2org-dev' -H 'x-sub2org-session: <上一步的值>' \
  -H 'content-type: application/json' \
  -d '{"model":"gpt-4o","messages":[{"role":"user","content":"接着上文"}]}'
```

admin：`curl localhost:3300/admin/sessions`、`curl -X POST localhost:3300/admin/api-keys -d '{"owner":"alice"}'`。

## 协议（worker ↔ sub2org，Socket.IO）

握手：`io(server, { auth: { worker_token, worker_id, provider, account_label, capacity } })`

| 方向 | 事件 | payload |
|---|---|---|
| S→W | `dispatch` | `{ req_id, session_id, action, native_conversation_id?, provider, prompt }` |
| S→W | `cancel` | `{ req_id }` |
| W→S | `chunk` | `{ req_id, delta }` |
| W→S | `done` | `{ req_id, full_text, native_conversation_id }` |
| W→S | `error` | `{ req_id, code, message }` |
| W→S | `heartbeat` | `{ active_slots, free_slots }` |

## 端到端自测（无需浏览器，用 mock worker）

1. 起服务：`cargo run`
2. 用任意 Socket.IO v4 客户端连入并实现上面协议（参考 `docs/mock-worker.js` 思路：connect→收 dispatch→emit chunk×N→emit done）。
3. 发请求：

```bash
curl -N -X POST localhost:3300/v1/chat/completions \
  -H 'authorization: Bearer sk-sub2org-dev' \
  -H 'content-type: application/json' \
  -d '{"model":"gpt-4o","stream":true,"messages":[{"role":"user","content":"count to 5"}]}'
```

应看到逐帧 `data: {...chat.completion.chunk...}` 直到 `data: [DONE]`。
`curl localhost:3300/admin/workers` 可看在线 worker。

## 真实链路

装 `extension/`（sub2org-worker）扩展到指纹浏览器，设置页填 `sub2org 服务地址` + `Worker Token` +
`账号标签` + `Provider`，点连接；该浏览器需已登录对应 ChatGPT/Claude。之后对 sub2org
发 OpenAI 请求即可由该账号的网页会话作答。

# linuxdo
- [linuxdo](https://linux.do)

## 其它工具
- [中转站 tps 测试 - 模型固有参数，tps不稳一定为假](https://curl0.com/)
