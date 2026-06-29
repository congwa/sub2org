# sub2org console

sub2org 的 Web 控制台（React + Vite + TypeScript + Tailwind v4）。一个角色路由 SPA：
- **admin** 登录 → 全控制台：Overview（KPI/worker 池/吞吐/实时请求流）、Workers、Sessions、Logs、Metrics、Users、Billing。
- **普通用户** 登录 → 自助面板：余额/流水、API Keys、兑换码充值。

实时数据来自后端 `GET /admin/metrics`（KPI/吞吐）与 `GET /admin/events`（SSE 实时事件，fetch-stream 订阅，断线自动降级轮询）。

## 开发

```bash
# 1) 先起后端（server/），建议设置 admin 账号便于登录：
#    config.toml: admin_username="admin" admin_password="admin123"（密码空则首启随机打印一次）
cd ../server && cargo run        # 监听 :3300

# 2) 起前端 dev（Vite proxy 已把 /v1 /auth /admin /healthz 代理到 :3300，免 CORS）
cd ../console && npm install && npm run dev   # http://localhost:5300
```

打开 http://localhost:5300 → 用 admin 账号登录看控制台；注册/普通用户登录看自助面板。

## 构建

```bash
npm run build      # 产物在 dist/，静态托管或交给反代/后端 static 路由
```

后端地址：默认同源（dev 走 proxy）。如需指向独立后端，设 `VITE_API_BASE_URL=https://gateway.example.com`（此时后端需开 CORS）。

## 结构
```
src/
  api/        client(fetch封装/鉴权/错误分类) · types · errors · sse(实时) · endpoints
  lib/        auth(JWT/storage/Provider/guards) · cn · format
  components/ ui(shadcn 风格基础件) · common(KpiCard/StatusBadge/DataTable/Sparkline…) · Logo
  app/        AppShell(深色侧栏+顶栏) · router(角色路由)
  hooks.ts    TanStack Query 查询/变更 + useAdminEvents(SSE)
  routes/     admin/* · user/* · Login
  globals.css 设计 token（ink/amber + 语义色，light 主区 / dark 侧栏）
```
配色/标识：ink `#13161D` + amber `#F5A73E`，logo 为 3 节点→网关→出射光束。
