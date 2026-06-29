import { Card } from "@/components/ui";
import { Button } from "@/components/ui";
import { DataTable, LoadBar, ProviderGlyph, StatusBadge, type Column } from "@/components/common";
import { useDisableWorker, useEnableWorker, useWorkers } from "@/hooks";
import { num, relSecs } from "@/lib/format";
import type { WorkerView } from "@/api/types";

export function Workers() {
  const workers = useWorkers();
  const disable = useDisableWorker();
  const enable = useEnableWorker();

  const cols: Column<WorkerView>[] = [
    {
      key: "worker",
      header: "Worker",
      render: (w) => (
        <div>
          <div className="font-mono text-[12px] font-medium text-foreground">{w.worker_id}</div>
          <div className="text-[11.5px] text-muted-foreground">
            {w.account_label || "—"}
            {w.group ? <span className="ml-1.5 rounded bg-muted px-1.5 py-0.5 text-[10px] text-foreground">{w.group}</span> : null}
          </div>
        </div>
      ),
    },
    { key: "provider", header: "Provider", render: (w) => <ProviderGlyph provider={w.provider} /> },
    { key: "status", header: "Status", render: (w) => <StatusBadge status={w.status} /> },
    {
      key: "load",
      header: "Load",
      render: (w) => (w.online ? <LoadBar value={w.in_flight} max={w.capacity} /> : <span className="text-muted-foreground">—</span>),
    },
    { key: "served", header: "Reqs", render: (w) => num(w.total_served) },
    { key: "hb", header: "Heartbeat", render: (w) => <span className="font-mono text-[11.5px] text-muted-foreground">{relSecs(w.last_seen_at)}</span> },
    {
      key: "act",
      header: "",
      className: "text-right",
      render: (w) =>
        w.disabled || w.status === "disabled" ? (
          <Button size="xs" variant="outline" onClick={() => enable.mutate(w.worker_id)}>
            启用
          </Button>
        ) : (
          <Button size="xs" variant="outline" onClick={() => disable.mutate(w.worker_id)}>
            禁用
          </Button>
        ),
    },
  ];

  return (
    <Card>
      <DataTable columns={cols} rows={workers.data ?? []} loading={workers.isLoading} empty="暂无 worker（启动 sub2org-worker 扩展或 mock）" />
    </Card>
  );
}
