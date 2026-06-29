import { api } from "./client";
import type {
  ApiKeyRow,
  AuditList,
  LoginResult,
  LogEntry,
  Metrics,
  PaymentCreate,
  PaymentOrderStatus,
  Pricing,
  RedeemCode,
  Session,
  Tx,
  UsageByModel,
  UsageByUser,
  UsageStats,
  User,
  WorkerView,
} from "./types";

export const ep = {
  // auth
  login: (b: { username: string; password: string }) =>
    api.post<LoginResult>("/auth/login", b),
  login2fa: (b: { challenge_token: string; code: string }) =>
    api.post<{ jwt: string }>("/auth/login/2fa", b),
  register: (b: { username: string; password: string; email?: string; turnstile_token?: string }) =>
    api.post<{ user_id: string; jwt: string | null }>("/auth/register", b),
  setKeyGroup: (id: string, group: string) => api.post(`/admin/api-keys/${id}/group`, { group }), // B2
  forgot: (email: string) => api.post<{ ok: boolean }>("/auth/forgot", { email }),
  // 当前用户 / 安全
  me: () => api.get<{ user: User }>("/v1/me"),
  logoutAll: () => api.post<{ ok: boolean; token_version: number }>("/v1/logout-all"),
  totpSetup: () => api.post<{ secret: string; otpauth_uri: string }>("/v1/2fa/setup"),
  totpEnable: (code: string) => api.post<{ enabled: boolean }>("/v1/2fa/enable", { code }),
  totpDisable: (code?: string, password?: string) =>
    api.post<{ enabled: boolean }>("/v1/2fa/disable", { code, password }),
  reset: (token: string, password: string) => api.post<{ ok: boolean }>("/auth/reset", { token, password }),

  // pricing (公开)
  pricing: () => api.get<Pricing>("/v1/pricing"),

  // user dashboard
  balance: () => api.get<{ balance: number; recent_tx: Tx[] }>("/v1/balance"),
  transactions: () => api.get<{ transactions: Tx[]; count: number }>("/v1/transactions"),
  redeem: (code: string) => api.post<{ balance: number }>("/v1/redeem", { code }),
  myKeys: () => api.get<{ keys: ApiKeyRow[] }>("/v1/keys"),
  createMyKey: (name: string) => api.post<{ id: string; api_key: string; name: string }>("/v1/keys", { name }),
  disableMyKey: (id: string) => api.post(`/v1/keys/${id}/disable`),
  enableMyKey: (id: string) => api.post(`/v1/keys/${id}/enable`),
  deleteMyKey: (id: string) => api.del(`/v1/keys/${id}`),

  // 在线支付
  createPayment: (amount: number, channel: string) =>
    api.post<PaymentCreate>("/v1/payment/create", { amount, channel }),
  paymentOrder: (id: string) => api.get<PaymentOrderStatus>(`/v1/payment/orders/${id}`),

  // admin
  metrics: () => api.get<Metrics>("/admin/metrics"),
  workers: () => api.get<{ workers: WorkerView[]; count: number }>("/admin/workers"),
  disableWorker: (id: string) => api.post(`/admin/workers/${id}/disable`),
  enableWorker: (id: string) => api.post(`/admin/workers/${id}/enable`),
  sessions: () => api.get<{ sessions: Session[]; count: number }>("/admin/sessions"),
  logs: (q: Record<string, unknown>) =>
    api.get<{ logs: LogEntry[]; count: number; dropped: number }>("/admin/logs", { query: q }),
  users: () => api.get<{ users: User[]; count: number }>("/admin/users"),
  createUser: (b: { username: string; password: string; role?: string; initial_balance?: number }) =>
    api.post<{ id: string; username: string; role: string }>("/admin/users", b),
  banUser: (id: string) => api.post(`/admin/users/${id}/ban`),
  unbanUser: (id: string) => api.post(`/admin/users/${id}/unban`),
  rechargeUser: (id: string, amount: number, note: string) =>
    api.post<{ id: string; balance: number }>(`/admin/users/${id}/recharge`, { amount, note }),
  setRateLimit: (id: string, n: number) => api.post(`/admin/users/${id}/rate-limit`, { n }),
  userKeys: (id: string) => api.get<{ keys: ApiKeyRow[] }>(`/admin/users/${id}/keys`),
  createUserKey: (id: string, name: string) =>
    api.post<{ id: string; api_key: string; user_id: string }>(`/admin/users/${id}/keys`, { name }),
  redeemCodes: () => api.get<{ codes: RedeemCode[]; count: number }>("/admin/redeem-codes"),
  createRedeemCodes: (amount: number, count: number) =>
    api.post<{ codes: string[]; amount: number }>("/admin/redeem-codes", { amount, count }),
  usageStats: (q?: { since?: number; until?: number }) =>
    api.get<UsageStats>("/admin/stats/usage", { query: q }),
  usageByUser: (q?: { since?: number; until?: number; limit?: number }) =>
    api.get<UsageByUser>("/admin/stats/usage", { query: { ...q, group_by: "user" } }),
  usageByModel: (q?: { since?: number; until?: number }) =>
    api.get<UsageByModel>("/admin/stats/usage", { query: { ...q, group_by: "model" } }),
  audit: (q?: { actor?: string; action?: string; since?: number; limit?: number }) =>
    api.get<AuditList>("/admin/audit", { query: q }),
  reset2fa: (id: string) => api.post<{ id: string; totp_enabled: boolean }>(`/admin/users/${id}/2fa/reset`),
};
