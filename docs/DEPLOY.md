# sub2org 部署运维手册（公开付费 SaaS）

把 sub2org 作为对外付费服务运行。架构：`nginx`(托管 console + 反代 API) → `sub2org`(Rust 中转) → `postgres`；worker（浏览器扩展，登录 ChatGPT/Claude）从各机器主动连入。

## 1. 前置
- 一台公网服务器，装好 Docker + Docker Compose。
- 一个域名解析到该服务器（在线支付回调必须公网可达）。
- 至少 1 台装好 sub2org-worker 扩展、已登录 AI 网页的机器（见 `USAGE.md` B 章）。

## 2. 一键起服务
```bash
cd deploy
cp .env.example .env                       # 填好密钥/域名（见下）
cp ../server/config.example.toml config.toml   # 调 worker_tokens / model_cost / [payment] / [smtp]
docker compose up -d --build
docker compose logs -f sub2org             # 看启动日志：首启会打印 admin 随机密码（若 .env 未设）
```
`.env` 必填：`POSTGRES_PASSWORD`、`SUB2ORG_JWT_SECRET`(≥16 位)、`SUB2ORG_ADMIN_TOKEN`、`PUBLIC_BASE_URL`(https 域名)。
`config.toml` 必改：`worker_tokens`(强随机)、`[model_cost]` 费率、`default_initial_balance`。

> **生产硬化（自动）**：server 以 `SUB2ORG_ENV=prod` 启动，若 jwt_secret 仍是默认、admin_token 为空、含 `sk-sub2org-dev`/`dev-worker-token`、或 database_url 为空，会**拒绝启动**并打印原因。敏感项由 `.env` 注入并覆盖 config 里的占位值。

## 3. 出 TLS 证书（强烈建议）
1. 用 certbot/acme.sh 签发证书，放到 `deploy/certs/{fullchain.pem,privkey.pem}`。
2. 编辑 `deploy/nginx.conf`：解开 80 块的 301 跳转 + 解开底部 443 块（填 server_name）。
3. 编辑 `docker-compose.yml`：解开 web 的 `443:443`。
4. `docker compose up -d web`。

## 4. 首次配置（控制台）
- 浏览器开 `https://<域名>` → 用 admin 登录（用户名 `admin`，密码见启动日志或 `.env`）。
- **Workers**：确认各机器 worker 在线（先按 `USAGE.md` 把扩展指到 `https://<域名>`、填 `worker_token`=config 的 worker_tokens 之一）。
- **Billing**：看近 24h 收入/消耗概览；生成兑换码备用。
- **Users**：手动建用户 / 充值 / 封禁 / 限流。
- 普通用户：注册 → 自助建 api_key → 充值（在线支付或兑换码）→ 用 `sk-sub2org-…` 调 `/v1/chat/completions`。

## 5. 计费（按次 / 按模型固定价）
- `config.toml`：`billing_enabled=true`，`[model_cost]` 配每模型每次扣的积分（最长前缀匹配，fallback `default_cost`）。
- 余额挂用户；预扣 + 失败自动退还。`tokens` 不计（浏览器会话拿不到上游 token 用量，本系统按次计费）。
- 定价对用户可见：`GET /v1/pricing`。

## 6. 在线支付（易支付 / epay）
1. 在易支付商户后台拿到 `pid` / `key` / 网关地址。
2. `config.toml [payment]`：`epay_url`、`enabled_channels`、`credits_per_cny`(积分↔元)、`min/max_amount`；`pid/key` 走 `.env` 的 `SUB2ORG_EPAY_PID/KEY`。
3. 商户后台异步通知地址填 `https://<域名>/payment/notify`，同步跳转 `https://<域名>/payment/return`。
4. 用户在「充值」页选金额+渠道→去支付→回调验签通过后**幂等入账**（重复回调不重复加钱）。
- 不接在线支付：把 `[payment]` 整段删掉/留空 → 仅兑换码 + admin 充值。

## 7. 邮箱找回密码 / 验证（可选，需 SMTP）
- `config.toml [smtp]` 配 host/port/from，密码走 `.env` 的 `SUB2ORG_SMTP_PASSWORD`。
- 找回密码：登录页「忘记密码」→ 收邮件 → `/reset?token=` 改密。
- 邮箱验证：`require_email_verify=true` 时，注册需点验证链接才能消费。未配 SMTP 时相关端点返回"未配置"，不影响主流程。

