# sub2org 使用文档

把多台「指纹浏览器 + sub2org-worker 扩展 + 登录态 ChatGPT/Claude 账号」组成一个 **worker 池**，
对外暴露 **OpenAI 兼容 API**。用户请求进来 → sub2org 调度一个空闲 worker → 让它在网页 AI 里
提问 → 把**逐字流式**结果转发回用户。无需任何模型 API key。

```
   OpenAI SDK / curl ──HTTP──▶  sub2org (Rust, 单实例)  ──Socket.IO──▶  多台云机器
   Bearer sk-sub2org-...            · 调度/会话亲和/日志              每台 = 指纹浏览器
   X-Sub2org-Session: ...           · Postgres 持久化                + sub2org-worker 扩展
                                                                  + 1 个登录账号
```

三个角色，对号入座：
- **A. 运维**：部署并运行 sub2org 服务（本机/一台服务器，**只需一个实例**）。
- **B. 机器管理员**：在每台云机器的指纹浏览器上装 `sub2org-worker` 扩展、登录账号、连上 sub2org。
- **C. 开发者/消费方**：拿 API key，用 OpenAI SDK 或 curl 调用。

---

## A. 部署 sub2org 服务

### 依赖
- Rust（`cargo`，1.90+）。
- 可选 Postgres（强烈建议生产开启；不开则用内存，**重启丢会话亲和与日志**）。

### 最简启动（dev，无数据库）
```bash
cd sub2org/server
cargo run            # 读 ./config.toml，默认监听 0.0.0.0:3300
```
看到 `sub2org listening on http://0.0.0.0:3300` 即就绪。自测：
```bash
curl localhost:3300/healthz        # → ok
curl localhost:3300/admin/workers  # → {"workers":[],"count":0}
```

### 生产启动（接 Postgres）
1. 准备一个 Postgres 库，例如 `sub2org`。
2. 编辑 `config.toml`，取消注释并填上 `database_url`：
   ```toml
   database_url = "postgres://user:pass@host:5432/sub2org"
   ```
   首次启动会**自动建表**（`migrations/` 下的 0001~0003，含 sessions / workers / api_keys / logs）。
3. 改掉默认凭据（见下）。
4. 编译运行：
   ```bash
   cargo build --release
   ./target/release/sub2org          # 或用 systemd/docker 托管
   ```
   指定别的配置文件：`SUB2ORG_CONFIG=/etc/sub2org/config.toml ./sub2org`

> 进程托管遵守仓库规则：先改配置→落盘→再重启进程→等 `listening` 日志出现再做下一步。

### 必须改的默认值（安全）
`config.toml` 自带的 `worker_tokens=["dev-worker-token"]`、`api_keys=["sk-sub2org-dev"]` 仅供本地测试。
生产请改成你自己的随机串：
- `worker_tokens`：**worker 准入令牌**（所有云机器共用这个 bootstrap token 自注册）。
- `api_keys`：消费方 key（也可启动后用 admin 接口动态签发，见 D 节）。

---

## B. 接入一台 worker 机器

每台云机器 = 一个指纹浏览器 + 一份扩展 + 一个账号 = 池里一个 worker。**加机器即扩容**。

1. 构建扩展（一次即可，产物可复制到各机器）：
   ```bash
   cd sub2org/extension
   yarn install
   yarn build:chrome      # 产物在 dist_chrome/
   ```
2. 在该机器的指纹浏览器里加载扩展：`chrome://extensions` → 打开「开发者模式」→「加载已解压的扩展程序」→ 选 `dist_chrome`。
3. 浏览器里**登录**对应的 ChatGPT / Claude 账号。
4. 扩展安装后会自动打开设置页（或点扩展图标进入），填：
   | 字段 | 说明 |
   |---|---|
   | **sub2org 服务地址** | 如 `http://你的服务器:3300` |
   | **Worker Token** | 必须等于 sub2org `config.toml` 的 `worker_tokens` 之一 |
   | **账号标签** | 便于在控制台区分，如 `chatgpt-plus-01` |
   | **Provider** | ChatGPT / Claude |
   并发槽位固定为 1（一台一次处理一个对话，靠加机器扩展）。
