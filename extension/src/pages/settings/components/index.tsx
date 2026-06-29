import { useEffect, useRef, useState } from "react";
import { Server, Plug, Save } from "lucide-react";
import type { Provider, WorkerConfig } from "@src/pages/background";
import {
  parseUrlConfig,
  type BootstrapConfig,
} from "@src/pages/settings/lib/configBootstrap";
import { ConfigIO } from "./ConfigIO";

type Status = "pending" | "connected" | "failed" | "disconnected";

export const SettingPage = () => {
  const [cfg, setCfg] = useState<WorkerConfig>({
    serverBaseUrl: "",
    workerToken: "",
    workerId: "",
    accountLabel: "",
    provider: "chatgpt",
    capacity: 1,
  });
  const [status, setStatus] = useState<Status>("disconnected");
  const [error, setError] = useState<string>("");
  const [saved, setSaved] = useState(false);
  // URL 预填的字段：随后到达的持久化配置不得覆盖它们。
  const urlKeysRef = useRef<string[]>([]);

  // 保存一份引导配置（持久化 + 重连），不依赖 state 落定（用于 URL autosave / 导入）。
  const applyBootstrap = (patch: Partial<BootstrapConfig>) => {
    setCfg((prev) => ({ ...prev, ...patch }));
    const content: Record<string, unknown> = {};
    if (patch.serverBaseUrl !== undefined) content.serverBaseUrl = patch.serverBaseUrl;
    if (patch.workerToken !== undefined) content.workerToken = patch.workerToken;
    if (patch.accountLabel !== undefined) content.accountLabel = patch.accountLabel;
    if (patch.provider !== undefined) content.provider = patch.provider;
    chrome.runtime.sendMessage({ type: "set_worker_config", content });
    setSaved(true);
    setTimeout(() => setSaved(false), 1500);
  };

  useEffect(() => {
    // 先吃 URL 参数（脚本/RPA 批量预配：?serverBaseUrl=…&workerToken=…&autosave=1）。
    const { patch, autosave } = parseUrlConfig(window.location.search);
    const urlKeys = Object.keys(patch);
    urlKeysRef.current = urlKeys;
    if (urlKeys.length) setCfg((prev) => ({ ...prev, ...patch }));

    chrome.runtime.sendMessage({ type: "get_worker_config" });
    chrome.runtime.sendMessage({ type: "get_connection_status" }, (resp) => {
      if (resp?.content) {
        setStatus(resp.content.status);
        setError(resp.content.errorMessage || "");
      }
    });
    const listener = (msg: any) => {
      if (msg.type === "set_worker_config" && msg.content) {
        // 合并持久化配置，但 URL 提供的字段保持优先。
        setCfg((prev) => {
          const merged = { ...(msg.content as WorkerConfig) };
          for (const k of urlKeysRef.current) {
            (merged as any)[k] = (prev as any)[k];
          }
          return merged;
        });
      } else if (msg.type === "get_connection_status" && msg.content) {
        setStatus(msg.content.status);
        setError(msg.content.errorMessage || "");
      }
    };
    chrome.runtime.onMessage.addListener(listener);

    if (autosave && urlKeys.length) applyBootstrap(patch);

    return () => chrome.runtime.onMessage.removeListener(listener);
  }, []);

  const save = () => {
    chrome.runtime.sendMessage({
      type: "set_worker_config",
      content: {
        serverBaseUrl: cfg.serverBaseUrl,
        workerToken: cfg.workerToken,
        accountLabel: cfg.accountLabel,
        provider: cfg.provider,
        capacity: Number(cfg.capacity) || 1,
      },
    });
    setSaved(true);
    setTimeout(() => setSaved(false), 1500);
  };

  const statusColor =
    status === "connected"
      ? "text-green-600"
      : status === "pending"
      ? "text-amber-600"
      : "text-red-600";

  const field = (label: string, node: React.ReactNode, hint?: string) => (
    <div className="mb-4">
      <label className="block text-sm font-medium text-gray-700 mb-1">{label}</label>
      {node}
      {hint && <p className="text-xs text-gray-400 mt-1">{hint}</p>}
    </div>
  );

  const input = (
    value: string | number,
    onChange: (v: string) => void,
    placeholder = ""
  ) => (
    <input
      value={value}
      placeholder={placeholder}
      onChange={(e) => onChange(e.target.value)}
      className="w-full bg-gray-50 border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
    />
  );

  return (
    <div className="max-w-xl mx-auto p-6">
      <div className="flex items-center gap-2 mb-1">
        <Server size={22} className="text-blue-600" />
        <h1 className="text-xl font-semibold text-gray-900">sub2org-worker</h1>
      </div>
      <p className="text-sm text-gray-500 mb-6">
        把这台浏览器变成 sub2org 的一个 worker。配置好后点连接。
      </p>

      <div className="bg-white border border-gray-200 rounded-xl p-5 shadow-sm">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <Plug size={16} className={statusColor} />
            <span className={`text-sm font-medium ${statusColor}`}>
              {status}
              {error ? ` — ${error}` : ""}
            </span>
          </div>
          <div className="flex gap-2">
            <button
              onClick={() => chrome.runtime.sendMessage({ type: "connect" })}
              className="bg-blue-600 hover:bg-blue-700 text-white text-sm px-3 py-1.5 rounded-lg"
            >
              连接
            </button>
            <button
              onClick={() => chrome.runtime.sendMessage({ type: "disconnect" })}
              className="bg-gray-200 hover:bg-gray-300 text-gray-800 text-sm px-3 py-1.5 rounded-lg"
            >
              断开
            </button>
          </div>
        </div>

        {field(
          "sub2org 服务地址",
          input(cfg.serverBaseUrl, (v) => setCfg({ ...cfg, serverBaseUrl: v }), "http://localhost:3300"),
          "Rust 中转控制中心的 base URL"
        )}
        {field(
          "Worker Token",
          input(cfg.workerToken, (v) => setCfg({ ...cfg, workerToken: v }), "dev-worker-token"),
          "准入令牌，需与 sub2org 的 worker_tokens 匹配"
        )}
        {field(
          "账号标签 (account label)",
          input(cfg.accountLabel, (v) => setCfg({ ...cfg, accountLabel: v }), "如 chatgpt-plus-01"),
          "便于在控制台区分多个账号"
        )}
        {field(
          "Provider",
          <select
            value={cfg.provider}
            onChange={(e) => setCfg({ ...cfg, provider: e.target.value as Provider })}
            className="w-full bg-gray-50 border border-gray-300 rounded-lg px-3 py-2 text-sm"
          >
            <option value="chatgpt">ChatGPT</option>
            <option value="claude">Claude</option>
          </select>
        )}
        <div className="text-xs text-gray-400 mb-4">
          并发槽位固定为 1（一台浏览器一次处理一个对话，靠加机器扩展）<br />
          worker_id: <span className="font-mono">{cfg.workerId || "(生成中)"}</span>
        </div>

        <button
          onClick={save}
          className="w-full flex items-center justify-center gap-2 bg-green-600 hover:bg-green-700 text-white text-sm font-medium py-2.5 rounded-lg"
        >
          <Save size={16} />
          {saved ? "已保存并重连" : "保存并重连"}
        </button>

        <ConfigIO
          current={{
            serverBaseUrl: cfg.serverBaseUrl,
            workerToken: cfg.workerToken,
            provider: cfg.provider,
            accountLabel: cfg.accountLabel,
          }}
          onImport={(patch) => applyBootstrap(patch)}
        />
      </div>

      <p className="text-xs text-gray-400 mt-4">
        提示：请确保本浏览器已登录对应的 ChatGPT / Claude 账号。
      </p>
    </div>
  );
};
