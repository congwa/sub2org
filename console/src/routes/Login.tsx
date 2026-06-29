import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { decodeJwt, useAuth } from "@/lib/auth";
import { useForgot, useLogin, useLogin2fa, usePricing, useRegister } from "@/hooks";
import { Logo, Wordmark } from "@/components/Logo";
import { Button, Card, CardContent, Input, Label } from "@/components/ui";
import { ApiError } from "@/api/errors";
import { cn } from "@/lib/cn";

type Mode = "login" | "register" | "forgot" | "2fa";

export function Login() {
  const { login } = useAuth();
  const nav = useNavigate();
  const loginM = useLogin();
  const login2faM = useLogin2fa();
  const registerM = useRegister();
  const forgotM = useForgot();
  const [mode, setMode] = useState<Mode>("login");
  const [u, setU] = useState("");
  const [p, setP] = useState("");
  const [email, setEmail] = useState("");
  const [challenge, setChallenge] = useState("");
  const [code, setCode] = useState("");
  const [err, setErr] = useState("");
  const [info, setInfo] = useState("");
  const busy = loginM.isPending || login2faM.isPending || registerM.isPending || forgotM.isPending;

  // B5：Turnstile 验证码（仅注册、且服务端配置了 site key 时渲染）。
  const pricing = usePricing();
  const siteKey = pricing.data?.turnstile_site_key || "";
  const [tsToken, setTsToken] = useState("");
  const tsRef = useRef<HTMLDivElement>(null);
  const tsWidget = useRef<string | null>(null);
  useEffect(() => {
    if (mode !== "register" || !siteKey) {
      tsWidget.current = null;
      setTsToken("");
      return;
    }
    const SRC = "https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit";
    const render = () => {
      const w = (window as any).turnstile;
      if (!w || !tsRef.current || tsWidget.current) return;
      tsWidget.current = w.render(tsRef.current, {
        sitekey: siteKey,
        callback: (t: string) => setTsToken(t),
        "error-callback": () => setTsToken(""),
        "expired-callback": () => setTsToken(""),
      });
    };
    if ((window as any).turnstile) {
      render();
      return;
    }
    if (!document.querySelector(`script[src="${SRC}"]`)) {
      const s = document.createElement("script");
      s.src = SRC;
      s.async = true;
      s.defer = true;
      s.onload = render;
      document.head.appendChild(s);
    }
    const iv = setInterval(() => {
      if ((window as any).turnstile) {
        clearInterval(iv);
        render();
      }
    }, 200);
    return () => clearInterval(iv);
  }, [mode, siteKey]);

  function goByRole(jwt: string) {
    login(jwt);
    const c = decodeJwt(jwt);
    nav(c?.role === "admin" ? "/overview" : "/dashboard", { replace: true });
  }

  function switchMode(m: Mode) {
    setMode(m);
    setErr("");
    setInfo("");
  }

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setErr("");
    setInfo("");
    try {
      if (mode === "forgot") {
        await forgotM.mutateAsync(email.trim());
        setInfo("若该邮箱已注册，重置链接已发送，请查收邮件。");
        return;
      }
      if (mode === "2fa") {
        const { jwt } = await login2faM.mutateAsync({ challenge_token: challenge, code: code.trim() });
        goByRole(jwt);
        return;
      }
      if (mode === "register") {
        const r = await registerM.mutateAsync({ username: u, password: p, email: email.trim() || undefined, turnstile_token: tsToken || undefined });
        if (r.jwt) goByRole(r.jwt);
        else {
          const lr = await loginM.mutateAsync({ username: u, password: p });
          if ("jwt" in lr) goByRole(lr.jwt);
        }
      } else {
        const lr = await loginM.mutateAsync({ username: u, password: p });
        if ("need_2fa" in lr) {
          setChallenge(lr.challenge_token);
          setCode("");
          switchMode("2fa");
        } else {
          goByRole(lr.jwt);
        }
      }
    } catch (e) {
      const fallback =
        mode === "register" ? "注册失败" : mode === "forgot" ? "发送失败" : mode === "2fa" ? "验证码错误" : "登录失败";
      setErr(e instanceof ApiError ? e.message : fallback);
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
              <div className="text-xs text-muted-foreground">中转控制中心</div>
            </div>
          </div>

          {(mode === "login" || mode === "register") && (
            <div className="mb-5 flex rounded-lg bg-muted p-1 text-sm">
              {(["login", "register"] as Mode[]).map((m) => (
                <button
                  key={m}
                  onClick={() => switchMode(m)}
                  className={cn(
                    "flex-1 rounded-md py-1.5 font-medium transition-colors",
                    mode === m ? "bg-card text-foreground shadow-sm" : "text-muted-foreground",
                  )}
                >
                  {m === "login" ? "登录" : "注册"}
                </button>
              ))}
            </div>
          )}

          <form onSubmit={submit} className="space-y-4">
            {mode === "2fa" && (
              <div className="space-y-1.5">
                <Label>两步验证码</Label>
                <Input
                  value={code}
                  onChange={(e) => setCode(e.target.value)}
                  autoFocus
                  inputMode="numeric"
                  placeholder="6 位动态码"
                />
                <p className="text-xs text-muted-foreground">请输入身份验证器 App 上的 6 位动态码。</p>
              </div>
            )}
            {(mode === "login" || mode === "register") && (
              <>
                <div className="space-y-1.5">
                  <Label>用户名</Label>
                  <Input value={u} onChange={(e) => setU(e.target.value)} autoFocus placeholder="username" />
                </div>
                <div className="space-y-1.5">
                  <Label>密码</Label>
                  <Input
                    type="password"
                    value={p}
                    onChange={(e) => setP(e.target.value)}
                    placeholder={mode === "register" ? "至少 6 位" : "••••••••"}
                  />
                </div>
              </>
            )}
            {(mode === "register" || mode === "forgot") && (
              <div className="space-y-1.5">
                <Label>邮箱{mode === "register" ? "（选填，用于找回密码）" : ""}</Label>
                <Input
                  type="email"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  autoFocus={mode === "forgot"}
                  placeholder="you@example.com"
                />
              </div>
            )}
            {mode === "register" && siteKey && <div ref={tsRef} className="flex justify-center" />}
            {err && <div className="rounded-lg bg-red-50 px-3 py-2 text-[13px] text-red-600">{err}</div>}
            {info && <div className="rounded-lg bg-green-100 px-3 py-2 text-[13px] text-green-700">{info}</div>}
            <Button type="submit" className="w-full" disabled={busy}>
              {busy
                ? "处理中…"
                : mode === "register"
                  ? "注册并登录"
                  : mode === "forgot"
                    ? "发送重置链接"
                    : mode === "2fa"
                      ? "验证"
                      : "登录"}
            </Button>
          </form>

          <div className="mt-4 text-center text-xs text-muted-foreground">
            {mode === "login" && (
              <button onClick={() => switchMode("forgot")} className="hover:text-foreground">
                忘记密码？
              </button>
            )}
            {(mode === "forgot" || mode === "2fa") && (
              <button onClick={() => switchMode("login")} className="hover:text-foreground">
                返回登录
              </button>
            )}
            {mode === "register" && <span>注册即创建普通用户账号</span>}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
