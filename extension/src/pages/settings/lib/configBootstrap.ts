// 规模化预配的纯函数：URL 参数解析 + JSON 配置导入导出。无 React、无 chrome API。
// 可共享的「引导字段」——刻意不含 workerId/capacity（每台机器保留唯一身份、并发固定 1）。

export type ProviderValue = "chatgpt" | "claude";

export type BootstrapConfig = {
  serverBaseUrl: string;
  workerToken: string;
  provider: ProviderValue;
  accountLabel: string;
};

/** 可被 URL / 导入文件 / 导出携带的字段（单一真相源）。 */
export const BOOTSTRAP_FIELDS = [
  "serverBaseUrl",
  "workerToken",
  "provider",
  "accountLabel",
] as const;

const CONFIG_SCHEMA = "sub2org-worker-config@1";

const isProvider = (v: unknown): v is ProviderValue => v === "chatgpt" || v === "claude";

/**
 * 从 settings 页 URL query 读引导配置。
 * 接受 `apiBaseUrl` 作为 `serverBaseUrl` 的别名；`autosave=1` → 自动保存+连接。
 */
export function parseUrlConfig(search: string): {
  patch: Partial<BootstrapConfig>;
  autosave: boolean;
} {
  const q = new URLSearchParams(search || "");
  const patch: Partial<BootstrapConfig> = {};

  const server = q.get("serverBaseUrl") ?? q.get("apiBaseUrl");
  if (server && server.trim()) patch.serverBaseUrl = server.trim();

  const token = q.get("workerToken");
  if (token && token.trim()) patch.workerToken = token.trim();

  const provider = q.get("provider");
  if (provider && isProvider(provider)) patch.provider = provider;

  const label = q.get("accountLabel");
  if (label && label.trim()) patch.accountLabel = label.trim();

  const autosave = q.get("autosave") === "1" || q.get("autosave") === "true";
  return { patch, autosave };
}

/** 序列化为可下载的配置 JSON（仅引导字段 + 版本标记）。 */
export function serializeConfig(cfg: Partial<BootstrapConfig>): string {
  const out: Record<string, unknown> = { _schema: CONFIG_SCHEMA };
  for (const k of BOOTSTRAP_FIELDS) {
    if (cfg[k] !== undefined && cfg[k] !== "") out[k] = cfg[k];
  }
  return JSON.stringify(out, null, 2);
}

/** 解析导入的配置文件，白名单到引导字段；校验失败抛错。 */
export function parseConfigFile(text: string): Partial<BootstrapConfig> {
  let obj: unknown;
  try {
    obj = JSON.parse(text);
  } catch {
    throw new Error("不是合法的 JSON 文件");
  }
  if (typeof obj !== "object" || obj === null) throw new Error("配置内容格式错误");
  const rec = obj as Record<string, unknown>;
  if (rec._schema !== CONFIG_SCHEMA) {
    throw new Error(`配置版本不匹配（需要 ${CONFIG_SCHEMA}）`);
  }
  const patch: Partial<BootstrapConfig> = {};
  if (typeof rec.serverBaseUrl === "string") patch.serverBaseUrl = rec.serverBaseUrl;
  if (typeof rec.workerToken === "string") patch.workerToken = rec.workerToken;
  if (isProvider(rec.provider)) patch.provider = rec.provider;
  if (typeof rec.accountLabel === "string") patch.accountLabel = rec.accountLabel;
  return patch;
}
