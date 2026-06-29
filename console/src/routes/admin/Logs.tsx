import { useState } from "react";
import { Badge, Card, CardHeader, CardTitle, Input } from "@/components/ui";
import { DataTable, type Column } from "@/components/common";
import { useLogs } from "@/hooks";
import { relMs } from "@/lib/format";
import type { LogEntry, LogLevel } from "@/api/types";
import { cn } from "@/lib/cn";

const LEVEL_TONE: Record<LogLevel, string> = {
  trace: "text-gray-400",
  debug: "text-gray-500",
  info: "text-blue-600",
  warn: "text-amber-600",
  error: "text-red-600",
};
const sel = "h-9 rounded-lg border border-input bg-card px-2 text-sm text-foreground";

export function Logs() {
  const [source, setSource] = useState("");
  const [category, setCategory] = useState("");
  const [level, setLevel] = useState("");
  const [reqId, setReqId] = useState("");
  const q = useLogs({ source, category, level, req_id: reqId, limit: 300 });

  const cols: Column<LogEntry>[] = [
    { key: "ts", header: "Time", render: (l) => <span className="font-mono text-[11.5px] text-muted-foreground">{relMs(l.ts_ms)}</span> },
    { key: "source", header: "Source", render: (l) => <Badge tone={l.source === "worker" ? "amber" : "blue"}>{l.source}</Badge> },
    { key: "cat", header: "Category", render: (l) => <span className="font-mono text-[12px]">{l.category}</span> },
    { key: "level", header: "Level", render: (l) => <span className={cn("font-mono text-[12px] font-semibold uppercase", LEVEL_TONE[l.level])}>{l.level}</span> },
    { key: "msg", header: "Message", render: (l) => <span className="text-[12.5px]">{l.message}</span> },
    { key: "who", header: "Worker / req", render: (l) => <span className="font-mono text-[11px] text-muted-foreground">{l.worker_id ?? l.req_id ?? "—"}</span> },
  ];

  return (
    <Card>
      <CardHeader>
        <CardTitle>Logs</CardTitle>
        <div className="flex items-center gap-2">
          <select className={sel} value={source} onChange={(e) => setSource(e.target.value)}>
            <option value="">all sources</option>
            <option value="server">server</option>
            <option value="worker">worker</option>
          </select>
          <select className={sel} value={level} onChange={(e) => setLevel(e.target.value)}>
            <option value="">all levels</option>
            {["info", "warn", "error", "debug"].map((l) => (
              <option key={l} value={l}>
                ≥ {l}
              </option>
            ))}
          </select>
          <Input className="h-9 w-32" placeholder="category" value={category} onChange={(e) => setCategory(e.target.value)} />
          <Input className="h-9 w-40" placeholder="req_id" value={reqId} onChange={(e) => setReqId(e.target.value)} />
        </div>
      </CardHeader>
      <DataTable columns={cols} rows={q.data?.logs ?? []} loading={q.isLoading} empty="暂无日志" />
    </Card>
  );
}
