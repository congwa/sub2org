/** unix 秒 → 相对时间 "2s ago" / "3m ago"。 */
export function relSecs(unixSecs?: number | null): string {
  if (!unixSecs) return "—";
  const d = Math.max(0, Math.floor(Date.now() / 1000 - unixSecs));
  if (d < 60) return `${d}s`;
  if (d < 3600) return `${Math.floor(d / 60)}m`;
  if (d < 86400) return `${Math.floor(d / 3600)}h`;
  return `${Math.floor(d / 86400)}d`;
}

/** unix 毫秒 → 相对时间。 */
export function relMs(ms?: number | null): string {
  return relSecs(ms ? Math.floor(ms / 1000) : null);
}

export function num(n?: number | null): string {
  return (n ?? 0).toLocaleString("en-US");
}

/** 中间截断 id：sess-9c…21 */
export function shortId(id?: string | null, head = 6, tail = 2): string {
  if (!id) return "—";
  if (id.length <= head + tail + 1) return id;
  return `${id.slice(0, head)}…${id.slice(-tail)}`;
}

export function ms(n?: number | null): string {
  if (n == null) return "—";
  return n < 1000 ? `${n}ms` : `${(n / 1000).toFixed(1)}s`;
}
