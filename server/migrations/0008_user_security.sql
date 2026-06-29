-- 用户安全：TOTP 两步验证 + JWT 主动吊销版本号。
-- token_version 用 BIGINT 以匹配 Rust i64（Claims.tv / User.token_version），避免 FromRow 解码失配。
ALTER TABLE users ADD COLUMN IF NOT EXISTS totp_secret   TEXT;
ALTER TABLE users ADD COLUMN IF NOT EXISTS totp_enabled  BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE users ADD COLUMN IF NOT EXISTS token_version BIGINT  NOT NULL DEFAULT 0;
