-- sub2org M3：worker 自注册入库（身份/标签/统计/禁用开关）。
-- 注意：worker「在线状态」仍只在内存(registry)，此表只存持久化身份与统计。

CREATE TABLE IF NOT EXISTS workers (
    worker_id      TEXT PRIMARY KEY,
    account_label  TEXT NOT NULL DEFAULT '',
    provider       TEXT NOT NULL DEFAULT 'chatgpt',
    created_at     BIGINT NOT NULL,
    last_seen_at   BIGINT NOT NULL,
    total_served   BIGINT NOT NULL DEFAULT 0,
    disabled       BOOLEAN NOT NULL DEFAULT false
);
