// 业务职责：用紧凑链路条说明 sub2org 的核心协议路径，帮助访问者快速理解请求如何穿过 relay。
import { ChevronRight } from "lucide-react";
import type { SiteDictionary } from "@/lib/content";

type ProtocolStripProps = {
  content: SiteDictionary;
};

const accents = [
  {
    before: "before:bg-signal",
    label: "text-signal-strong"
  },
  {
    before: "before:bg-coral",
    label: "text-coral"
  },
  {
    before: "before:bg-amber",
    label: "text-[#b9800e]"
  },
  {
    before: "before:bg-blueprint",
    label: "text-blueprint"
  }
];

/**
 * 业务职责：把消费者、服务端、worker 和 SSE 输出四个业务节点串联展示，降低架构理解门槛。
 */
export function ProtocolStrip({ content }: ProtocolStripProps) {
  return (
    <section
      className="relative mx-auto w-[calc(100%-1.5rem)] max-w-[1180px] border-t border-ink/[0.12] py-12 pt-14 sm:w-[calc(100%-2rem)] sm:py-16 sm:pt-[4.5rem] lg:py-20"
      aria-labelledby="protocol-title"
    >
      <div className="mb-7 grid gap-6 lg:grid-cols-[minmax(0,0.82fr)_minmax(280px,0.38fr)] lg:items-end">
        <div className="max-w-[860px]">
        <p className="mb-3.5 font-mono text-xs font-extrabold uppercase text-signal-strong">
          {content.protocol.eyebrow}
        </p>
        <h2
          className="mb-3.5 max-w-[860px] break-words font-display text-[2rem] font-[820] leading-[1.06] tracking-normal text-ink text-balance sm:text-[2.66rem] sm:leading-[1.04]"
          id="protocol-title"
        >
          {content.protocol.title}
        </h2>
        <p className="max-w-[760px] text-base leading-[1.72] text-muted sm:text-[1.02rem]">
          {content.protocol.intro}
        </p>
        </div>
        <div
          aria-hidden="true"
          className="hidden h-24 overflow-hidden rounded-lg border border-ink/[0.10] bg-white/55 p-3 shadow-[0_12px_36px_rgba(9,17,15,0.06)] lg:grid"
        >
          <div className="grid grid-cols-7 gap-1.5">
            {content.protocol.steps.map((step, index) => (
              <span
                className={`h-full min-h-14 rounded-[5px] ${index % 2 === 0 ? "bg-signal/20" : "bg-coral/20"}`}
                key={step.label}
              />
            ))}
            <span className="min-h-14 rounded-[5px] bg-amber/25" />
            <span className="min-h-14 rounded-[5px] bg-blueprint/20" />
            <span className="min-h-14 rounded-[5px] bg-ink/10" />
          </div>
        </div>
      </div>

      <div className="relative grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
        <span
          aria-hidden="true"
          className="absolute left-8 right-8 top-1/2 hidden h-px -translate-y-1/2 bg-gradient-to-r from-transparent via-signal-strong/45 to-transparent lg:block"
        />
        {content.protocol.steps.map((step, index) => {
          const accent = accents[index];

          return (
            <div
              className={`signal-card micro-lift relative min-h-36 overflow-hidden rounded-lg border border-ink/[0.12] bg-white/80 p-5 shadow-[0_14px_42px_rgba(9,17,15,0.06)] hover:-translate-y-1 hover:border-signal-strong/35 hover:shadow-[0_22px_58px_rgba(9,17,15,0.10)] before:absolute before:inset-y-0 before:left-0 before:w-1 ${accent.before}`}
              key={step.label}
            >
              <span
                className={`mb-5 block font-mono text-xs font-extrabold uppercase ${accent.label}`}
              >
                {step.label}
              </span>
              <strong className="block text-[1.08rem] leading-[1.35] text-ink">
                {step.value}
              </strong>
              {index < content.protocol.steps.length - 1 ? (
                <ChevronRight
                  aria-hidden="true"
                  className="absolute -right-[18px] top-1/2 z-10 hidden -translate-y-1/2 text-coral lg:block"
                  size={22}
                />
              ) : null}
            </div>
          );
        })}
      </div>
    </section>
  );
}
