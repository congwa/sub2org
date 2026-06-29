export type UserRole = "admin" | "user";
export type UserStatus = "active" | "banned";
export type KeyStatus = "active" | "disabled";
export type TxKind = "recharge" | "charge" | "refund" | "redeem";
export type WorkerStatus = "idle" | "busy" | "cooling" | "logged_out" | "offline" | "disabled" | "tripped";
export type LogLevel = "trace" | "debug" | "info" | "warn" | "error";
export type LogSource = "server" | "worker";

export interface JwtClaims { sub: string; role: UserRole; exp: number; tv?: number }

export interface User {
  id: string;
  username: string;
  email: string | null;
  role: UserRole;
  status: UserStatus;
  balance: number;
  rate_limit: number;
  created_at: number;
  totp_enabled?: boolean;
}

export type LoginResult = { jwt: string } | { need_2fa: true; challenge_token: string };

export interface AuditRow {
  id: number;
  actor: string;
  action: string;
  target: string;
  detail: Record<string, unknown>;
  created_at: number;
}
export interface AuditList {
  audit: AuditRow[];
  count: number;
}
export interface Tx {
  id: number;
  user_id: string;
  kind: TxKind;
  amount: number;
  balance_after: number;
  req_id: string | null;
  model: string | null;
  note: string | null;
  created_at: number;
}
export interface ApiKeyRow {
  id: string;
  user_id: string;
  name: string;
  status: KeyStatus;
  created_at: number;
  group?: string; // B2 分组标签
}
export interface WorkerView {
  worker_id: string;
  provider: string;
  group?: string; // B2 分组标签
  account_label: string;
  online: boolean;
  status: WorkerStatus;
  in_flight: number;
  free: number;
  capacity: number;
  cooldown_until: number;
  last_seen_at: number;
  total_served: number;
  disabled?: boolean;
}
export interface Session {
  session_id: string;
  api_key_id: string;
  worker_id: string;
  provider: string;
  native_conversation_id: string | null;
  created_at: number;
  last_used_at: number;
}
export interface LogEntry {
  ts_ms: number;
  source: LogSource;
  worker_id: string | null;
  account_label: string | null;
  category: string;
  level: LogLevel;
  req_id: string | null;
  session_id: string | null;
  message: string;
  fields: Record<string, unknown>;
}
export interface RedeemCode {
  code: string;
  amount: number;
  created_at: number;
  used_by: string | null;
  used_at: number | null;
}
export interface Metrics {
  workers: { online: number; total: number };
  active_sessions: number;
  requests_24h: number;
  avg_queue_wait_ms: number | null;
  throughput: { bucket_secs: number; points: { t: number; count: number }[] };
  storage_backend?: "postgres" | "memory";
}

export type LiveEvent =
  | { type: "request_dispatched"; req_id: string; model: string; worker_id: string; session_id: string; action: string }
  | { type: "request_done"; req_id: string; status: string; tokens?: number; latency_ms: number }
  | { type: "request_error"; req_id: string; code: string }
  | { type: "worker_status"; worker_id: string; status: WorkerStatus }
  | { type: "balance_low"; user_id: string; balance: number; threshold: number };

export interface Pricing {
  billing_enabled: boolean;
  default_cost: number;
  model_cost: Record<string, number>;
  cost_multiplier?: number; // B3
  model_multiplier?: Record<string, number>; // B3
  credits_per_cny: number | null; // null = 在线支付未配置
  turnstile_site_key?: string | null; // B5：注册验证码 site key（空=未启用）
}
export interface PaymentCreate {
  order_id: string;
  pay_url: string;
  amount: number;
  money: string;
  channel: string;
}
export interface PaymentOrderStatus {
  order_id: string;
  status: "pending" | "paid";
  amount: number;
  paid_at: number | null;
  created_at: number;
}
export interface UsageStats {
  since: number;
  until: number;
  by_kind: { kind: string; sum: number; count: number }[];
  spent_credits: number;
  refunded_credits: number;
  recharged_credits: number;
  redeemed_credits: number;
  net_consumed_credits: number;
}

export interface UserUsageRow {
  user_id: string;
  username: string;
  spent: number;
  refunded: number;
  count: number;
}
export interface ModelUsageRow {
  model: string;
  spent: number;
  count: number;
}
export interface UsageByUser {
  since: number;
  until: number;
  by_user: UserUsageRow[];
}
export interface UsageByModel {
  since: number;
  until: number;
  by_model: ModelUsageRow[];
}