5. 点「保存并重连」。连接成功后，在服务端能看到它：
   ```bash
   curl http://你的服务器:3300/admin/workers
   ```
   该机器会以一个稳定的 `worker_id`（首次自动生成、持久化）出现在列表里，`online:true`。

> 多机器：每台重复 2~5。worker_token 都用同一个，`账号标签` 各不相同即可。

---

## C. 调用 API（消费方）

OpenAI 兼容。base_url 指到 `/v1`，api_key 用你的 `sk-sub2org-...`。

> **base_url 怎么填**：经 nginx 部署（生产）时填**对外域名**，即 `https://<域名>/v1`——不要填容器内部端口 `:3300`（那是后端直连口，未对外）。仅本机/调试直连后端时才用 `http://<host>:3300/v1`。下文示例用 `:3300` 演示直连，生产请换成你的域名。控制台「API Keys → 接入指南」卡片会直接显示当前可用的 Base URL，可一键复制。

### curl（流式）
```bash
curl -N http://<host>:3300/v1/chat/completions \
  -H 'authorization: Bearer sk-sub2org-dev' \
  -H 'content-type: application/json' \
  -d '{"model":"gpt-4o","stream":true,"messages":[{"role":"user","content":"用一句话介绍你自己"}]}'
```
返回标准 SSE：逐帧 `data: {chat.completion.chunk}`，以 `data: [DONE]` 收尾。
非流式去掉 `"stream":true`，返回单个 `chat.completion` JSON。

### OpenAI SDK（Python）
```python
from openai import OpenAI
client = OpenAI(base_url="http://<host>:3300/v1", api_key="sk-sub2org-dev")
r = client.chat.completions.create(
    model="gpt-4o",
    messages=[{"role": "user", "content": "hello"}],
    stream=True,
)
for chunk in r:
    print(chunk.choices[0].delta.content or "", end="")
```

### OpenAI SDK（Node）
```js
import OpenAI from "openai";
const client = new OpenAI({ baseURL: "http://<host>:3300/v1", apiKey: "sk-sub2org-dev" });
const r = await client.chat.completions.create({
  model: "gpt-4o",
  messages: [{ role: "user", content: "hello" }],
});
console.log(r.choices[0].message.content);
```

### model → provider 映射
`model` 名决定走哪类账号（`config.toml` 的 `[provider_map]` 前缀匹配）：`gpt-*` / `o1` / `o3` → ChatGPT，`claude*` → Claude。也可用请求头 `X-Sub2org-Provider: chatgpt|claude` 覆盖。

### 会话亲和（连续对话回到同一账号的同一聊天窗）
| 头 | 作用 |
|---|---|
| 无 `X-Sub2org-Session` | 新建会话：调度一个空闲 worker **开新聊天窗**；**响应头**返回 `X-Sub2org-Session: <id>` |
| `X-Sub2org-Session: <id>` | 续接：路由回该会话**钉住的同一台机器+账号+网页历史窗口**，只把最新一条消息发进去 |
| `X-Sub2org-New-Window: true` | 同账号**另开新窗**（不接历史） |

典型用法：第一次请求从响应头拿到 `X-Sub2org-Session`，之后每次带上它，就能在同一个网页对话里连续追问。

```bash
# 1) 新会话，拿 session（-i 看响应头）
curl -i http://<host>:3300/v1/chat/completions \
  -H 'authorization: Bearer sk-sub2org-dev' -H 'content-type: application/json' \
  -d '{"model":"gpt-4o","messages":[{"role":"user","content":"记住数字 42"}]}'
#    响应头里有： x-sub2org-session: sess-xxxxx

# 2) 续接同一会话
curl http://<host>:3300/v1/chat/completions \
  -H 'authorization: Bearer sk-sub2org-dev' -H 'x-sub2org-session: sess-xxxxx' \
  -H 'content-type: application/json' \
  -d '{"model":"gpt-4o","messages":[{"role":"user","content":"我刚让你记的数字是多少？"}]}'
```
SDK 里用 `extra_headers={"X-Sub2org-Session": "sess-xxxxx"}`（Python）/ `{ headers: {...} }`（Node）。

