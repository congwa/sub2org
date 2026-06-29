import { useState } from "react";
import { Badge, Button, Card, CardContent, CardHeader, CardTitle, Input, Label } from "@/components/ui";
import { DataTable, type Column } from "@/components/common";
import { useCreateRedeemCodes, useRedeemCodes, useUsageByModel, useUsageByUser, useUsageStats } from "@/hooks";
import { num, relSecs, shortId } from "@/lib/format";
import type { ModelUsageRow, RedeemCode, UserUsageRow } from "@/api/types";

const cols: Column<RedeemCode>[] = [
  { key: "code", header: "Code", render: (c) => <span className="font-mono text-[12px]">{c.code}</span> },
  { key: "amount", header: "Amount", render: (c) => <span className="font-mono">{num(c.amount)}</span> },
  {
    key: "used",
    header: "Status",
    render: (c) => (c.used_by ? <Badge tone="neutral">used · {shortId(c.used_by, 6, 3)}</Badge> : <Badge tone="green">unused</Badge>),
  },
  { key: "created", header: "Created", render: (c) => <span className="font-mono text-[11.5px] text-muted-foreground">{relSecs(c.created_at)}</span> },
];

const userCols: Column<UserUsageRow>[] = [
  { key: "username", header: "用户", render: (r) => <span className="font-mono text-[12px]">{r.username || shortId(r.user_id, 6, 3)}</span> },
  { key: "spent", header: "消费", render: (r) => <span className="font-mono">{num(r.spent)}</span> },
  { key: "refunded", header: "退款", render: (r) => <span className="font-mono">{num(r.refunded)}</span> },
  { key: "count", header: "笔数", render: (r) => <span className="font-mono">{num(r.count)}</span> },
];

const modelCols: Column<ModelUsageRow>[] = [
  { key: "model", header: "模型", render: (r) => <span className="font-mono text-[12px]">{r.model}</span> },
  { key: "spent", header: "消费", render: (r) => <span className="font-mono">{num(r.spent)}</span> },
  { key: "count", header: "笔数", render: (r) => <span className="font-mono">{num(r.count)}</span> },
];

export function Billing() {
  const codes = useRedeemCodes();
  const create = useCreateRedeemCodes();
  const [amount, setAmount] = useState("100");
  const [count, setCount] = useState("10");
  const [generated, setGenerated] = useState<string[]>([]);

  async function gen(e: React.FormEvent) {
    e.preventDefault();
    const r = await create.mutateAsync({ amount: parseInt(amount, 10) || 0, count: parseInt(count, 10) || 1 });
    setGenerated(r.codes);
  }

  const usage = useUsageStats();
  const topUsers = useUsageByUser({ limit: 20 });
  const byModel = useUsageByModel();
  const u = usage.data;
  const stats: { label: string; value: number; tone?: string }[] = [
    { label: "净消耗", value: u?.net_consumed_credits ?? 0, tone: "text-foreground" },
    { label: "扣费", value: u?.spent_credits ?? 0 },
    { label: "退款", value: u?.refunded_credits ?? 0 },
    { label: "充值入账", value: u?.recharged_credits ?? 0, tone: "text-green-600" },
    { label: "兑换入账", value: u?.redeemed_credits ?? 0, tone: "text-green-600" },
  ];

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader>
          <CardTitle>近 24h 概览（积分）</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-5">
            {stats.map((s) => (
              <div key={s.label} className="rounded-lg border border-border bg-muted/30 p-3">
                <div className="text-xs text-muted-foreground">{s.label}</div>
                <div className={`mt-1 font-mono text-lg ${s.tone ?? "text-foreground"}`}>{num(s.value)}</div>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>
      <div className="grid gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Top 用户消费（近 24h）</CardTitle>
          </CardHeader>
          <DataTable columns={userCols} rows={topUsers.data?.by_user ?? []} loading={topUsers.isLoading} empty="暂无消费" />
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>按模型消费（近 24h）</CardTitle>
          </CardHeader>
          <DataTable columns={modelCols} rows={byModel.data?.by_model ?? []} loading={byModel.isLoading} empty="暂无消费" />
        </Card>
      </div>
      <Card>
        <CardHeader>
          <CardTitle>生成兑换码</CardTitle>
        </CardHeader>
        <CardContent>
          <form onSubmit={gen} className="flex flex-wrap items-end gap-3">
            <div className="space-y-1.5">
              <Label>每张额度</Label>
              <Input value={amount} onChange={(e) => setAmount(e.target.value)} className="w-32" />
            </div>
            <div className="space-y-1.5">
              <Label>数量</Label>
              <Input value={count} onChange={(e) => setCount(e.target.value)} className="w-24" />
            </div>
            <Button type="submit" disabled={create.isPending}>
              生成
            </Button>
          </form>
          {generated.length > 0 && (
            <div className="mt-4 rounded-lg border border-border bg-muted/40 p-3">
              <div className="mb-2 text-xs text-muted-foreground">新生成（请复制保存）：</div>
              <div className="grid grid-cols-2 gap-1.5 font-mono text-[12px] md:grid-cols-3">
                {generated.map((c) => (
                  <span key={c} className="rounded bg-card px-2 py-1">
                    {c}
                  </span>
                ))}
              </div>
            </div>
          )}
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>兑换码</CardTitle>
        </CardHeader>
        <DataTable columns={cols} rows={codes.data ?? []} loading={codes.isLoading} empty="暂无兑换码" />
      </Card>
    </div>
  );
}
