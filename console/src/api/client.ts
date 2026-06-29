import { API_BASE } from "@/config";
import { ApiError, toApiError } from "./errors";

let getToken: () => string | null = () => null;
let onUnauthorized: (() => void) | null = null;

export function configureClient(opts: { getToken: () => string | null; onUnauthorized: () => void }) {
  getToken = opts.getToken;
  onUnauthorized = opts.onUnauthorized;
}

export function authHeader(): Record<string, string> {
  const t = getToken();
  return t ? { Authorization: `Bearer ${t}` } : {};
}

function buildUrl(path: string, query?: Record<string, unknown>): string {
  let qs = "";
  if (query) {
    const p = new URLSearchParams();
    for (const [k, v] of Object.entries(query)) {
      if (v !== undefined && v !== null && v !== "") p.set(k, String(v));
    }
    const s = p.toString();
    if (s) qs = `?${s}`;
  }
  return `${API_BASE}${path}${qs}`;
}

async function handle<T>(res: Response): Promise<T> {
  if (res.ok) {
    if (res.status === 204) return undefined as T;
    const ct = res.headers.get("content-type") ?? "";
    return ct.includes("application/json") ? ((await res.json()) as T) : (undefined as T);
  }
  const err = await toApiError(res);
  if (err.kind === "unauthorized") onUnauthorized?.();
  throw err;
}

interface Opts {
  query?: Record<string, unknown>;
  signal?: AbortSignal;
}

async function request<T>(method: string, path: string, body?: unknown, opts: Opts = {}): Promise<T> {
  let res: Response;
  try {
    res = await fetch(buildUrl(path, opts.query), {
      method,
      headers: {
        ...authHeader(),
        ...(body !== undefined ? { "Content-Type": "application/json" } : {}),
      },
      body: body !== undefined ? JSON.stringify(body) : undefined,
      signal: opts.signal,
    });
  } catch (e) {
    if ((e as DOMException)?.name === "AbortError") throw e;
    throw new ApiError("network", 0, "network error", undefined, true);
  }
  return handle<T>(res);
}

export const api = {
  get: <T>(p: string, o?: Opts) => request<T>("GET", p, undefined, o),
  post: <T>(p: string, body?: unknown, o?: Opts) => request<T>("POST", p, body, o),
  del: <T>(p: string, o?: Opts) => request<T>("DELETE", p, undefined, o),
};