## 8. 健康检查 / 监控
- `/livez`（存活）、`/readyz`（就绪，pg 模式会 ping 库）、`/healthz`（ok）。compose/反代探针建议用 `/readyz`。
- 运维可观测：控制台 Overview/Workers/Logs/Metrics + Billing（Top 用户消费 / 按模型消费）；`GET /admin/stats/usage`（可加 `?group_by=user|model`）看收入/消耗。
- **Prometheus 指标**：`GET /metrics`（`text/plain; version=0.0.4`，手写导出、零额外依赖）。导出
  `sub2org_workers_online` / `_total` / `_by_status{status=…}` / `requests_inflight` / `requests_1m` / `requests_24h` /
  `worker_slots_in_flight|capacity` / `worker_served_total` / `queue_wait_ms`（有样本才出）/ `logs_dropped_total`。
  鉴权设 `metrics_token`（或 env `SUB2ORG_METRICS_TOKEN`），抓取端用 Bearer：

      scrape_configs:
        - job_name: sub2org
          metrics_path: /metrics
          authorization: { credentials: <metrics_token> }
          static_configs: [{ targets: ['sub2org:3300'] }]

  也可用 `?token=<metrics_token>`。`metrics_token` 留空 = `/metrics` 开放，**必须**用 nginx allowlist 锁住：
  `location = /metrics { allow 10.0.0.0/8; deny all; proxy_pass http://sub2org_api; }`。
- **告警**：`config.toml [alert]`（`enabled` + `webhook_url`/`email_to`）。触发：worker 掉线、错误率超阈（60s 窗 ≥20 样本）、
  用户扣后余额低于 `balance_low_threshold`。webhook 为 HTTP POST JSON（可带 `X-Sub2org-Alert-Token`，走 env `SUB2ORG_ALERT_WEBHOOK_SECRET`）；邮件复用 `[smtp]`。`min_interval_secs` 去重防刷。

## 9. 备份 / 升级 / 回滚
- **备份**（务必上 cron）：
  ```bash
  docker compose exec -T postgres pg_dump -U sub2org sub2org | gzip > backup-$(date +%F).sql.gz
  ```
  恢复：`gunzip -c backup.sql.gz | docker compose exec -T postgres psql -U sub2org sub2org`。
- **升级**：拉新代码 → `docker compose up -d --build`（迁移随启动自动跑，编译期嵌入）。
- **回滚**：切回旧镜像/旧 tag 重启；DB 迁移只增不删，向后兼容。
- **优雅关闭**：`docker compose stop`/SIGTERM → server 停收新连接、让在途请求自然结束再退出。

