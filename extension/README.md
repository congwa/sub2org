# sub2org-worker

`sub2org` 中转控制中心的 **worker 扩展**。装到一台（指纹）浏览器上，把这台浏览器里
登录态的 ChatGPT / Claude 接入 sub2org 的 worker 池——sub2org 派发提问，本扩展在网页 UI
里提问并把**逐字流式**结果回传。

> 复用成熟的 MV3 + Vite 扩展脚手架与 loader 劫持 fetch 的技巧；
> 连接层、消息协议、会话窗口管理为 worker 模型。

> 本扩展是 sub2org 项目的一部分（位于 `sub2org/extension/`）。完整使用文档见 `../docs/USAGE.md` 的 B 节。

## 关键设计

- 身份从「随机 roomId 一对一」改为 **worker 身份**（`worker_id` + `worker_token` 握手鉴权），主动连入 sub2org。
- `serverMessage`/`clientResponse` → **`dispatch` / `chunk`+`done`+`error`**（按 `req_id`、支持流式、可并发）。
- loader 从「答完只发最终 JSON」→ **解析到增量就 postMessage 流式 chunk**，并捕获网页 `conversation_id`（会话亲和用）。
- 设置页改为 **worker 配置面板**：sub2org 地址、worker token、账号标签、provider、并发槽位。
- 不再要求 AI 返回「JSON 格式块」，chat 路径直接流式回传自然语言。

## 构建 / 安装

```bash
cd sub2org/extension
yarn install
yarn build:chrome      # → dist_chrome
```

Chrome：`chrome://extensions` → 开发者模式 → 加载 `dist_chrome`。

## 配置

安装后会自动打开设置页（或点扩展进入）。填写：

- **sub2org 服务地址**：如 `http://localhost:3300`
- **Worker Token**：需与 sub2org `config.toml` 的 `worker_tokens` 之一匹配
- **账号标签**：便于控制台区分多账号，如 `chatgpt-plus-01`
- **Provider**：ChatGPT / Claude
- **并发槽位**：建议 1

点「保存并重连」。确保本浏览器**已登录**对应账号。连接成功后，sub2org 的
`GET /admin/workers` 能看到这个 worker。

## 规模化预配（批量部署 worker）

逐台手填表单不可扩展。本扩展支持四种预配方式，目标是「N 台机器零点击上线」：

1. **URL 参数 + 配置导入导出**：打开
   `chrome-extension://<id>/src/pages/settings/index.html?serverBaseUrl=https://x&workerToken=T&provider=chatgpt&autosave=1`
   即自动保存并连接（`autosave=1`）；省去 `autosave` 则仅预填。设置页还有「导出/导入配置」按钮（导出不含 `worker_id`，每台机器各自唯一）。
2. **托管策略 managed storage**：manifest 已声明 `storage.managed_schema`（`schema.json`）。
   用企业策略 / MDM / 指纹浏览器集中下发 `serverBaseUrl`/`workerToken`/`provider`/`accountLabel`。
   **策略为准——每次启动覆盖本地**，fleet 不漂移。
3. **服务端领取 claim**：首启若已有引导（server+token，来自 URL/策略）且未分配标签，扩展会
   `POST <server>/worker/claim` 用共享 `worker_token` 领取唯一 `worker_id` + `account_label`，
   由服务端集中签发；失败软降级到本地自生成 id（worker 仍能连）。
4. **CRX 自托管自动更新**：`yarn release:chrome` → `dist_release/{*.crx, updates.xml}`，自托管后改一次代码全网更新。
   见下「自动更新限制」。

### CRX 自托管自动更新限制（务必知悉）

> 消费版 Chrome **默认禁止**非 Web Store 扩展的 off-store `update_url` 自动更新。
> 只有在以下环境才会真正触发：
> - 经**企业策略** `ExtensionInstallForcelist` / `ExtensionSettings`（`installation_mode: force_installed` + `update_url`）强制安装/允许的机器；
> - 或放宽该限制的**指纹/防关联浏览器**、以策略运行的 Chromium。
>
> 普通 Chrome 上 `.crx` 只能「装一次」，自动更新静默不跑。因此 **CRX 自动更新实质与「托管策略」配套**。
> 打包：`node scripts/pack-crx.mjs`（首次生成 `keys/sub2org-worker.pem` 并打印需设置的
> `SUB2ORG_EXT_KEY` / `SUB2ORG_EXT_UPDATE_URL`，用它们重新 `build:chrome` 后再 `pack:crx`）。
> 私钥 `keys/*.pem` 已 gitignore，**勿入库、勿丢失**（丢了 = 扩展 ID 变、更新链断）。

## 注意

- 每台指纹浏览器装一份 = 池里一个独立 worker（身份用 claim 集中签发，或手填不同标签）。
- M1：ChatGPT 链路为主；Claude loader 已按相同流式协议改造但未深度实测。
- 会话亲和（历史窗口续接 `/c/<id>`）、多 tab 并发为 M2/M3。
