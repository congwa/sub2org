import { useState } from "react";
import { Badge, Button, Card, CardHeader, CardTitle, Input } from "@/components/ui";
import { DataTable, type Column } from "@/components/common";
import { useCreateMyKey, useDeleteMyKey, useDisableMyKey, useEnableMyKey, useMyKeys } from "@/hooks";
import { relSecs } from "@/lib/format";
import type { ApiKeyRow } from "@/api/types";

export function ApiKeys() {
  const keys = useMyKeys();
  const create = useCreateMyKey();
  const disable = useDisableMyKey();
  const enable = useEnableMyKey();
  const del = useDeleteMyKey();
  const [name, setName] = useState("");
  const [created, setCreated] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);

  async function add(e: React.FormEvent) {
    e.preventDefault();
    const r = await create.mutateAsync(name || "key");
    setCreated(r.api_key);
    setName("");
  }

  // #5：消费端 base URL = 当前同源（经 nginx）+ /v1，避免用户误填内部 :3300。
  const baseUrl = `${window.location.origin}/v1`;
  async function copy(text: string, tag: string) {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(tag);
      setTimeout(() => setCopied((c) => (c === tag ? null : c)), 2000);
    } catch {
      /* 剪贴板不可用时静默 */
    }
  }

  const cols: Column<ApiKeyRow>[] = [
    { key: "name", header: "Name", render: (k) => k.name || "—" },
    { key: "id", header: "ID", render: (k) => <span className="font-mono text-[12px]">{k.id}</span> },
    { key: "status", header: "Status", render: (k) => <Badge tone={k.status === "active" ? "green" : "red"}>{k.status}</Badge> },
    { key: "created", header: "Created", render: (k) => <span className="font-mono text-[11.5px] text-muted-foreground">{relSecs(k.created_at)}</span> },
    {
      key: "act",
      header: "",
      className: "text-right",
      render: (k) => (
        <div className="flex justify-end gap-1.5">
          {k.status === "active" ? (
            <Button size="xs" variant="outline" onClick={() => disable.mutate(k.id)}>
              禁用
            </Button>
          ) : (
            <Button size="xs" variant="outline" onClick={() => enable.mutate(k.id)}>
              启用
            </Button>
          )}
          <Button size="xs" variant="destructive" onClick={() => del.mutate(k.id)}>
            删除
          </Button>
        </div>
      ),
    },
  ];

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader>
          <CardTitle>新建 API Key</CardTitle>
        </CardHeader>
        <div className="p-4">
          <form onSubmit={add} className="flex items-center gap-3">
            <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="名称（如 my-app）" className="max-w-xs" />
            <Button type="submit" disabled={create.isPending}>
              创建
            </Button>
          </form>
          {created && (
            <div className="mt-3 rounded-lg border border-amber-500/40 bg-[#fef3e2] p-3">
              <div className="mb-1 text-xs font-semibold text-amber-600">密钥仅显示这一次，请立即复制：</div>
              <code className="break-all font-mono text-[13px]">{created}</code>
            </div>
          )}
        </div>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>接入指南</CardTitle>
        </CardHeader>
        <div className="space-y-3 p-4">
          <div className="rounded-lg border border-border bg-muted/40 p-3">
            <div className="mb-2 text-xs font-semibold text-muted-foreground">
              Base URL（OpenAI 兼容；填这个，不要填内部 3300 端口）
            </div>
            <div className="flex items-center justify-between gap-2">
              <code className="flex-1 break-all font-mono text-[13px] text-foreground">{baseUrl}</code>
              <Button size="xs" variant="outline" onClick={() => copy(baseUrl, "base")}>
                {copied === "base" ? "已复制" : "复制"}
              </Button>
            </div>
          </div>
          <div className="rounded-lg border border-border bg-muted/40 p-3">
            <div className="mb-2 text-xs font-semibold text-muted-foreground">OpenAI SDK 示例</div>
            <pre className="overflow-x-auto whitespace-pre-wrap break-all font-mono text-[12px] text-foreground">{`const client = new OpenAI({
  baseURL: "${baseUrl}",
  apiKey: "${created ?? "sk-sub2org-..."}",
});`}</pre>
          </div>
          <div className="text-[11.5px] text-muted-foreground">
            Cherry Studio 等客户端：API 地址填上面的 Base URL，API Key 用你创建的{" "}
            <code className="font-mono">sk-sub2org-…</code>，模型名按运营方公布（如{" "}
            <code className="font-mono">gpt-4o</code> / <code className="font-mono">claude</code>）。
          </div>
        </div>
      </Card>
      <Card>
        <DataTable columns={cols} rows={keys.data ?? []} loading={keys.isLoading} empty="还没有 key" />
      </Card>
    </div>
  );
}