## 10. 安全清单（上线前过一遍）
- [ ] `.env` 全部 CHANGE_ME 已替换；`.env`/`config.toml`/`certs` 不入库（deploy/.gitignore 已排除）。
- [ ] `SUB2ORG_ENV=prod` 已生效（启动无"配置校验失败"）。
- [ ] TLS 已出证、80→443 跳转开启。
- [ ] `worker_tokens` 为强随机；按需关闭 `allow_registration`。
- [ ] Postgres 端口未对公网暴露（compose 默认仅内网），定期备份已配。
- 注：/auth/* 已有后端 per-IP 限流 + 登录爆破锁定；/worker/claim 独立 per-IP 限流；/v1 全局限流可在 nginx.conf 解开 `limit_req`。

### 安全功能（Track 3）
- **审计日志**：所有 admin 变更（充值/封号/限流/禁 key/建用户/发兑换码/禁用 worker/重置 2FA）写入 `audit_log`，控制台 **可观测 → Audit** 查看（可按操作者/动作过滤）。actor=`jwt:<用户名>` / `token`（X-Admin-Token）/ `internal`（admin_token 为空时）。
- **两步验证 (2FA/TOTP)**：用户在 **账户 → Security** 启用（扫 otpauth 或手填 base32 密钥）。开启后登录走两步（密码 → 动态码）。两步均受**账号级失败锁定**（10 次/10 分钟→锁 5 分钟），动态码无法被暴力枚举。丢设备救援：admin `POST /admin/users/{id}/2fa/reset`（控制台亦可），或用 `X-Admin-Token` 破窗（该路径不校验 2FA）。**注意服务器时钟需 NTP 同步**（TOTP 容差 ±30s）。
- **JWT 主动吊销**：改密 / 封号 / `POST /v1/logout-all`（账户 → Security 的「在所有设备登出」）会令该用户全部旧 token 立即失效。
- **IP allow/deny**：`config.toml [security] ip_allow/ip_deny`（CIDR）。**仅在 nginx 反代后可信**（取 XFF 第一跳）——切勿把 server bind 端口直接暴露公网，否则可被伪造 XFF 绕过。allow 非空=白名单（自动放行回环）；非法 CIDR 启动期拒启。

## 11. 规模化 worker 分发（批量预配 + 自托管 CRX 自动更新）

加 worker = 在 N 台（指纹）浏览器装扩展。逐台手填表单不可扩展，用以下四种预配（详见 `../extension/README.md`）：

1. **URL 参数 / 配置导入导出**（最轻量）：脚本/RPA 打开扩展设置页
   `…/src/pages/settings/index.html?serverBaseUrl=https://<域名>&workerToken=<T>&provider=chatgpt&autosave=1`
   → 零点击保存并连接。或在一台导出 JSON、其余机器「导入配置」。
2. **托管策略 managed storage**：扩展已声明 `storage.managed_schema`。用企业策略 / MDM / 指纹浏览器集中下发
   `serverBaseUrl`/`workerToken`/`provider`/`accountLabel`（**策略为准，每次启动覆盖本地**）。
3. **服务端领取 claim**：首启扩展用共享 `worker_token` 调 `POST https://<域名>/worker/claim` 领取唯一
   `worker_id`+`account_label`，**集中签发身份**——运维只需把 `worker_token`+`serverBaseUrl` 下发一次（URL/策略），标签自动分配。
   - 服务端开关：`config.toml [worker] provision_enabled=true`（默认开），`default_provider` / `label_prefix(→wk-001)`。
   - 鉴权用 `worker_tokens`（与 socket 握手同一套）；独立 per-IP 限流；幂等（带已知 worker_id 重领返回同身份）。
   - 跨域：扩展从后台 service worker 发起、`host_permissions` 覆盖本域，**不受 CORS 限制**，无需改 `allowed_origins`。
4. **自托管 CRX 自动更新（off-store）**：改一次代码全网 worker 自动更新。
   - 打包（在 `extension/`）：首次 `node scripts/pack-crx.mjs` 生成 `keys/sub2org-worker.pem` 并打印
     `SUB2ORG_EXT_KEY` / `SUB2ORG_EXT_UPDATE_URL`；用它们 `SUB2ORG_EXT_KEY=… SUB2ORG_EXT_UPDATE_URL=https://<域名>/ext/updates.xml yarn build:chrome`，
     再 `SUB2ORG_EXT_UPDATE_BASE=https://<域名>/ext node scripts/pack-crx.mjs` → `dist_release/{*.crx,updates.xml}`。
   - 发布：把 `dist_release/sub2org-worker-*.crx` 与 `updates.xml` 放进 `deploy/ext/`（compose 已挂载，nginx 以 `/ext/` 托管）。升级只需 bump `package.json` 版本→重打包→替换文件。
   - **⚠️ 限制**：消费版 Chrome **默认禁止**非商店扩展 off-store 自动更新；只有经**企业策略**强制安装/允许（`ExtensionInstallForcelist` / `ExtensionSettings`，含 `update_url`），或在放宽限制的**指纹/防关联浏览器**上才会触发。各 OS 策略位置：Windows 注册表 `HKLM\Software\Policies\Google\Chrome`、macOS `/Library/Managed Preferences/com.google.Chrome.plist`、Linux `/etc/opt/chrome/policies/managed/*.json`。故 CRX 自动更新实质与「托管策略」配套。
   - 私钥 `keys/*.pem` 已 gitignore，**勿丢失**（丢了扩展 ID 变、更新链断）。

## 不在本轮（后续）
扩展商店上架、订阅套餐、Stripe/海外支付、i18n/公告/设置页、多实例/HA。
