-- 加速区间用量聚合（usage_summary / usage_by_user / usage_by_model）。
-- 纯性能优化，非正确性必需；billing 量大时显著降低全表扫描。
CREATE INDEX IF NOT EXISTS idx_tx_created ON billing_tx (created_at);
CREATE INDEX IF NOT EXISTS idx_tx_model_created ON billing_tx (model, created_at) WHERE model IS NOT NULL;
