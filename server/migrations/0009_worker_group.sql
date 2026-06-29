-- B2 账号分组路由：worker 与 api_key 各加分组标签（"" = 默认池，全向后兼容）。
-- 列名用 worker_group / group_tag 避开 SQL 保留字 group。
ALTER TABLE workers ADD COLUMN IF NOT EXISTS worker_group TEXT NOT NULL DEFAULT '';
ALTER TABLE api_keys ADD COLUMN IF NOT EXISTS group_tag TEXT NOT NULL DEFAULT '';