---

## D. 运维 / Admin 接口

> 当前 admin 接口未单独鉴权，请只在内网/受信网络暴露，或自行加反代鉴权。

### Worker 池
```bash
curl http://<host>:3300/admin/workers
```
返回每台机器：`online`、`status`(idle/busy/cooling/logged_out/disabled/offline)、`in_flight`、`free`、
`cooldown_until`、`last_seen_at`、`total_served`、`account_label`。**离线但曾注册的机器也会列出**（一眼看全所有机器）。

排空/恢复某台机器（维护时用）：
```bash
curl -X POST http://<host>:3300/admin/workers/<worker_id>/disable   # 不再派发新请求
curl -X POST http://<host>:3300/admin/workers/<worker_id>/enable
```

### 会话
```bash
curl http://<host>:3300/admin/sessions          # 列出
curl http://<host>:3300/admin/sessions/<id>     # 单个：看钉在哪台机器/哪个网页 conversation
```

### 签发 API key（需 Postgres 才持久）
```bash
curl -X POST http://<host>:3300/admin/api-keys \
  -H 'content-type: application/json' -d '{"owner":"alice"}'
# → {"api_key":"sk-sub2org-xxxxxxxx","owner":"alice"}   仅此一次返回，请保存
```

### 日志查询（见下一节）
`GET /admin/logs`

---

## E. 日志系统

按 **端 (source)** 和 **类型 (category)** 分类入库，可过滤查询。
- 端：`server`（sub2org 自身）/ `worker`（各机器扩展，带 `worker_id`）。
- 类型：`connection / dispatch / stream / scheduler / auth / rate_limit / http / dom / system / error`。
- 级别：`trace/debug/info/warn/error`，默认 `info` 及以上入库（`log_min_level` 可配）。
- 存储：有 `database_url` → Postgres `logs` 表（按 `log_retention_days` 自动清理）；无 → 内存环形缓冲（重启即失，仅供临时查看）。
- 设计上**绝不阻塞中转**：投递队列满了会丢弃并在查询结果里返回 `dropped` 计数。

查询（全部为可选 query 参数，组合过滤）：
```bash
# 某台机器最近 50 条
curl "http://<host>:3300/admin/logs?source=worker&worker_id=<id>&limit=50"
# 只看错误
curl "http://<host>:3300/admin/logs?level=error"
# 追踪某次请求全链路（server+worker 同一 req_id）
curl "http://<host>:3300/admin/logs?req_id=chatcmpl-xxxx"
# 某类型 + 时间窗（unix 毫秒）
curl "http://<host>:3300/admin/logs?category=dispatch&since=1730000000000&until=1730003600000"
```
参数：`source / worker_id / category / level(≥该级别) / req_id / session_id / since / until / limit(默认200,上限1000) / offset`。
返回：`{ logs:[...], count, dropped }`，按时间倒序。接 Grafana/脚本即可。

---

## E2. 用户 / 计费 / 充值

引入用户体系后：**用户**有余额，**api_key 归属用户**，每成功提问按 model 费率从用户余额扣 credits（`billing_enabled=true` 时）。三类鉴权：

| 面向 | 凭证 |
|---|---|
| 消费 API `/v1/chat/completions` | `Authorization: Bearer <api_key>` |
| 用户 dashboard `/auth/*`、`/v1/balance|keys|redeem|transactions` | 登录拿 **JWT**，`Authorization: Bearer <jwt>` |
| admin `/admin/*` | `X-Admin-Token: <config.admin_token>`（或 admin 角色 JWT） |

> 默认 `billing_enabled=false`（不计费，行为同以前）。要计费：config 设 `billing_enabled=true`、`default_cost`、可选 `[model_cost]` 费率表。

