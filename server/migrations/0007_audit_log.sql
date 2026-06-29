-- 管理员变更审计（append-only）。actor=触发者身份，detail=结构化上下文(JSONB)，时间为 unix 秒。
CREATE TABLE IF NOT EXISTS audit_log (
    id         BIGSERIAL PRIMARY KEY,
    actor      TEXT NOT NULL,            -- "jwt:<username>" | "token" | "internal"
    action     TEXT NOT NULL,            -- recharge | ban_user | set_rate_limit | ...
    target     TEXT NOT NULL DEFAULT '', -- 受影响实体 id（user_id / worker_id / key_id）
    detail     JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_audit_created ON audit_log (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_actor   ON audit_log (actor, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_action  ON audit_log (action, created_at DESC);
