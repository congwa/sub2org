import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Badge, Button, Card, CardContent, CardHeader, CardTitle, Input, Label } from "@/components/ui";
import { useMe, useTotpSetup, useTotpEnable, useTotpDisable, useLogoutAll } from "@/hooks";
import { useAuth } from "@/lib/auth";
import { ApiError } from "@/api/errors";

export function Security() {
  const qc = useQueryClient();
  const { logout } = useAuth();
  const me = useMe();
  const enabled = !!me.data?.user.totp_enabled;

  const setupM = useTotpSetup();
  const enableM = useTotpEnable();
  const disableM = useTotpDisable();
  const logoutAllM = useLogoutAll();

  const [setup, setSetup] = useState<{ secret: string; otpauth_uri: string } | null>(null);
  const [code, setCode] = useState("");
  const [disCode, setDisCode] = useState("");
  const [err, setErr] = useState("");
  const [info, setInfo] = useState("");

  const refreshMe = () => qc.invalidateQueries({ queryKey: ["me"] });

  async function startSetup() {
    setErr("");
    setInfo("");
    try {
      setSetup(await setupM.mutateAsync());
    } catch (e) {
      setErr(e instanceof ApiError ? e.message : "无法开始设置");
    }
  }

  async function confirmEnable() {
    setErr("");
    try {
      await enableM.mutateAsync(code.trim());
      setSetup(null);
      setCode("");
      setInfo("两步验证已启用。下次登录需输入动态码。");
      refreshMe();
    } catch (e) {
      setErr(e instanceof ApiError ? e.message : "验证码错误");
    }
  }

  async function confirmDisable() {
    setErr("");
    try {
      await disableM.mutateAsync({ code: disCode.trim() || undefined });
      setDisCode("");
      setInfo("两步验证已关闭。");
      refreshMe();
    } catch (e) {
      setErr(e instanceof ApiError ? e.message : "验证码错误");
    }
  }

  async function signOutEverywhere() {
    try {
      await logoutAllM.mutateAsync();
    } finally {
      logout();
    }
  }

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader>
          <CardTitle>
            两步验证 (2FA){" "}
            {enabled ? <Badge tone="green">已启用</Badge> : <Badge tone="neutral">未启用</Badge>}
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          {err && <div className="rounded-lg bg-red-50 px-3 py-2 text-[13px] text-red-600">{err}</div>}
          {info && <div className="rounded-lg bg-green-100 px-3 py-2 text-[13px] text-green-700">{info}</div>}

          {!enabled && !setup && (
            <Button onClick={startSetup} disabled={setupM.isPending}>
              启用两步验证
            </Button>
          )}

          {!enabled && setup && (
            <div className="space-y-3">
              <p className="text-sm text-muted-foreground">
                在身份验证器 App（Google Authenticator / 1Password 等）中添加，可扫描下方链接生成的二维码，或手动输入密钥：
              </p>
              <div className="space-y-1.5">
                <Label>密钥 (Base32)</Label>
                <Input readOnly value={setup.secret} className="font-mono" onFocus={(e) => e.currentTarget.select()} />
              </div>
              <div className="space-y-1.5">
                <Label>otpauth URI</Label>
                <Input readOnly value={setup.otpauth_uri} className="font-mono text-[11px]" onFocus={(e) => e.currentTarget.select()} />
              </div>
              <div className="space-y-1.5">
                <Label>输入 App 上的 6 位动态码以确认</Label>
                <Input value={code} onChange={(e) => setCode(e.target.value)} inputMode="numeric" placeholder="6 位动态码" className="w-40" />
              </div>
              <div className="flex gap-2">
                <Button onClick={confirmEnable} disabled={enableM.isPending}>
                  确认启用
                </Button>
                <Button variant="outline" onClick={() => setSetup(null)}>
                  取消
                </Button>
              </div>
            </div>
          )}

          {enabled && (
            <div className="space-y-2">
              <p className="text-sm text-muted-foreground">输入当前动态码以关闭两步验证：</p>
              <div className="flex gap-2">
                <Input value={disCode} onChange={(e) => setDisCode(e.target.value)} inputMode="numeric" placeholder="6 位动态码" className="w-40" />
                <Button variant="destructive" onClick={confirmDisable} disabled={disableM.isPending}>
                  关闭
                </Button>
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>会话</CardTitle>
        </CardHeader>
        <CardContent className="space-y-2">
          <p className="text-sm text-muted-foreground">在所有设备登出：吊销你已签发的全部令牌（本机也需重新登录）。</p>
          <Button variant="outline" onClick={signOutEverywhere} disabled={logoutAllM.isPending}>
            在所有设备登出
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}
