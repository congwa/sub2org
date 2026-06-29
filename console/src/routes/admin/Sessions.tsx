import { Card } from "@/components/ui";
import { DataTable, ProviderGlyph, type Column } from "@/components/common";
import { useSessions } from "@/hooks";
import { relSecs, shortId } from "@/lib/format";
import type { Session } from "@/api/types";

const cols: Column<Session>[] = [
  { key: "sid", header: "Session", render: (s) => <span className="font-mono text-[12px]">{s.session_id}</span> },
  { key: "user", header: "User", render: (s) => <span className="font-mono text-[11.5px] text-muted-foreground">{shortId(s.api_key_id, 8, 4)}</span> },
  { key: "worker", header: "Worker", render: (s) => <span className="font-mono text-[12px]">{s.worker_id}</span> },
  { key: "provider", header: "Provider", render: (s) => <ProviderGlyph provider={s.provider} /> },
  { key: "conv", header: "Web conversation", render: (s) => <span className="font-mono text-[11.5px] text-muted-foreground">{shortId(s.native_conversation_id, 10, 4)}</span> },
  { key: "used", header: "Last used", render: (s) => <span className="font-mono text-[11.5px] text-muted-foreground">{relSecs(s.last_used_at)}</span> },
];

export function Sessions() {
  const q = useSessions();
  return (
    <Card>
      <DataTable columns={cols} rows={q.data ?? []} loading={q.isLoading} empty="暂无会话" />
    </Card>
  );
}