### 用户自助（dashboard）
```bash
# 注册（config.allow_registration=true 时）→ 返回 jwt
curl -X POST $U/auth/register -H 'content-type: application/json' -d '{"username":"alice","password":"secret123"}'
# 登录
curl -X POST $U/auth/login -H 'content-type: application/json' -d '{"username":"alice","password":"secret123"}'   # → {"jwt":"..."}
JWT=...
curl $U/v1/balance      -H "authorization: Bearer $JWT"      # 余额 + 最近流水
curl $U/v1/transactions -H "authorization: Bearer $JWT"      # 全部流水
curl -X POST $U/v1/keys -H "authorization: Bearer $JWT" -d '{"name":"my-key"}'   # 建自己的 api_key（raw 仅返回一次）
curl $U/v1/keys -H "authorization: Bearer $JWT"              # 列出自己的 key
curl -X POST $U/v1/keys/<id>/disable -H "authorization: Bearer $JWT"
curl -X POST $U/v1/redeem -H "authorization: Bearer $JWT" -d '{"code":"SUB2ORG-XXXX-XXXX"}'  # 兑换码充值
```
之后用建好的 api_key 调 `/v1/chat/completions`；余额不足返回 **402**。

### admin 运营（X-Admin-Token）
```bash
A='x-admin-token: <admin_token>'
curl $U/admin/users -H "$A"                                  # 列用户(含余额/状态)
curl -X POST $U/admin/users -H "$A" -d '{"username":"bob","password":"p","role":"user","initial_balance":100}'
curl -X POST $U/admin/users/<id>/recharge  -H "$A" -d '{"amount":100,"note":"充值"}'   # 充值
curl -X POST $U/admin/users/<id>/ban       -H "$A"          # 封禁（其名下所有 key → 403）
curl -X POST $U/admin/users/<id>/unban     -H "$A"
curl -X POST $U/admin/users/<id>/rate-limit -H "$A" -d '{"n":5}'   # 用户级并发上限（0=不限）
curl -X POST $U/admin/users/<id>/keys      -H "$A" -d '{"name":"k"}'  # 替用户建 key
curl -X POST $U/admin/api-keys/<id>/disable -H "$A"          # 禁用某个 key
curl -X POST $U/admin/redeem-codes -H "$A" -d '{"amount":50,"count":10}'  # 批量生成兑换码
curl $U/admin/redeem-codes -H "$A"
```

### 计费规则
- 扣费 = **预扣 + 失败退还**：请求进来先原子预扣 `cost`（不足 402）；生成成功 → 落账；worker 报错/超时/断连 → 自动退还（流水里能看到 `charge` 后跟 `refund`）。
- `cost = cost_for_model(model)`：`[model_cost]` 前缀最长匹配，否则 `default_cost`。
- 余额挂在**用户**，名下多个 key 共用。
- 首启自动建 admin（`admin_username`/`admin_password`，密码空则随机生成并在启动日志打印一次）；config 的 `api_keys` 种子 key 归属该 admin。
- **在线支付（支付宝/Stripe）本期未做**（`payment_orders` 表已预留）；充值用 admin 充值或兑换码。

## F. 配置参考（config.toml）

