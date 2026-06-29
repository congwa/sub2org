import { useRef, useState } from "react";
import { Download, Upload } from "lucide-react";
import {
  serializeConfig,
  parseConfigFile,
  type BootstrapConfig,
} from "@src/pages/settings/lib/configBootstrap";

type Props = {
  current: Partial<BootstrapConfig>;
  onImport: (patch: Partial<BootstrapConfig>) => void;
};

/** 配置导入/导出：批量部署 worker 时一次导出、其余机器导入。 */
export const ConfigIO = ({ current, onImport }: Props) => {
  const fileRef = useRef<HTMLInputElement>(null);
  const [err, setErr] = useState("");

  const handleExport = () => {
    const blob = new Blob([serializeConfig(current)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "sub2org-worker.json";
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleFile = (e: React.ChangeEvent<HTMLInputElement>) => {
    setErr("");
    const file = e.target.files?.[0];
    e.target.value = ""; // 允许重复选同一文件
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      try {
        onImport(parseConfigFile(String(reader.result || "")));
      } catch (ex) {
        setErr(ex instanceof Error ? ex.message : "导入失败");
      }
    };
    reader.readAsText(file);
  };

  return (
    <div className="mt-3">
      <div className="flex gap-2">
        <button
          type="button"
          onClick={handleExport}
          className="flex-1 flex items-center justify-center gap-1.5 bg-gray-100 hover:bg-gray-200 text-gray-700 text-sm py-2 rounded-lg"
        >
          <Download size={15} />
          导出配置
        </button>
        <button
          type="button"
          onClick={() => fileRef.current?.click()}
          className="flex-1 flex items-center justify-center gap-1.5 bg-gray-100 hover:bg-gray-200 text-gray-700 text-sm py-2 rounded-lg"
        >
          <Upload size={15} />
          导入配置
        </button>
        <input
          ref={fileRef}
          type="file"
          accept="application/json"
          onChange={handleFile}
          className="hidden"
        />
      </div>
      <p className="text-xs text-gray-400 mt-1">
        导出不含 worker_id（每台机器各自唯一）；导入后自动保存并重连。
      </p>
      {err && <p className="text-xs text-red-600 mt-1">{err}</p>}
    </div>
  );
};
