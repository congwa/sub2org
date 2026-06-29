-- sub2org 用户体系 + 计费：users(余额挂用户) + api_keys 归属用户 + 流水 + 兑换码。

CREATE TABLE IF NOT EXISTS users (
    id            TEXT PRIMARY KEY,
    username      TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,                  -- argon2
    email         TEXT,
    role          TEXT NOT NULL DEFAULT 'user',   -- admin | user
    status        TEXT NOT NULL DEFAULT 'active', -- active | banned
    balance       BIGINT NOT NULL DEFAULT 0,      -- 余额挂用户
    rate_limit    INTEGER NOT NULL DEFAULT 0,     -- 每用户在途并发上限，0=不限
    created_at    BIGINT NOT NULL
);

-- api_keys 归属用户 + 状态（余额移到 users；owner 弃用，保留列以兼容旧迁移）
ALTER TABLE api_keys ADD COLUMN IF NOT EXISTS user_id TEXT;
ALTER TABLE api_keys ADD COLUMN IF NOT EXISTS name   TEXT NOT NULL DEFAULT '';
ALTER TABLE api_keys ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT 'active'; -- active | disabled
CREATE INDEX IF NOT EXISTS idx_keys_user ON api_keys (user_id);

CREATE TABLE IF NOT EXISTS billing_tx (
    id            BIGSERIAL PRIMARY KEY,
    user_id       TEXT NOT NULL,
    kind          TEXT NOT NULL,                  -- recharge | charge | refund | redeem
    amount        BIGINT NOT NULL,                -- 带符号：+充值 / -扣费
    balance_after BIGINT NOT NULL,
    req_id        TEXT,
    model         TEXT,
    note          TEXT,
    created_at    BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_tx_user ON billing_tx (user_id, created_at DESC);

CREATE TABLE IF NOT EXISTS redeem_codes (
    code       TEXT PRIMARY KEY,
    amount     BIGINT NOT NULL,
    created_at BIGINT NOT NULL,
    used_by    TEXT,
    used_at    BIGINT
);

CREATE TABLE IF NOT EXISTS payment_orders (   -- 预留，本期不接真实网关
    order_id   TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL,
    amount     BIGINT NOT NULL,
    provider   TEXT NOT NULL,
    status     TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    paid_at    BIGINT
);
