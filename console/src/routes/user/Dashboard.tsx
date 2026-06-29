import { Wallet } from "lucide-react";
import { Badge, Card, CardHeader, CardTitle } from "@/components/ui";
import { DataTable, type Column } from "@/components/common";
import { useBalance, useTransactions } from "@/hooks";
import { num, relSecs } from "@/lib/format";
import type { Tx, TxKind } from "@/api/types";

const tone: Record<TxKind, "green" | "red" | "blue" | "amber"> = {
  recharge: "green",
  redeem: "green",
  refund: "blue",
  charge: "amber",
};

const cols: Column<Tx>[] = [
  { key: "kind", header: "Type", render: (t) => <Badge tone={tone[t.kind]}>{t.kind}</Badge> },
  { key: "amount", header: "Amount", render: (t) => <span className={t.amount >= 0 ? "text-green-600" : "text-red-600"}>{t.amount >= 0 ? "+" : ""}{num(t.amount)}</span> },
  { key: "after", header: "Balance", render: (t) => <span className="font-mono">{num(t.balance_after)}</span> },
  { key: "model", header: "Model", render: (t) => t.model ?? "—" },
  { key: "ts", header: "Time", render: (t) => <span className="font-mono text-[11.5px] text-muted-foreground">{relSecs(t.created_at)}</span> },
];

export function Dashboard() {
  const bal = useBalance();
  const tx = useTransactions();
  return (
    <div className="space-y-4">
      <Card>
        <div className="flex items-center gap-4 p-5">
          <span className="flex h-12 w-12 items-center justify-center rounded-xl bg-[#fef3e2] text-brand-amber">
            <Wallet className="h-6 w-6" />
          </span>
          <div>
            <div className="text-xs text-muted-foreground">余额（credits）</div>
            <div className="text-3xl font-bold tracking-tight text-foreground">{bal.data ? num(bal.data.balance) : "—"}</div>
          </div>
        </div>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>流水</CardTitle>
        </CardHeader>
        <DataTable columns={cols} rows={tx.data ?? []} loading={tx.isLoading} empty="暂无流水" />
      </Card>
    </div>
  );
}
