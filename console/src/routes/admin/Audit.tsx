import { useState } from "react";
import { Badge, Card, CardHeader, CardTitle, Input } from "@/components/ui";
import { DataTable, type Column } from "@/components/common";
import { useAudit } from "@/hooks";
import { relSecs, shortId } from "@/lib/format";
import type { AuditRow } from "@/api/types";

const cols: Column<AuditRow>[] = [
  { key: "created", header: "时间", render: (r) => <span className="font-mono text-[11.5px] text-muted-foreground">{relSecs(r.created_at)}</span> },
  { key: "actor", header: "操作者", render: (r) => <Badge tone="neutral">{r.actor}</Badge> },
  { key: "action", header: "动作", render: (r) => <span className="font-mono text-[12px]">{r.action}</span> },
  { key: "target", header: "对象", render: (r) => <span className="font-mono text-[11.5px] text-muted-foreground">{r.target ? shortId(r.target, 8, 4) : "—"}</span> },
  {
    key: "detail",
    header: "详情",
    render: (r) => {
      const s = JSON.stringify(r.detail ?? {});
      return <span className="font-mono text-[11.5px] text-muted-foreground">{s === "{}" ? "—" : s}</span>;
    },
  },
];

export function Audit() {
  const [actor, setActor] = useState("");
  const [action, setAction] = useState("");
  const audit = useAudit({
    actor: actor.trim() || undefined,
    action: action.trim() || undefined,
    limit: 300,
  });

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader>
          <CardTitle>管理操作审计</CardTitle>
        </CardHeader>
        <div className="flex flex-wrap gap-3 px-5 pb-2">
          <Input value={actor} onChange={(e) => setActor(e.target.value)} placeholder="按操作者过滤（如 jwt:admin）" className="w-56" />
          <Input value={action} onChange={(e) => setAction(e.target.value)} placeholder="按动作过滤（如 recharge）" className="w-56" />
        </div>
        <DataTable columns={cols} rows={audit.data?.audit ?? []} loading={audit.isLoading} empty="暂无审计记录" />
      </Card>
    </div>
  );
}
