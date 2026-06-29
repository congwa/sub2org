import * as React from "react";
import type { LucideIcon } from "lucide-react";
import { ArrowDownRight, ArrowUpRight, Minus } from "lucide-react";
import { cn } from "@/lib/cn";
import { Badge, Table, TBody, TD, TH, THead, TR, Skeleton } from "./ui";
import type { WorkerStatus } from "@/api/types";

/* ---------- StatusBadge ---------- */
const STATUS: Record<WorkerStatus, { tone: "green" | "blue" | "amber" | "red" | "neutral"; dot: string; pulse?: boolean }> = {
  idle: { tone: "green", dot: "bg-green-500" },
  busy: { tone: "blue", dot: "bg-blue-500", pulse: true },
  cooling: { tone: "amber", dot: "bg-amber-500" },
  logged_out: { tone: "neutral", dot: "bg-gray-400" },
  offline: { tone: "neutral", dot: "bg-gray-400" },
  disabled: { tone: "red", dot: "bg-red-500" },
  tripped: { tone: "red", dot: "bg-red-500", pulse: true }, // B4 熔断
};
export function StatusBadge({ status }: { status: WorkerStatus }) {
  const s = STATUS[status] ?? STATUS.offline;
  return (
    <Badge tone={s.tone}>
      <span className={cn("h-1.5 w-1.5 rounded-full", s.dot, s.pulse && "animate-pulse-dot")} />
      {status}
    </Badge>
  );
}

/* ---------- ProviderGlyph ---------- */
export function ProviderGlyph({ provider }: { provider: string }) {
  const isClaude = provider.toLowerCase().includes("claude");
  return (
    <span className="inline-flex items-center gap-2 text-[12.5px]">
      <span
        className="inline-flex h-5 w-5 items-center justify-center rounded text-[11px] font-bold text-white"
        style={{ background: isClaude ? "#d97757" : "#10a37f" }}
      >
        {isClaude ? "C" : "G"}
      </span>
      {isClaude ? "Claude" : "ChatGPT"}
    </span>
  );
}

/* ---------- LoadBar ---------- */
export function LoadBar({ value, max, color = "var(--color-blue-500)" }: { value: number; max: number; color?: string }) {
  const pct = max > 0 ? Math.min(100, (value / max) * 100) : 0;
  return (
    <div className="flex items-center gap-2">
      <div className="h-1.5 w-14 overflow-hidden rounded bg-gray-200">
        <span className="block h-full rounded" style={{ width: `${pct}%`, background: color }} />
      </div>
      <span className="font-mono text-[11px] text-muted-foreground">
        {value}/{max}
      </span>
    </div>
  );
}

/* ---------- KpiCard ---------- */
const TONE: Record<string, string> = {
  green: "bg-green-100 text-green-600",
  blue: "bg-blue-100 text-blue-600",
  amber: "bg-amber-100 text-amber-600",
  orange: "bg-[#fef3e2] text-brand-amber",
};
export function KpiCard({
  icon: Icon,
  tone = "blue",
  label,
  value,
  unit,
  delta,
}: {
  icon: LucideIcon;
  tone?: "green" | "blue" | "amber" | "orange";
  label: string;
  value: React.ReactNode;
  unit?: string;
  delta?: { dir: "up" | "down" | "flat"; label: string };
}) {
  const DeltaIcon = delta?.dir === "up" ? ArrowUpRight : delta?.dir === "down" ? ArrowDownRight : Minus;
  const deltaColor =
    delta?.dir === "up" ? "text-green-600" : delta?.dir === "down" ? "text-red-600" : "text-muted-foreground";
  return (
    <div className="rounded-xl border border-border bg-card p-4 shadow-sm">
      <div className="mb-3 flex items-center gap-2.5">
        <span className={cn("flex h-8 w-8 items-center justify-center rounded-lg", TONE[tone])}>
          <Icon className="h-4 w-4" />
        </span>
        <span className="text-xs font-medium text-muted-foreground">{label}</span>
      </div>
      <div className="text-[27px] font-bold leading-none tracking-tight text-foreground">
        {value}
        {unit && <span className="ml-1 text-sm font-medium text-muted-foreground">{unit}</span>}
      </div>
      {delta && (
        <div className={cn("mt-2 inline-flex items-center gap-1 text-[11.5px] font-semibold", deltaColor)}>
          <DeltaIcon className="h-3 w-3" />
          {delta.label}
        </div>
      )}
    </div>
  );
}

/* ---------- Sparkline (amber bars) ---------- */
export function Sparkline({ data, height = 36 }: { data: number[]; height?: number }) {
  const max = Math.max(1, ...data);
  return (
    <div className="flex items-end gap-[3px]" style={{ height }}>
      {data.map((v, i) => (
        <span
          key={i}
          className="flex-1 rounded-t-sm bg-brand-amber opacity-85"
          style={{ height: `${Math.max(3, (v / max) * 100)}%` }}
        />
      ))}
    </div>
  );
}

/* ---------- DataTable (generic) ---------- */
export interface Column<T> {
  key: string;
  header: string;
  render?: (row: T) => React.ReactNode;
  className?: string;
}
export function DataTable<T>({
  columns,
  rows,
  loading,
  empty = "暂无数据",
}: {
  columns: Column<T>[];
  rows: T[];
  loading?: boolean;
  empty?: string;
}) {
  return (
    <Table>
      <THead>
        <TR className="hover:bg-transparent">
          {columns.map((c) => (
            <TH key={c.key} className={c.className}>
              {c.header}
            </TH>
          ))}
        </TR>
      </THead>
      <TBody>
        {loading ? (
          Array.from({ length: 4 }).map((_, i) => (
            <TR key={i} className="hover:bg-transparent">
              {columns.map((c) => (
                <TD key={c.key}>
                  <Skeleton className="h-4 w-20" />
                </TD>
              ))}
            </TR>
          ))
        ) : rows.length === 0 ? (
          <TR className="hover:bg-transparent">
            <TD colSpan={columns.length} className="py-8 text-center text-muted-foreground">
              {empty}
            </TD>
          </TR>
        ) : (
          rows.map((r, i) => (
            <TR key={i}>
              {columns.map((c) => (
                <TD key={c.key} className={c.className}>
                  {c.render ? c.render(r) : ((r as Record<string, unknown>)[c.key] as React.ReactNode)}
                </TD>
              ))}
            </TR>
          ))
        )}
      </TBody>
    </Table>
  );
}

/* ---------- HealthPill ---------- */
export function HealthPill({ ok, label }: { ok: boolean; label: string }) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[12.5px] font-semibold",
        ok ? "bg-green-100 text-green-700" : "bg-amber-100 text-amber-600",
      )}
    >
      <span className={cn("h-1.5 w-1.5 rounded-full", ok ? "bg-green-500 animate-pulse-dot" : "bg-amber-500")} />
      {label}
    </span>
  );
}
