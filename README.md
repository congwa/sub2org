# sub2org — 中转控制中心（Rust）

把多台「指纹浏览器 + sub2org-worker 扩展 + 登录态 ChatGPT/Claude 账号」组成一个 **worker 池**，
对外暴露 OpenAI 兼容 API。用户请求进来 → 调度一个空闲 worker → 让它在网页 AI 里提问 →
把**逐字流式**结果转发回用户。

**📖 使用文档（部署/接 worker/调 API/运维/排查）：[`docs/USAGE.md`](docs/USAGE.md)**

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

## 当前进度：M1–M3 + 日志系统 已完成

M1（单实例 / 内存池 / 真流式）：
- ✅ axum + socketioxide 服务，worker 经 Socket.IO 握手（auth 带 `worker_token`）接入
- ✅ worker 的 `chunk`/`done` 实时转成 `chat.completion.chunk` SSE（含 `finish_reason:"stop"` + `[DONE]`）
- ✅ `GET /admin/workers` 在线/负载、`GET /healthz`

M2（会话亲和 + 调度 + 持久化）：
- ✅ 会话亲和：`X-Sub2org-Session`（无则新建并在响应头回传；`X-Sub2org-New-Window: true` 同账号开新窗）
- ✅ 续接路由到钉定的 worker + 网页 `conversation_id`，prompt 只发最新一条 user 消息
- ✅ 调度：每 worker Semaphore 控并发槽位 + 排队等待（`queue_wait_secs`）+ 负载均衡选 worker
- ✅ worker 离线策略：`failover_on_offline`（true 转移 / false 报 409）
- ✅ 持久化：`Store` 抽象，内存（dev）/ Postgres（sqlx 运行期查询 + 启动跑 migrations）；会话重启不丢
- ✅ 鉴权走 store；admin：`POST /admin/api-keys`、`GET /admin/sessions[/:id]`

M3（多 worker 机器规模化，单 sub2org + 多云机器）：
- ✅ worker **自注册入库**（共享 bootstrap token，首连自动写 `workers` 表）
- ✅ worker **状态模型** idle/busy/cooling/logged_out/disabled，调度自动跳过+轮换+到点恢复
- ✅ **心跳保活** + 过期剔除（`heartbeat_stale_secs`）+ **断连快速失败**（在途请求秒级失败，不等生成超时）
- ✅ **故障转移**（`failover_on_offline`）；全员不可用 → 503 带恢复时间
- ✅ admin：`GET /admin/workers`（合并在线+DB离线、状态/负载/统计）、`POST /admin/workers/{id}/disable|enable`
- ✅ 扩展端：心跳(chrome.alarms)、429/401 限流/掉登录检测上报、capacity 锁 1（机制就绪，选择器/阈值需按现网调）
- ❌ M4：Redis 多实例（不做，已确认 sub2org 单实例）

日志系统（按 端/类型/级别 分类入库）：
- ✅ 两个端：`server`(sub2org 自身，经 tracing Layer 自动采集) + `worker:<id>`(各云机器扩展经 Socket.IO `log` 事件批量上报，worker_id 由服务端按连接身份打)
- ✅ 10 类 category(connection/dispatch/stream/scheduler/auth/rate_limit/http/dom/system/error) + 级别过滤(默认 info 入库)
- ✅ 存储：Postgres `logs` 表(`0003_logs.sql`)；dev 无 DB 用内存环形缓冲;14 天自动清理
- ✅ 非阻塞：bounded channel + try_send(满了丢+dropped 计数) + 批量异步 flush;DB 挂降级控制台;只采集本 crate target 防反馈环
- ✅ 查询：`GET /admin/logs?source/worker_id/category/level/req_id/session_id/since/until/limit/offset`

用户体系 + 计费 + 鉴权（`server/src/account/`）：
- ✅ 用户（余额挂用户）+ api_key 归属用户 + 状态；argon2 密码 + JWT 自助登录 dashboard
- ✅ 计费：按 model 费率 **预扣 + 失败退还**、流水（充值/扣费/退还/兑换）；`billing_enabled` 默认 false
- ✅ 充值：admin 充值 + 兑换码（在线支付预留）；消费方 `GET /v1/balance` 自查
- ✅ 用户管理：封禁/解封、禁用/启用单 key、用户 CRUD、用户级并发限流；`/admin/*` 加 `X-Admin-Token` + 角色鉴权
- ✅ 双后端（内存/Postgres）端到端验证；首启自动 bootstrap admin
- ⏳ 在线支付网关 + dashboard 前端 UI（后续）

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
