import { API_BASE } from "@/config";
import type { LiveEvent } from "./types";

export type SseState = "connecting" | "open" | "closed" | "unsupported";

/** fetch-stream 方式订阅 /admin/events（可带 Authorization，自动重连）。返回停止函数。 */
export function startAdminEvents(opts: {
  token: () => string | null;
  onEvent: (e: LiveEvent) => void;
  onState: (s: SseState) => void;
}): () => void {
  const ctrl = new AbortController();
  let stopped = false;
  let attempt = 0;

  (async function loop() {
    while (!stopped) {
      opts.onState("connecting");
      try {
        const t = opts.token();
        const res = await fetch(`${API_BASE}/admin/events`, {
          headers: { Accept: "text/event-stream", ...(t ? { Authorization: `Bearer ${t}` } : {}) },
          signal: ctrl.signal,
        });
        if (res.status === 404 || res.status === 501) {
          opts.onState("unsupported");
          return;
        }
        if (res.status === 401 || res.status === 403) {
          opts.onState("closed");
          return;
        }
        if (!res.ok || !res.body) throw new Error("sse failed");
        attempt = 0;
        opts.onState("open");
        const reader = res.body.pipeThrough(new TextDecoderStream()).getReader();
        let buf = "";
        for (;;) {
          const { value, done } = await reader.read();
          if (done) break;
          buf += value;
          let i: number;
          while ((i = buf.indexOf("\n\n")) !== -1) {
            const frame = buf.slice(0, i);
            buf = buf.slice(i + 2);
            let data = "";
            for (const line of frame.split("\n")) if (line.startsWith("data:")) data += line.slice(5).trim();
            if (!data || data === "[DONE]") continue;
            try {
              opts.onEvent(JSON.parse(data) as LiveEvent);
            } catch {
              /* skip */
            }
          }
        }
      } catch {
        if (ctrl.signal.aborted) return;
      }
      if (stopped) return;
      opts.onState("closed");
      await new Promise((r) => setTimeout(r, Math.min(1000 * 2 ** attempt++, 15000)));
    }
  })();

  return () => {
    stopped = true;
    ctrl.abort();
  };
}
