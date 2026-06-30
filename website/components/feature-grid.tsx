// 业务职责：展示官网的核心产品能力，帮助潜在部署者判断 sub2org 是否覆盖 worker 调度、流式、鉴权和计费场景。
import {
  Cable,
  Gauge,
  RadioTower,
  ReceiptText,
  ShieldCheck,
  Workflow
} from "lucide-react";
import type { SiteDictionary } from "@/lib/content";

type FeatureGridProps = {
  content: SiteDictionary;
};

const featureIconMap = {
  Cable,
  Gauge,
  RadioTower,
  ReceiptText,
  ShieldCheck,
  Workflow
};

const baseCardClass =
  "signal-card micro-lift group relative min-h-[228px] overflow-hidden rounded-lg border border-ink/[0.12] bg-white/82 p-5 shadow-[0_14px_42px_rgba(9,17,15,0.055)] hover:-translate-y-1 hover:border-signal-strong/40 hover:bg-white hover:shadow-[0_22px_58px_rgba(9,17,15,0.11)] sm:min-h-[260px] sm:p-6";

/**
 * 业务职责：把 sub2org 从浏览器自动化到可运营 API 的关键能力拆成可扫描卡片，支撑官网转化判断。
 */
export function FeatureGrid({ content }: FeatureGridProps) {
  return (
    <section
      className="mx-auto w-[calc(100%-1.5rem)] max-w-[1180px] py-12 sm:w-[calc(100%-2rem)] sm:py-16 lg:py-20"
      aria-labelledby="feature-title"
    >
      <div className="mb-8 max-w-[860px]">
        <p className="mb-3.5 font-mono text-xs font-extrabold uppercase text-signal-strong">
          {content.features.eyebrow}
        </p>
        <h2
          className="mb-3.5 max-w-[860px] break-words font-display text-[2rem] font-[820] leading-[1.06] tracking-normal text-ink text-balance sm:text-[2.66rem] sm:leading-[1.04]"
          id="feature-title"
        >
          {content.features.title}
        </h2>
        <p className="max-w-[760px] text-base leading-[1.72] text-muted sm:text-[1.02rem]">
          {content.features.kicker}
        </p>
      </div>

      <div className="grid grid-cols-1 gap-3 md:grid-cols-2 lg:grid-cols-12">
        {content.features.items.map((feature, index) => {
          const Icon = featureIconMap[feature.icon];
          const isDark = index === 0;
          const isWide = index === 0 || index === content.features.items.length - 1;
          const cardClass = [
            baseCardClass,
            isWide ? "lg:col-span-6" : "lg:col-span-4",
            isDark
              ? "signal-card-dark bg-[linear-gradient(135deg,rgba(9,17,15,0.96),rgba(17,16,13,0.94))] text-[#f4fff8] shadow-strong hover:bg-[linear-gradient(135deg,rgba(9,17,15,0.98),rgba(17,16,13,0.96))]"
              : "",
            index === content.features.items.length - 1
              ? "bg-[linear-gradient(135deg,rgba(87,224,177,0.2),rgba(255,116,72,0.13))]"
              : ""
          ]
            .filter(Boolean)
            .join(" ");

          return (
            <article
              className={cardClass}
              key={feature.title}
              style={
                isDark
                  ? {
                      background:
                        "linear-gradient(135deg, rgba(9,17,15,0.98), rgba(17,16,13,0.96))"
                    }
                  : undefined
              }
            >
              <div className="relative z-10">
                <div className="mb-7 flex items-start justify-between gap-4">
                  <div
                    className={
                      isDark
                        ? "grid h-11 w-11 place-items-center rounded-lg border border-white/[0.15] bg-signal/[0.15] text-signal"
                        : "grid h-11 w-11 place-items-center rounded-lg border border-ink/10 bg-paper-wash text-signal-strong"
                    }
                  >
                    <Icon aria-hidden="true" size={22} strokeWidth={2} />
                  </div>
                  <span
                    aria-hidden="true"
                    className={
                      isDark
                        ? "h-2 w-16 rounded-full bg-white/15"
                        : "h-2 w-16 rounded-full bg-ink/10"
                    }
                  />
                </div>
                <h3 className="mb-2.5 font-display text-lg font-[830] leading-[1.24]">
                  {feature.title}
                </h3>
                <p
                  className={
                    isDark
                      ? "text-base leading-[1.72] text-white/70"
                      : "text-base leading-[1.72] text-muted"
                  }
                >
                  {feature.body}
                </p>
              </div>
            </article>
          );
        })}
      </div>
    </section>
  );
}
