import { useEffect, useReducer, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ep } from "@/api/endpoints";
import { startAdminEvents, type SseState } from "@/api/sse";
import { useAuth } from "@/lib/auth";
import { POLL } from "@/config";
import type { LiveEvent, WorkerView } from "@/api/types";

export const qk = {
  metrics: ["metrics"] as const,
  workers: ["workers"] as const,
  sessions: ["sessions"] as const,
  logs: (q: unknown) => ["logs", q] as const,
  users: ["users"] as const,
  userKeys: (id: string) => ["userKeys", id] as const,
  redeemCodes: ["redeemCodes"] as const,
  balance: ["balance"] as const,
  transactions: ["transactions"] as const,
  myKeys: ["myKeys"] as const,
};

/* ---------- admin queries ---------- */
export const useMetrics = () =>
  useQuery({ queryKey: qk.metrics, queryFn: ep.metrics, refetchInterval: POLL.metrics });

export const useWorkers = () =>
  useQuery({
    queryKey: qk.workers,
    queryFn: ep.workers,
    refetchInterval: POLL.workers,
    select: (r) => r.workers,
  });

export const useSessions = () =>
  useQuery({ queryKey: qk.sessions, queryFn: ep.sessions, refetchInterval: POLL.sessions, select: (r) => r.sessions });

export const useLogs = (q: Record<string, unknown>) =>
  useQuery({ queryKey: qk.logs(q), queryFn: () => ep.logs(q), refetchInterval: POLL.logs });

export const useUsers = () => useQuery({ queryKey: qk.users, queryFn: ep.users, select: (r) => r.users });
export const useUserKeys = (id: string) =>
  useQuery({ queryKey: qk.userKeys(id), queryFn: () => ep.userKeys(id), select: (r) => r.keys, enabled: !!id });
export const useRedeemCodes = () =>
  useQuery({ queryKey: qk.redeemCodes, queryFn: ep.redeemCodes, select: (r) => r.codes });

/* ---------- admin mutations ---------- */
function useInvalidate(keys: readonly unknown[][]) {
  const qc = useQueryClient();
  return () => keys.forEach((k) => qc.invalidateQueries({ queryKey: k }));
}

export const useDisableWorker = () => {
  const inv = useInvalidate([qk.workers]);
  return useMutation({ mutationFn: ep.disableWorker, onSuccess: inv });
};
export const useEnableWorker = () => {
  const inv = useInvalidate([qk.workers]);
  return useMutation({ mutationFn: ep.enableWorker, onSuccess: inv });
};
export const useCreateUser = () => {
  const inv = useInvalidate([qk.users]);
  return useMutation({
    mutationFn: (b: { username: string; password: string; role?: string; initial_balance?: number }) => ep.createUser(b),
    onSuccess: inv,
  });
};
export const useBanUser = () => {
  const inv = useInvalidate([qk.users]);
  return useMutation({ mutationFn: ep.banUser, onSuccess: inv });
};
export const useUnbanUser = () => {
  const inv = useInvalidate([qk.users]);
  return useMutation({ mutationFn: ep.unbanUser, onSuccess: inv });
};
export const useRechargeUser = () => {
  const inv = useInvalidate([qk.users]);
  return useMutation({
    mutationFn: (v: { id: string; amount: number; note: string }) => ep.rechargeUser(v.id, v.amount, v.note),
    onSuccess: inv,
  });
};
export const useSetRateLimit = () => {
  const inv = useInvalidate([qk.users]);
  return useMutation({ mutationFn: (v: { id: string; n: number }) => ep.setRateLimit(v.id, v.n), onSuccess: inv });
};
export const useCreateUserKey = () =>
  useMutation({ mutationFn: (v: { id: string; name: string }) => ep.createUserKey(v.id, v.name) });
export const useCreateRedeemCodes = () => {
  const inv = useInvalidate([qk.redeemCodes]);
  return useMutation({ mutationFn: (v: { amount: number; count: number }) => ep.createRedeemCodes(v.amount, v.count), onSuccess: inv });
};
export const useUsageStats = (q?: { since?: number; until?: number }) =>
  useQuery({ queryKey: ["usageStats", q] as const, queryFn: () => ep.usageStats(q) });
export const useUsageByUser = (q?: { since?: number; until?: number; limit?: number }) =>
  useQuery({ queryKey: ["usageByUser", q] as const, queryFn: () => ep.usageByUser(q) });
export const useUsageByModel = (q?: { since?: number; until?: number }) =>
  useQuery({ queryKey: ["usageByModel", q] as const, queryFn: () => ep.usageByModel(q) });
