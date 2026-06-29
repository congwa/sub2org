import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { Navigate, useLocation } from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";
import { configureClient } from "@/api/client";
import type { JwtClaims, UserRole } from "@/api/types";

const KEY = "sub2org.jwt";
let mem: string | null = null;
export const tokenStore = {
  get: () => (mem ??= localStorage.getItem(KEY)),
  set: (t: string) => {
    mem = t;
    localStorage.setItem(KEY, t);
  },
  clear: () => {
    mem = null;
    localStorage.removeItem(KEY);
  },
};

export function decodeJwt(token: string): JwtClaims | null {
  try {
    const [, payload] = token.split(".");
    const json = atob(payload.replace(/-/g, "+").replace(/_/g, "/"));
    const c = JSON.parse(json) as JwtClaims;
    return c.sub && c.role && typeof c.exp === "number" ? c : null;
  } catch {
    return null;
  }
}
const isExpired = (c: JwtClaims, skew = 30) => c.exp * 1000 <= Date.now() + skew * 1000;

interface AuthValue {
  token: string | null;
  role: UserRole | null;
  username: string | null;
  isAuthed: boolean;
  isAdmin: boolean;
  login: (jwt: string) => void;
  logout: () => void;
}
const Ctx = createContext<AuthValue | null>(null);

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const qc = useQueryClient();
  const [token, setToken] = useState<string | null>(() => {
    const t = tokenStore.get();
    const c = t && decodeJwt(t);
    if (!c || isExpired(c)) {
      tokenStore.clear();
      return null;
    }
    return t;
  });
  const claims = useMemo(() => (token ? decodeJwt(token) : null), [token]);

  const logout = useCallback(() => {
    tokenStore.clear();
    setToken(null);
    qc.clear();
  }, [qc]);
  const login = useCallback((jwt: string) => {
    tokenStore.set(jwt);
    setToken(jwt);
  }, []);

  useEffect(() => {
    configureClient({ getToken: tokenStore.get, onUnauthorized: logout });
  }, [logout]);

  const value: AuthValue = {
    token,
    role: claims?.role ?? null,
    username: claims?.sub ?? null,
    isAuthed: !!claims && !isExpired(claims),
    isAdmin: claims?.role === "admin",
    login,
    logout,
  };
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useAuth(): AuthValue {
  const v = useContext(Ctx);
  if (!v) throw new Error("useAuth outside AuthProvider");
  return v;
}

export function RequireAuth({ children }: { children: React.ReactNode }) {
  const { isAuthed } = useAuth();
  const loc = useLocation();
  return isAuthed ? <>{children}</> : <Navigate to="/login" replace state={{ from: loc.pathname }} />;
}
export function RequireAdmin({ children }: { children: React.ReactNode }) {
  const { isAuthed, isAdmin } = useAuth();
  if (!isAuthed) return <Navigate to="/login" replace />;
  return isAdmin ? <>{children}</> : <Navigate to="/dashboard" replace />;
}
