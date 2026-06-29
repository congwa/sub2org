import { cn } from "@/lib/cn";

type Variant = "light" | "onDark" | "mono";
const P: Record<Variant, { ink: string; amber: string; gold: string; node: string; plate: boolean }> = {
  light: { ink: "#13161d", amber: "#f5a73e", gold: "#f9be41", node: "#f5a73e", plate: true },
  onDark: { ink: "#181c25", amber: "#f5a73e", gold: "#f9be41", node: "#f5a73e", plate: true },
  mono: { ink: "#13161d", amber: "#cdd3dd", gold: "#cdd3dd", node: "#cdd3dd", plate: false },
};

/** sub2org 标识：3 节点 → 网关竖杆 → 出射光束（移植自设计稿 markSVG）。 */
export function Logo({ variant = "light", size = 32 }: { variant?: Variant; size?: number }) {
  const { ink, amber, gold, node, plate } = P[variant];
  return (
    <svg width={size} height={size} viewBox="0 0 48 48" role="img" aria-label="sub2org">
      {plate && <rect x="2" y="2" width="44" height="44" rx="12" fill={ink} />}
      <g fill="none" stroke={amber} strokeWidth="1.7" strokeLinecap="round">
        <path d="M15 15 L28.5 23" />
        <path d="M15 24 L28.5 24" />
        <path d="M15 33 L28.5 25" />
      </g>
      <circle cx="13.4" cy="15" r="2.5" fill={node} />
      <circle cx="13.4" cy="24" r="2.5" fill={node} />
      <circle cx="13.4" cy="33" r="2.5" fill={node} />
      <rect x="28.5" y="13.5" width="3.4" height="21" rx="1.7" fill={amber} />
      <path d="M33.5 24 H41.5" stroke={gold} strokeWidth="2.6" strokeLinecap="round" />
      <circle cx="43.4" cy="24" r="1.7" fill={gold} />
    </svg>
  );
}

export function Wordmark({ onDark, className }: { onDark?: boolean; className?: string }) {
  return (
    <span className={cn("text-[19px] font-bold tracking-tight", onDark ? "text-white" : "text-ink-900", className)}>
      sub<span className="text-brand-amber">2</span>org
    </span>
  );
}
