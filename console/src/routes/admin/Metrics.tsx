import { MessagesSquare, Server, Timer, Zap } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui";
import { KpiCard, Sparkline } from "@/components/common";
import { useMetrics } from "@/hooks";
import { ms, num } from "@/lib/format";

export function Metrics() {
  const q = useMetrics();
  const m = q.data;
  const series = m?.throughput.points.map((p) => p.count) ?? [];
  return (
    <div className="space-y-4">
      <div className="grid grid-cols-4 gap-3.5">
        <KpiCard icon={Server} tone="green" label="Online / total workers" value={m ? `${m.workers.online}/${m.workers.total}` : "—"} />
        <KpiCard icon={MessagesSquare} tone="blue" label="Active sessions" value={m?.active_sessions ?? "—"} />
        <KpiCard icon={Zap} tone="orange" label="Requests · 24h" value={m ? num(m.requests_24h) : "—"} />
        <KpiCard icon={Timer} tone="amber" label="Avg queue wait" value={m ? ms(m.avg_queue_wait_ms ?? 0) : "—"} />
      </div>
      <Card>
        <CardHeader>
          <CardTitle>Throughput · 60m</CardTitle>
          <span className="text-xs text-muted-foreground">req / min · peak {series.length ? Math.max(...series) : 0}</span>
        </CardHeader>
        <CardContent>
          <Sparkline data={series.length ? series : Array(60).fill(0)} height={120} />
        </CardContent>
      </Card>
    </div>
  );
}