export const useAudit = (q: { actor?: string; action?: string; limit?: number }) =>
  useQuery({ queryKey: ["audit", q] as const, queryFn: () => ep.audit(q) });

/* ---------- 安全 / 2FA ---------- */
export const useLogin2fa = () =>
  useMutation({ mutationFn: (b: { challenge_token: string; code: string }) => ep.login2fa(b) });
export const useMe = () => useQuery({ queryKey: ["me"] as const, queryFn: () => ep.me() });
export const useLogoutAll = () => useMutation({ mutationFn: () => ep.logoutAll() });
export const useTotpSetup = () => useMutation({ mutationFn: () => ep.totpSetup() });
export const useTotpEnable = () => useMutation({ mutationFn: (code: string) => ep.totpEnable(code) });
export const useTotpDisable = () =>
  useMutation({ mutationFn: (v: { code?: string; password?: string }) => ep.totpDisable(v.code, v.password) });
export const useReset2fa = () => useMutation({ mutationFn: (id: string) => ep.reset2fa(id) });

/* ---------- pricing / payment ---------- */
export const usePricing = () => useQuery({ queryKey: ["pricing"] as const, queryFn: ep.pricing });
export const useCreatePayment = () =>
  useMutation({ mutationFn: (v: { amount: number; channel: string }) => ep.createPayment(v.amount, v.channel) });

/* ---------- user dashboard ---------- */
export const useBalance = () => useQuery({ queryKey: qk.balance, queryFn: ep.balance });
export const useTransactions = () =>
  useQuery({ queryKey: qk.transactions, queryFn: ep.transactions, select: (r) => r.transactions });
export const useMyKeys = () => useQuery({ queryKey: qk.myKeys, queryFn: ep.myKeys, select: (r) => r.keys });
export const useRedeem = () => {
  const inv = useInvalidate([qk.balance, qk.transactions]);
  return useMutation({ mutationFn: (code: string) => ep.redeem(code), onSuccess: inv });
};
export const useCreateMyKey = () => {
  const inv = useInvalidate([qk.myKeys]);
  return useMutation({ mutationFn: (name: string) => ep.createMyKey(name), onSuccess: inv });
};
export const useDisableMyKey = () => {
  const inv = useInvalidate([qk.myKeys]);
  return useMutation({ mutationFn: ep.disableMyKey, onSuccess: inv });
};
export const useEnableMyKey = () => {
  const inv = useInvalidate([qk.myKeys]);
  return useMutation({ mutationFn: ep.enableMyKey, onSuccess: inv });
};
export const useDeleteMyKey = () => {
  const inv = useInvalidate([qk.myKeys]);
  return useMutation({ mutationFn: ep.deleteMyKey, onSuccess: inv });
};

/* ---------- auth ---------- */
export const useLogin = () => useMutation({ mutationFn: (b: { username: string; password: string }) => ep.login(b) });
export const useRegister = () =>
  useMutation({ mutationFn: (b: { username: string; password: string; email?: string; turnstile_token?: string }) => ep.register(b) });
export const useForgot = () => useMutation({ mutationFn: (email: string) => ep.forgot(email) });
export const useReset = () =>
  useMutation({ mutationFn: (v: { token: string; password: string }) => ep.reset(v.token, v.password) });

/* ---------- admin live events (SSE → cache patch + feed) ---------- */
export interface FeedItem extends LiveEvent {
  at: number;
}
export function useAdminEvents() {
  const qc = useQueryClient();
  const { token, isAdmin } = useAuth();
  const [conn, setConn] = useState<SseState>("connecting");
  const feed = useRef<FeedItem[]>([]);
  const [, bump] = useReducer((x: number) => x + 1, 0);

  useEffect(() => {
    if (!isAdmin) return;
    const stop = startAdminEvents({
      token: () => token,
      onState: setConn,
      onEvent: (ev) => {
        feed.current = [{ ...ev, at: Date.now() }, ...feed.current].slice(0, 200);
        if (ev.type === "worker_status") {
          qc.setQueryData<{ workers: WorkerView[]; count: number }>(qk.workers, (o) =>
            o
              ? {
                  ...o,
                  workers: o.workers.map((w) =>
                    w.worker_id === ev.worker_id ? { ...w, status: ev.status } : w,
                  ),
                }
              : o,
          );
        }
        bump();
      },
    });
    return stop;
  }, [isAdmin, token, qc]);

  return { connection: conn, events: feed.current, live: conn === "open" };
}