| 字段 | 默认 | 说明 |
|---|---|---|
| `bind` | `0.0.0.0:3300` | 监听地址端口 |
| `database_url` | （注释） | Postgres 连接串；不填=内存模式（重启丢会话/日志） |
| `worker_tokens` | `["dev-worker-token"]` | worker 准入 token（共享 bootstrap），**生产必须改** |
| `api_keys` | `["sk-sub2org-dev"]` | 消费方 key，**生产必须改**（或用 admin 签发） |
| `failover_on_offline` | `false` | 续接会话的 worker 离线时：true=换机开新窗，false=报 409 |
| `queue_wait_secs` | `120` | 无空闲 worker 时请求最长排队等待 |
| `generation_secs` | `180` | 单次生成最长等待 |
| `heartbeat_stale_secs` | `90` | 心跳超时即判定 worker 离线、剔除 |
| `default_cooldown_secs` | `3600` | 限流恢复时间解析不出时的兜底冷却 |
| `log_to_db` | `true` | 是否写库（false=只控制台+内存环形） |
| `log_min_level` | `info` | 入库最低级别 |
| `log_retention_days` | `14` | 日志保留天数，超期自动清理 |
| `log_channel_capacity` | `10000` | 日志投递队列容量（满了丢+计数） |
| `admin_token` | `""` | `/admin/*` 的 `X-Admin-Token`；空=不校验（仅内网），**生产必设** |
| `jwt_secret` | `dev-secret-change-me` | 用户 JWT 密钥，**生产必改** |
| `allow_registration` | `true` | 是否允许 `/auth/register` 自助注册 |
| `admin_username`/`admin_password` | `admin`/`""` | 首启 bootstrap 的 admin；密码空=随机生成打印一次 |
| `billing_enabled` | `false` | 计费总开关 |
| `default_cost` | `1` | 每次提问默认扣 credits |
| `default_initial_balance` | `0` | 新用户初始余额 |
| `[model_cost]` | 空 | model 名前缀 → 每次扣 credits（最长前缀匹配） |
| `[provider_map]` | gpt-/o1/o3→chatgpt, claude→claude | model 名前缀 → provider |

---

## G. 故障排查

| 现象 | 含义 / 处理 |
|---|---|
| HTTP 401 | 缺少或无效 `Authorization: Bearer`。检查 key 是否在 `api_keys` 或已签发 |
| HTTP 503 `no available worker ...` | 该 provider 当前无可派发 worker（全忙/冷却/掉登录/离线）。响应里 `earliest worker recovers at unix ...` 给出最早恢复时间。加机器或等冷却 |
| HTTP 409 `session worker offline` | 续接的会话其 worker 离线，且 `failover_on_offline=false`。开启故障转移，或新开会话 |
| HTTP 502 `worker_error` / 504 `generation timeout` | worker 侧报错 / 生成超过 `generation_secs`。看 `/admin/logs?req_id=...` 定位 |
| `/admin/workers` 看不到某机器 | 检查扩展设置里的服务地址、worker_token 是否匹配；浏览器是否登录；看扩展后台 console 与 `/admin/logs?source=worker` |
| 该机器 `status=cooling` | 账号被限流，到 `cooldown_until` 自动恢复；期间请求自动轮到别的账号 |
| 该机器 `status=logged_out` | 账号掉登录/被风控：在途请求立即 502（不再干等超时），并触发 `worker_logged_out` 告警（若配 `[alert]`）。去该浏览器重新登录，worker 每 ~2min 自动探测，登录后自动重新上线 |
| 重启后用户/API key/会话/计费丢失（客户端如 Cherry 报「key 失效需换新」） | 内存模式（没接 Postgres）重启清空所有数据。控制台 Overview 顶部会有内存模式警示。对外运营务必配 `database_url`（Postgres）。注：`config.toml` 的 `api_keys` 每次启动会重新种入，故那些 key 不丢；dashboard 创建的 key 会丢 |

> worker 侧的「限流/掉登录」检测依赖网页 DOM 与接口特征，ChatGPT/Claude 改版后可能需要在
> `sub2org-worker` 的 `content/loader.ts` 调整选择器/状态码判断。

---

## H. 端到端自测（无需真实浏览器）

用可控 mock worker 跑通中转 + 池行为：
```bash
# 终端1：起服务
cd sub2org/server && cargo run

# 终端2：装依赖并起 mock worker（模拟一台机器）
cd sub2org/docs && npm i socket.io-client@4
WORKER_ID=mock-a ACCOUNT=acct-a node mock-worker.js

# 终端3：发请求
curl -N http://localhost:3300/v1/chat/completions \
  -H 'authorization: Bearer sk-sub2org-dev' -H 'content-type: application/json' \
  -d '{"model":"gpt-4o","stream":true,"messages":[{"role":"user","content":"hi"}]}'
```
mock worker 支持用提问里的标记词模拟状况（见 `docs/mock-worker.js` 顶部注释）：`COOL`（限流）、
`LOGOUT`（掉登录）、`DROP`（崩溃断连）。配合 `/admin/workers`、`/admin/logs` 观察调度/状态/日志。
