// 空字符串 = 同源（dev 走 Vite proxy；prod 同域部署）。也可用 VITE_API_BASE_URL 覆盖。
export const API_BASE = (import.meta.env.VITE_API_BASE_URL as string | undefined) ?? "";

export const POLL = {
  workers: 5000,
  sessions: 10000,
  logs: 4000,
  metrics: 10000,
};
