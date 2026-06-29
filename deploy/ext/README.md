# 自托管 CRX 目录

把 `extension/scripts/pack-crx.mjs` 产出的 `sub2org-worker-<version>.crx` 与 `updates.xml`
放到这里（被 nginx 以 `/ext/` 静态托管，供 worker 扩展自动更新）。

实际产物（`*.crx` / `*.pem` / `updates.xml`）已在 `deploy/.gitignore` 排除，不入库。
流程见 `../../docs/DEPLOY.md` 第 11 节。
