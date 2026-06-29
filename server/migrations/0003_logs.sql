-- sub2org 日志系统：按端(source) / 类型(category) / 级别(level) 分类入库。

CREATE TABLE IF NOT EXISTS logs (
    id            BIGSERIAL PRIMARY KEY,
    ts_ms         BIGINT NOT NULL,        -- unix 毫秒
    source        TEXT   NOT NULL,        -- 'server' | 'worker'
    worker_id     TEXT,                   -- source=worker 时（服务端按连接身份打）
    account_label TEXT,
    category      TEXT   NOT NULL,        -- connection/dispatch/stream/scheduler/auth/rate_limit/http/dom/system/error
    level         TEXT   NOT NULL,        -- trace/debug/info/warn/error
    req_id        TEXT,
    session_id    TEXT,
    message       TEXT   NOT NULL,
    fields        JSONB
);

CREATE INDEX IF NOT EXISTS idx_logs_ts ON logs (ts_ms DESC);
CREATE INDEX IF NOT EXISTS idx_logs_source ON logs (source);
CREATE INDEX IF NOT EXISTS idx_logs_worker ON logs (worker_id);
CREATE INDEX IF NOT EXISTS idx_logs_cat ON logs (category);
CREATE INDEX IF NOT EXISTS idx_logs_level ON logs (level);
