import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Button, Card, CardContent, CardHeader, CardTitle, Input, Label } from "@/components/ui";
import { useCreatePayment, usePricing, useRedeem } from "@/hooks";
import { qk } from "@/hooks";
import { ep } from "@/api/endpoints";
import { num } from "@/lib/format";
import { ApiError } from "@/api/errors";
import { cn } from "@/lib/cn";

const PRESETS = [100, 500, 1000, 5000];
const CHANNELS: { key: string; label: string }[] = [
  { key: "alipay", label: "支付宝" },
  { key: "wxpay", label: "微信支付" },
];

export function Recharge() {
  const qc = useQueryClient();
  const pricing = usePricing();
  const onlineEnabled = pricing.data?.credits_per_cny != null;
  const cpc = pricing.data?.credits_per_cny ?? 100;

  // ---- 在线支付 ----
  const createPay = useCreatePayment();
  const [amount, setAmount] = useState(500);
  const [channel, setChannel] = useState("alipay");
  const [orderId, setOrderId] = useState<string | null>(null);
  const [payMsg, setPayMsg] = useState<{ ok: boolean; text: string } | null>(null);

  // 下单后轮询订单状态。
  const order = useQuery({
    queryKey: ["paymentOrder", orderId],
    queryFn: () => ep.paymentOrder(orderId!),
    enabled: !!orderId,
    refetchInterval: (q) => (q.state.data?.status === "paid" ? false : 3000),
  });

  useEffect(() => {
    if (order.data?.status === "paid") {
      setPayMsg({ ok: true, text: `充值成功，到账 ${num(order.data.amount)} 积分` });
      setOrderId(null);
      qc.invalidateQueries({ queryKey: qk.balance });
      qc.invalidateQueries({ queryKey: qk.transactions });
    }
  }, [order.data?.status, order.data?.amount, qc]);

  async function pay() {
    setPayMsg(null);
    try {
      const r = await createPay.mutateAsync({ amount, channel });
      window.open(r.pay_url, "_blank", "noopener");
      setOrderId(r.order_id);
      setPayMsg({ ok: true, text: `已创建订单 ${r.order_id}（¥${r.money}），在新标签页完成支付后此处自动到账…` });
    } catch (e) {
      setPayMsg({ ok: false, text: e instanceof ApiError ? e.message : "下单失败" });
    }
  }

  // ---- 兑换码 ----
  const redeem = useRedeem();
  const [code, setCode] = useState("");
  const [redeemMsg, setRedeemMsg] = useState<{ ok: boolean; text: string } | null>(null);

  async function submitRedeem(e: React.FormEvent) {
    e.preventDefault();
    setRedeemMsg(null);
    try {
      const r = await redeem.mutateAsync(code.trim());
      setRedeemMsg({ ok: true, text: `兑换成功，当前余额 ${num(r.balance)}` });
      setCode("");
    } catch (e) {
      setRedeemMsg({ ok: false, text: e instanceof ApiError ? e.message : "兑换失败" });
    }
  }

  return (
    <div className="grid max-w-3xl gap-6 md:grid-cols-2">
      {/* 在线支付 */}
      <Card>
        <CardHeader>
          <CardTitle>在线充值</CardTitle>
        </CardHeader>
        <CardContent>
          {!onlineEnabled ? (
            <p className="text-sm text-muted-foreground">
              在线支付未配置。请使用右侧兑换码，或联系管理员充值。
            </p>
          ) : (
            <div className="space-y-4">
              <div className="space-y-1.5">
                <Label>充值积分（{cpc} 积分 = ¥1）</Label>
                <div className="flex flex-wrap gap-2">
                  {PRESETS.map((p) => (
                    <button
                      key={p}
                      type="button"
                      onClick={() => setAmount(p)}
                      className={cn(
                        "rounded-lg border px-3 py-1.5 text-sm transition-colors",
                        amount === p ? "border-primary bg-primary/10 text-primary" : "border-border text-muted-foreground",
                      )}
                    >
                      {p} <span className="opacity-60">¥{(p / cpc).toFixed(2)}</span>
                    </button>
                  ))}
                </div>
                <Input
                  type="number"
                  min={1}
                  value={amount}
                  onChange={(e) => setAmount(Math.max(1, Number(e.target.value) || 0))}
                  className="mt-1"
                />
              </div>
              <div className="space-y-1.5">
                <Label>支付渠道</Label>
                <div className="flex gap-2">
                  {CHANNELS.map((c) => (
                    <button
                      key={c.key}
                      type="button"
                      onClick={() => setChannel(c.key)}
                      className={cn(
                        "flex-1 rounded-lg border py-1.5 text-sm transition-colors",
                        channel === c.key ? "border-primary bg-primary/10 text-primary" : "border-border text-muted-foreground",
                      )}
                    >
                      {c.label}
                    </button>
                  ))}
                </div>
              </div>
              {payMsg && (
                <div className={cn("rounded-lg px-3 py-2 text-[13px]", payMsg.ok ? "bg-green-100 text-green-700" : "bg-red-50 text-red-600")}>
                  {payMsg.text}
                </div>
              )}
              <Button onClick={pay} disabled={createPay.isPending || !!orderId} className="w-full">
                {orderId ? "等待支付到账…" : createPay.isPending ? "下单中…" : `去支付 ¥${(amount / cpc).toFixed(2)}`}
              </Button>
            </div>
          )}
        </CardContent>
      </Card>

      {/* 兑换码 */}
      <Card>
        <CardHeader>
          <CardTitle>兑换码充值</CardTitle>
        </CardHeader>
        <CardContent>
          <form onSubmit={submitRedeem} className="space-y-3">
            <div className="space-y-1.5">
              <Label>兑换码</Label>
              <Input value={code} onChange={(e) => setCode(e.target.value)} placeholder="SUB2ORG-XXXX-XXXX" className="font-mono" />
            </div>
            {redeemMsg && (
              <div className={cn("rounded-lg px-3 py-2 text-[13px]", redeemMsg.ok ? "bg-green-100 text-green-700" : "bg-red-50 text-red-600")}>
                {redeemMsg.text}
              </div>
            )}
            <Button type="submit" disabled={redeem.isPending || !code.trim()}>
              兑换
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
