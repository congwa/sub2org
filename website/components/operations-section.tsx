// 业务职责：展示 sub2org 的生产运维、可观测和存储策略，回应部署者对可靠性与维护成本的核心顾虑。
import { Activity, DatabaseZap, ListChecks } from "lucide-react";
import type { SiteDictionary } from "@/lib/content";

type OperationsSectionProps = {
  content: SiteDictionary;
};

const panelIcons = [Activity, DatabaseZap, ListChecks];

/**
 * 业务职责：把故障处理、持久化和可观测策略放在同一段落，突出单实例架构下的运维可控性。
 */
export function OperationsSection({ content }: OperationsSectionProps) {
  return (
    <section
      className="ops-band mt-8 bg-night py-14 text-[#f4fff8] sm:mt-12 sm:py-[4.5rem] lg:py-24"
      id="ops"
      aria-labelledby="ops-title"
    >
      <div className="relative z-10 mx-auto w-[calc(100%-1.5rem)] max-w-[1180px] sm:w-[calc(100%-2rem)]">
        <div className="mb-8 max-w-[860px]">
          <p className="mb-3.5 font-mono text-xs font-extrabold uppercase text-signal">
            {content.operations.eyebrow}
          </p>
          <h2
            className="mb-3.5 max-w-[860px] break-words font-display text-[2rem] font-[830] leading-[1.06] tracking-normal text-[#f4fff8] text-balance sm:text-[2.66rem] sm:leading-[1.04]"
            id="ops-title"
          >
            {content.operations.title}
          </h2>
          <p className="max-w-[760px] text-base leading-[1.72] text-white/70 sm:text-[1.02rem]">
            {content.operations.intro}
          </p>
        </div>

        <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
          {content.operations.panels.map((panel, index) => {
            const Icon = panelIcons[index];

            return (
              <article
                className="micro-lift rounded-lg border border-white/[0.14] bg-white/[0.07] p-5 shadow-[inset_0_1px_0_rgba(255,255,255,0.08)] backdrop-blur-sm hover:-translate-y-1 hover:border-signal/35 hover:bg-white/[0.10] sm:p-6"
                key={panel.title}
              >
                <Icon
                  aria-hidden="true"
                  className="mb-8 text-coral"
                  size={21}
                  strokeWidth={2}
                />
                <h3 className="mb-2.5 font-display text-lg font-[820] leading-[1.24] text-[#f4fff8]">
                  {panel.title}
                </h3>
                <p className="text-base leading-[1.72] text-white/68">
                  {panel.body}
                </p>
              </article>
            );
          })}
        </div>
      </div>
    </section>
  );
}
