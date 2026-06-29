import { NavLink, Outlet, useLocation } from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";
import {
  Activity,
  CreditCard,
  History,
  KeyRound,
  LayoutDashboard,
  LogOut,
  MessagesSquare,
  RefreshCw,
  ScrollText,
  Server,
  ShieldCheck,
  Users,
  Wallet,
  type LucideIcon,
} from "lucide-react";
import { cn } from "@/lib/cn";
import { useAuth } from "@/lib/auth";
import { Logo, Wordmark } from "@/components/Logo";
import { Button } from "@/components/ui";
import { HealthPill } from "@/components/common";
import { useWorkers } from "@/hooks";

type Role = "admin" | "user";
interface NavItem {
  to: string;
  label: string;
  icon: LucideIcon;
  roles: Role[];
}
const NAV: { group: string; items: NavItem[] }[] = [
  {
    group: "控制台",
    items: [
      { to: "/overview", label: "Overview", icon: LayoutDashboard, roles: ["admin"] },
      { to: "/workers", label: "Workers", icon: Server, roles: ["admin"] },
      { to: "/sessions", label: "Sessions", icon: MessagesSquare, roles: ["admin"] },
    ],
  },
  {
    group: "可观测",
    items: [
      { to: "/logs", label: "Logs", icon: ScrollText, roles: ["admin"] },
      { to: "/metrics", label: "Metrics", icon: Activity, roles: ["admin"] },
      { to: "/audit", label: "Audit", icon: History, roles: ["admin"] },
    ],
  },
  {
    group: "账户",
    items: [
      { to: "/dashboard", label: "Dashboard", icon: Wallet, roles: ["user", "admin"] },
      { to: "/keys", label: "API Keys", icon: KeyRound, roles: ["user", "admin"] },
      { to: "/security", label: "Security", icon: ShieldCheck, roles: ["user", "admin"] },
      { to: "/recharge", label: "Recharge", icon: CreditCard, roles: ["user"] },
      { to: "/users", label: "Users", icon: Users, roles: ["admin"] },
      { to: "/billing", label: "Billing", icon: CreditCard, roles: ["admin"] },
    ],
  },
];

const TITLES: Record<string, string> = {
  "/overview": "Overview",
  "/workers": "Workers",
  "/sessions": "Sessions",
  "/logs": "Logs",
  "/metrics": "Metrics",
  "/audit": "Audit",
  "/users": "Users",
  "/billing": "Billing",
  "/dashboard": "Dashboard",
  "/keys": "API Keys",
  "/security": "Security",
  "/recharge": "Recharge",
};

function Sidebar() {
  const { role, username, logout, isAdmin } = useAuth();
  const myRole: Role = isAdmin ? "admin" : "user";
  return (
    <aside className="dark flex h-full flex-col bg-ink-850 px-3.5 py-4 text-foreground">
      <div className="flex items-center gap-2.5 px-2 pb-4">
        <Logo variant="onDark" size={30} />
        <Wordmark onDark />
      </div>
      <nav className="flex-1 space-y-1">
        {NAV.map((g) => {
          const items = g.items.filter((i) => i.roles.includes(myRole));
          if (!items.length) return null;
          return (
            <div key={g.group} className="mt-2">
              <div className="px-2.5 pb-1.5 pt-2 text-[10px] font-bold uppercase tracking-widest text-ink-soft/70">
                {g.group}
              </div>
              {items.map((it) => (
                <NavLink
                  key={it.to}
                  to={it.to}
                  className={({ isActive }) =>
                    cn(
                      "relative flex items-center gap-3 rounded-lg px-2.5 py-2 text-[13.5px] font-medium transition-colors",
                      isActive
                        ? "bg-[rgba(245,167,62,0.13)] text-white"
                        : "text-ink-soft hover:bg-ink-700 hover:text-gray-200",
                    )
                  }
                >
                  {({ isActive }) => (
                    <>
                      {isActive && (
                        <span className="absolute -left-3.5 top-1.5 bottom-1.5 w-[3px] rounded-r bg-brand-amber" />
                      )}
                      <it.icon className="h-[17px] w-[17px]" />
                      {it.label}
                    </>
                  )}
                </NavLink>
              ))}
            </div>
          );
        })}
      </nav>
      <div className="mt-auto flex items-center gap-2.5 border-t border-ink-line pt-3">
        <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-brand-amber text-sm font-bold text-ink-900">
          {(username ?? "?")[0]?.toUpperCase()}
        </div>
        <div className="text-[12.5px] leading-tight text-gray-200">
          {role ?? "—"}
          <small className="block font-mono text-[10.5px] text-ink-soft/70">{username?.slice(0, 8)}</small>
        </div>
        <button onClick={logout} className="ml-auto text-ink-soft hover:text-white" title="退出">
          <LogOut className="h-4 w-4" />
        </button>
      </div>
    </aside>
  );
}

function Topbar() {
  const loc = useLocation();
  const qc = useQueryClient();
  const { role } = useAuth();
  const workers = useWorkers();
  const onlineCount = (workers.data ?? []).filter((w) => w.online).length;
  const ok = onlineCount > 0;
  const title = TITLES[loc.pathname] ?? "sub2org";
  return (
    <div className="flex items-center gap-4 border-b border-border bg-card px-6 py-3.5">
      <div className="flex-1">
        <h2 className="text-[19px] font-semibold text-foreground">{title}</h2>
        <div className="text-[12.5px] text-muted-foreground">单实例 · worker 池中转控制中心</div>
      </div>
      <HealthPill ok={ok} label={ok ? `${onlineCount} workers online` : "no workers"} />
      <span className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-muted px-2.5 py-1.5 font-mono text-[12px] text-muted-foreground">
        <ShieldCheck className="h-3.5 w-3.5" />
        {role} authed
      </span>
      <Button variant="outline" size="icon" onClick={() => qc.invalidateQueries()} title="刷新">
        <RefreshCw className="h-4 w-4" />
      </Button>
    </div>
  );
}

export function AppShell() {
  return (
    <div className="grid h-screen grid-cols-[236px_1fr]">
      <Sidebar />
      <section className="flex min-w-0 flex-col bg-background">
        <Topbar />
        <main className="flex-1 overflow-auto px-6 py-5">
          <Outlet />
        </main>
      </section>
    </div>
  );
}
