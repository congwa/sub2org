import { useState } from "react";
import { Badge, Button, Card, CardContent, CardHeader, CardTitle, Input, Label } from "@/components/ui";
import { DataTable, type Column } from "@/components/common";
import { useBanUser, useCreateUser, useRechargeUser, useUnbanUser, useUsers } from "@/hooks";
import { num, relSecs } from "@/lib/format";
import type { User } from "@/api/types";

export function UsersPage() {
  const users = useUsers();
  const ban = useBanUser();
  const unban = useUnbanUser();
  const recharge = useRechargeUser();
  const create = useCreateUser();
  const [open, setOpen] = useState(false);
  const [u, setU] = useState("");
  const [p, setP] = useState("");
  const [bal, setBal] = useState("0");

  function doRecharge(id: string) {
    const v = window.prompt("充值额度（credits）", "100");
    if (!v) return;
    const amount = parseInt(v, 10);
    if (!Number.isNaN(amount)) recharge.mutate({ id, amount, note: "console" });
  }

  async function submitCreate(e: React.FormEvent) {
    e.preventDefault();
    await create.mutateAsync({ username: u, password: p, initial_balance: parseInt(bal, 10) || 0 });
    setU("");
    setP("");
    setBal("0");
    setOpen(false);
  }

  const cols: Column<User>[] = [
    { key: "username", header: "User", render: (x) => <span className="font-medium">{x.username}</span> },
    { key: "role", header: "Role", render: (x) => <Badge tone={x.role === "admin" ? "amber" : "neutral"}>{x.role}</Badge> },
    { key: "status", header: "Status", render: (x) => <Badge tone={x.status === "active" ? "green" : "red"}>{x.status}</Badge> },
    { key: "balance", header: "Balance", render: (x) => <span className="font-mono">{num(x.balance)}</span> },
    { key: "rl", header: "Rate limit", render: (x) => (x.rate_limit ? x.rate_limit : "∞") },
    { key: "created", header: "Created", render: (x) => <span className="font-mono text-[11.5px] text-muted-foreground">{relSecs(x.created_at)}</span> },
    {
      key: "act",
      header: "",
      className: "text-right",
      render: (x) => (
        <div className="flex justify-end gap-1.5">
          <Button size="xs" variant="outline" onClick={() => doRecharge(x.id)}>
            充值
          </Button>
          {x.status === "active" ? (
            <Button size="xs" variant="outline" onClick={() => ban.mutate(x.id)}>
              封禁
            </Button>
          ) : (
            <Button size="xs" variant="outline" onClick={() => unban.mutate(x.id)}>
              解封
            </Button>
          )}
        </div>
      ),
    },
  ];

  return (
    <div className="space-y-4">
      <div className="flex justify-end">
        <Button onClick={() => setOpen((v) => !v)}>{open ? "取消" : "+ 新建用户"}</Button>
      </div>
      {open && (
        <Card>
          <CardHeader>
            <CardTitle>新建用户</CardTitle>
          </CardHeader>
          <CardContent>
            <form onSubmit={submitCreate} className="flex flex-wrap items-end gap-3">
              <div className="space-y-1.5">
                <Label>用户名</Label>
                <Input value={u} onChange={(e) => setU(e.target.value)} className="w-44" />
              </div>
              <div className="space-y-1.5">
                <Label>密码</Label>
                <Input type="password" value={p} onChange={(e) => setP(e.target.value)} className="w-44" />
              </div>
              <div className="space-y-1.5">
                <Label>初始余额</Label>
                <Input value={bal} onChange={(e) => setBal(e.target.value)} className="w-28" />
              </div>
              <Button type="submit" disabled={create.isPending}>
                创建
              </Button>
            </form>
          </CardContent>
        </Card>
      )}
      <Card>
        <DataTable columns={cols} rows={users.data ?? []} loading={users.isLoading} empty="暂无用户" />
      </Card>
    </div>
  );
}
