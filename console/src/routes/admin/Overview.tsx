import { AlertTriangle, MessagesSquare, Server, Timer, Zap } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui";
import { DataTable, KpiCard, LoadBar, ProviderGlyph, Sparkline, StatusBadge, type Column } from "@/components/common";
import { useMetrics, useWorkers, useAdminEvents, type FeedItem } from "@/hooks";
import { ms, num, relSecs, shortId } from "@/lib/format";
import type { WorkerView } from "@/api/types";
import { cn } from "@/lib/cn";

const cols: Column<WorkerView>[] = [
  {
    key: "worker",
    header: "Worker",
    render: (w) => (
      <div>
        <div className="font-mono text-[12px] font-medium text-foreground">{w.worker_id}</div>
        <div className="text-[11.5px] text-muted-foreground">{w.account_label || "—"}</div>
      </div>
    ),
  },
  { key: "provider", header: "Provider", render: (w) => <ProviderGlyph provider={w.provider} /> },
  { key: "status", header: "Status", render: (w) => <StatusBadge status={w.status} /> },
  {
    key: "load",
    header: "Load",
    render: (w) =>
      w.online ? <LoadBar value={w.in_flight} max={w.capacity} /> : <span className="text-muted-foreground">—</span>,
  },
  { key: "served", header: "Reqs", render: (w) => num(w.total_served) },
  { key: "hb", header: "Heartbeat", render: (w) => <span className="font-mono text-[11.5px] text-muted-foreground">{relSecs(w.last_seen_at)}</span> },
];

function feedLine(f: FeedItem) {
  if (f.type === "request_dispatched")
    return { mdl: f.model, worker: `→ ${f.worker_id}`, rid: f.req_id, state: "streaming" as const, extra: shortId(f.session_id) };
  if (f.type === "request_done")
    return { mdl: "", worker: "", rid: f.req_id, state: "stop" as const, extra: ms(f.latency_ms) };
  if (f.type === "request_error")
    return { mdl: "", worker: "", rid: f.req_id, state: "error" as const, extra: f.code };
  if (f.type === "balance_low")
    return { mdl: "", worker: "余额低", rid: shortId(f.user_id), state: "error" as const, extra: `${f.balance}<${f.threshold}` };
  return { mdl: "", worker: `${f.worker_id}`, rid: "", state: "status" as const, extra: f.status };
}

export function Overview() {
  const metrics = useMetrics();
  const workers = useWorkers();
  const { events, connection } = useAdminEvents();
  const m = metrics.data;
  const series = m?.throughput.points.map((p) => p.count) ?? [];

  return (
    <div className="space-y-4">
      {m?.storage_backend === "memory" && (
        <div className="flex items-start gap-2.5 rounded-lg border border-amber-300 bg-amber-50 px-4 py-3 text-[13px] text-amber-900 dark:border-amber-700/60 dark:bg-amber-950/40 dark:text-amber-200">
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
          <div>
            <span className="font-semibold">内存模式（无持久化）</span>
            ：重启将丢失全部用户 / API key / 会话 / 计费 / 审计——客户端会报「key 失效需换新」。对外运营请在 <code className="font-mono">config.toml</code> 配 <code className="font-mono">database_url</code>（Postgres）。
          </div>
        </div>
      )}

      <div className="grid grid-cols-4 gap-3.5">
        <KpiCard icon={Server} tone="green" label="Online workers" value={m?.workers.online ?? "—"} unit={m ? `/ ${m.workers.total}` : undefined} />
        <KpiCard icon={MessagesSquare} tone="blue" label="Active sessions" value={m?.active_sessions ?? "—"} />
        <KpiCard icon={Zap} tone="orange" label="Requests · 24h" value={m ? num(m.requests_24h) : "—"} />
        <KpiCard icon={Timer} tone="amber" label="Avg queue wait" value={m ? ms(m.avg_queue_wait_ms ?? 0) : "—"} />
      </div>

      <div className="grid grid-cols-[1.7fr_1fr] gap-3.5">
        <Card>
          <CardHeader>
            <CardTitle>Worker pool</CardTitle>
            <span className="text-xs text-muted-foreground">{(workers.data ?? []).length} workers</span>
          </CardHeader>
          <DataTable columns={cols} rows={(workers.data ?? []).slice(0, 8)} loading={workers.isLoading} />
        </Card>

        <div className="flex min-w-0 flex-col gap-3.5">
          <Card>
            <CardHeader>
              <CardTitle>Throughput · 60m</CardTitle>
              <span className="text-xs text-muted-foreground">req / min</span>
            </CardHeader>
            <CardContent>
              <Sparkline data={series.length ? series : Array(40).fill(0)} />
              <div className="mt-2 flex justify-between font-mono text-[10.5px] text-muted-foreground">
                <span>-60m</span>
                <span>peak {series.length ? Math.max(...series) : 0}/m</span>
                <span>now</span>
              </div>
            </CardContent>
          </Card>

          <Card className="flex-1">
            <CardHeader>
              <CardTitle>Live requests</CardTitle>
              <span className={cn("text-xs", connection === "open" ? "text-green-600" : "text-muted-foreground")}>
                {connection === "open" ? "● live" : connection}
              </span>
            </CardHeader>
            <div className="divide-y divide-border">
              {events.length === 0 && <div className="px-4 py-8 text-center text-[13px] text-muted-foreground">等待事件…</div>}
              {events.slice(0, 12).map((f, i) => {
                const l = feedLine(f);
                const stColor =
                  l.state === "stop" ? "text-green-600" : l.state === "error" ? "text-red-600" : l.state === "streaming" ? "text-blue-600" : "text-muted-foreground";
                return (
                  <div key={i} className="flex items-center gap-2.5 px-4 py-2.5 text-[12px]">
                    {l.mdl && <span className="rounded bg-muted px-1.5 py-0.5 font-mono text-[10.5px] font-semibold">{l.mdl}</span>}
                    <span className="min-w-0 flex-1 truncate text-foreground">{l.worker || l.rid}</span>
                    <span className={cn("font-semibold", stColor)}>{l.state}</span>
                    <span className="font-mono text-[11px] text-muted-foreground">{l.extra}</span>
                  </div>
                );
              })}
            </div>
          </Card>
        </div>
      </div>
    </div>
  );
}
