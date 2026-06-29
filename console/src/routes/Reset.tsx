import { useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { useReset } from "@/hooks";
import { Logo, Wordmark } from "@/components/Logo";
import { Button, Card, CardContent, Input, Label } from "@/components/ui";
import { ApiError } from "@/api/errors";

export function Reset() {
  const [sp] = useSearchParams();
  const token = sp.get("token") ?? "";
  const nav = useNavigate();
  const resetM = useReset();
  const [p, setP] = useState("");
  const [done, setDone] = useState(false);
  const [err, setErr] = useState("");

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setErr("");
    if (!token) {
      setErr("链接无效或缺少 token");
      return;
    }
    try {
      await resetM.mutateAsync({ token, password: p });
      setDone(true);
      setTimeout(() => nav("/login", { replace: true }), 1500);
    } catch (e) {
      setErr(e instanceof ApiError ? e.message : "重置失败，链接可能已过期");
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-[#eef0f3] px-4">
      <Card className="w-full max-w-sm">
        <CardContent className="p-7">
          <div className="mb-6 flex items-center gap-3">
            <Logo size={40} />
            <div>
              <Wordmark />
              <div className="text-xs text-muted-foreground">重置密码</div>
            </div>
          </div>
          {done ? (
            <div className="rounded-lg bg-green-100 px-3 py-2 text-[13px] text-green-700">
              密码已重置，正在跳转登录…
            </div>
          ) : (
            <form onSubmit={submit} className="space-y-4">
              <div className="space-y-1.5">
                <Label>新密码</Label>
                <Input type="password" value={p} onChange={(e) => setP(e.target.value)} placeholder="至少 6 位" autoFocus />
              </div>
              {err && <div className="rounded-lg bg-red-50 px-3 py-2 text-[13px] text-red-600">{err}</div>}
              <Button type="submit" className="w-full" disabled={resetM.isPending || p.length < 6}>
                {resetM.isPending ? "提交中…" : "重置密码"}
              </Button>
            </form>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
