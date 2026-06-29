# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

**sub2org** turns a pool of browser-driven ChatGPT/Claude web sessions (running on fingerprint browsers across many cloud machines) into one **OpenAI-compatible relay API** — no model API keys. A consumer hits `/v1/chat/completions`; sub2org's scheduler picks an idle **worker** (a browser extension), dispatches the prompt over Socket.IO, the worker types it into the web UI and streams the answer back token-by-token. On top sit a user/billing/auth system and a web console.

This repo (`congwa/sub2org`) is one git repo with **three sub-projects**:

- `server/` — the relay control center (**Rust**: axum + socketioxide + sqlx/Postgres).
- `extension/` — the **worker** browser extension (MV3, React+Vite), `sub2org-worker`.
- `console/` — the web **console** (React 19 + Vite + Tailwind v4).
- `docs/USAGE.md` — full operator/consumer guide; `docs/mock-worker.js` — a controllable mock worker for testing without a real browser.

`README.md` has the architecture diagram and milestone status (M1–M3 + logging + accounts/billing + console all done; M4/Redis intentionally dropped — single instance).

## Commands

Commit messages are enforced by a `prepare-commit-msg` git hook: the **first line must match** `{需求|任务|task|feat|feature|fix|bug}#{id}{sep}{msg}`, e.g. `feat#sub2org: 描述`. Other formats are rejected. End commit/PR bodies with the `Co-Authored-By` line.

### server (`cd server`) — Rust
```bash
cargo run                 # reads ./config.toml; default bind 0.0.0.0:3300 (port 3000 is taken on this machine)
SUB2ORG_CONFIG=path cargo run   # alternate config
cargo build [--release]
cargo test                # unit tests (config/metrics/logging)
cargo test -- <name>      # single test by name pattern
cargo clippy
```
Postgres is **optional**: set `database_url` in config.toml → Postgres backend (migrations auto-run at startup); omit → in-memory (dev; lost on restart). For Postgres tests, spin a throwaway container on a non-default port (5432 is used by another project): `docker run -d -e POSTGRES_PASSWORD=postgres -e POSTGRES_DB=sub2org -p 5433:5432 postgres:16`.

### extension (`cd extension`) — yarn, Node 18.17.1
```bash
yarn install && yarn build:chrome   # → dist_chrome (load unpacked at chrome://extensions)
```
No test runner.

### console (`cd console`) — npm
```bash
npm install && npm run dev    # http://localhost:5300, Vite proxies /v1 /auth /admin /healthz → :3300
npm run build                 # → dist/ (build script is `vite build` only — esbuild transpiles, no tsc gate)
```

### End-to-end verification (no real browser needed)
Start `server`, then run a mock worker that speaks the worker protocol: `cd docs && npm i socket.io-client@4 && WORKER_ID=mock-a node mock-worker.js`. It supports prompt markers `COOL` (rate-limit), `LOGOUT`, `DROP` (crash) to exercise pool behavior. Then curl `/v1/chat/completions` or the console.

## Architecture (the big picture)

### Relay flow & request lifecycle (`server/src/http/consumer.rs`)
`POST /v1/chat/completions` → resolve api_key→user (`account.store.resolve_key`) → per-user rate-limit guard → **scheduler picks a worker** (`pool::scheduler` / `pool::registry`) → optional billing pre-deduct → emit `dispatch` over Socket.IO → worker streams `chunk`/`done` back, correlated by **`req_id`** through `AppState.pending` (a `DashMap<req_id, PendingEntry>`) → server re-frames as OpenAI `chat.completion.chunk` SSE (or aggregates for non-stream). Earlier single-tenant designs keyed everything by a single `roomId` with one pending request; sub2org keys by `req_id` and supports a pool with concurrency.

Two failure invariants worth knowing: a `PendingGuard` removes the pending entry on drop, and `state.fail_worker_inflight(worker_id, msg)` is called on worker disconnect/heartbeat-timeout so in-flight requests fail **immediately** instead of waiting for `generation_secs`.

### Worker pool (`server/src/pool/`)
`registry.rs` is the **in-memory source of truth for who's online** (single instance, no Redis). Each worker has a `tokio::sync::Semaphore` for capacity (locked to 1 per machine) and a status model: `idle/busy/cooling/logged_out/disabled` (+ `offline` for DB-known-but-not-connected). `is_dispatchable(now)` gates selection; `scheduler.rs` does `acquire_any`/`acquire_specific` with a queue-wait + a `Notify` woken on slot-release or new-worker. `pool/health.rs` is a spawned task that evicts stale workers. Postgres only persists worker **identity/stats/disabled** (not live presence).

