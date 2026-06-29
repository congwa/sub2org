-- 账号恢复（邮箱验证）+ 在线支付订单索引。

-- 邮箱验证标记（找回密码用无状态签名 token，不建表）。
ALTER TABLE users ADD COLUMN IF NOT EXISTS email_verified BOOLEAN NOT NULL DEFAULT false;
CREATE INDEX IF NOT EXISTS idx_users_email ON users (email);

-- payment_orders 在 0004 已建（预留），这里补查询索引。
CREATE INDEX IF NOT EXISTS idx_payorders_user ON payment_orders (user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_payorders_status ON payment_orders (status);