### Storage = trait + (InMemory | Postgres), chosen by `database_url`
There are **separate trait-based stores**, each with both backends, all using sqlx **runtime queries** (no compile-time DB) — `PgStore::connect` runs `sqlx::migrate!("./migrations")` (all of `0001..0004`) at startup:
- `store::Store` — sessions (affinity) + worker registry/stats.
- `account::AccountStore` — users, api_keys (belong to users), `billing_tx`, `redeem_codes`.
- `logging::LogStore` — logs (Postgres table or in-memory ring).
- `metrics` (in `server/src/metrics.rs`) — in-memory `RequestRing` (24h request counts) + `QueueWaitStat`, always-on.

`AppState` (`server/src/state.rs`) is the shared hub holding all of the above + the SocketIo handle + a `broadcast::Sender<LiveEvent>`. **It is constructed once in `main.rs`'s startup sequence (before serving)** — do not introduce lazy/module-level singletons (a documented global rule; avoids restart-time races).

### Auth & roles (`server/src/account/`)
Three auth surfaces, all `Authorization: Bearer …`:
- **Consumer API** (`/v1/chat/completions`): bearer is an **api_key** (`sk-sub2org-…`); resolves to a user, checks key/user status, rate-limit, balance.
- **User dashboard** (`/auth/*`, `/v1/balance|keys|redeem|transactions`): bearer is a **JWT** (argon2 password login → `jsonwebtoken`); `CurrentUser` extractor.
- **Admin** (`/admin/*`): `AdminGuard` extractor — `X-Admin-Token` header (config) OR an admin-role JWT.
First boot bootstraps an admin from `config.admin_username/password` and assigns config's seed `api_keys` to it.

### Billing (`account::Charge`, `billing_enabled` default **false**)
Per-request, per-model cost (`[model_cost]` prefix map, fallback `default_cost`). Model = **pre-deduct + refund-on-failure**: `reserve_charge` atomically deducts before dispatch (402 if insufficient); a `Charge` handle is moved into the stream and `commit()`-ed on `Done` or `refund()`-ed on error/timeout. With `billing_enabled=false` the whole path is bypassed (keeps the relay behaving as before).

### Logging (`server/src/logging/`) — never blocks the relay
A `tracing` Layer captures **server** events (only `target` starting with `sub2org`, to avoid a write→log→write feedback loop with sqlx); add a `category=` field on `tracing::*` calls for classification. **Worker** logs arrive over the socket `log` event and are tagged `source=worker` by connection identity. `LogSink` is a bounded channel with non-blocking `try_send` (drops + counts on overflow); a flusher batches to the store.

### Real-time for the console
`GET /admin/metrics` (KPIs + 60-pt throughput from `RequestRing`) and `GET /admin/events` (SSE over the `broadcast` channel; events published from consumer/gateway/health/admin via `state.publish(LiveEvent::…)`). The console consumes `/admin/events` via **fetch-stream** (so it can send the bearer header), and degrades to polling `/admin/logs` if absent.

### Worker ↔ server protocol (Socket.IO, `server/src/ws/`)
Handshake `auth: { worker_token, worker_id, provider, account_label, capacity }`. S→W: `dispatch`, `cancel`. W→S: `chunk`, `done`, `error`, `status`, `heartbeat`, `log`. This contract spans **server + extension** — changing an event name/shape means editing both.

### Extension internals (`extension/src/pages/`)
`background/index.ts` (service worker) owns the socket + worker identity + tab management. `content/loader.ts` (+ claude variant) is injected into the **page's JS world** to hijack `window.fetch`, parse the AI's SSE, and `postMessage` increments tagged `{ s2o: true, event: … }` back to the content script → background → socket. The `s2o` marker is the private loader↔content contract (rename both sides together). DOM selectors / rate-limit detection are best-effort and need tuning against the live ChatGPT/Claude UI.

### Console internals (`console/src/`)
Tailwind **v4 CSS-first** (`globals.css` `@theme` tokens; light `:root` for main area, `.dark` scope override for the sidebar) — there is **no `tailwind.config.js`**. shadcn-style primitives are **hand-written** in `components/ui.tsx` (not the shadcn CLI). Data layer: `api/` (typed fetch client + error-by-HTTP-status + endpoints) and `hooks.ts` (TanStack Query). Single role-routed SPA: admin sees the full console, normal users see the self-service dashboard (`lib/auth.tsx` guards).

## Conventions & gotchas

- **Naming**: the project is `sub2org`. Wire identifiers: api keys `sk-sub2org-`, headers `X-Sub2org-Session|New-Window|Provider`, redeem codes `SUB2ORG-…`, loader marker `s2o`, extension `sub2org-worker`. Keep these consistent across server/extension/console/docs.
- **Cross-component contracts**: a change to a Socket.IO event, the `/admin/*` or `/v1/*` JSON shape, or the SSE frame usually requires matching edits in server **and** console (and sometimes extension). Grep all three.
- `config.toml` ships with dev creds (`worker_tokens`, `api_keys`, `jwt_secret`, empty `admin_token`) — these are for local use; production must set real values (`admin_token` empty = `/admin/*` open / internal-only).
- Timestamps: users/workers/sessions use unix **seconds**; logs use unix **milliseconds** (`ts_ms`).
