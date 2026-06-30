// 业务职责：说明一次用户请求在 sub2org 内部的生命周期，连接 README 架构与官网叙事。
import type { SiteDictionary } from "@/lib/content";

type WorkflowSectionProps = {
  content: SiteDictionary;
};

/**
 * 业务职责：按请求生命周期展示 consumer、scheduler、browser execution 和 OpenAI-style response 的责任边界。
 */
export function WorkflowSection({ content }: WorkflowSectionProps) {
  return (
    <section
      className="mx-auto w-[calc(100%-1.5rem)] max-w-[1180px] py-12 sm:w-[calc(100%-2rem)] sm:py-16 lg:py-20"
      id="flow"
      aria-labelledby="workflow-title"
    >
      <div className="mb-8 max-w-[860px]">
        <p className="mb-3.5 font-mono text-xs font-extrabold uppercase text-signal-strong">
          {content.workflow.eyebrow}
        </p>
        <h2
          className="mb-3.5 max-w-[860px] break-words font-display text-[2rem] font-[820] leading-[1.06] tracking-normal text-ink text-balance sm:text-[2.66rem] sm:leading-[1.04]"
          id="workflow-title"
        >
          {content.workflow.title}
        </h2>
        <p className="max-w-[760px] text-base leading-[1.72] text-muted sm:text-[1.02rem]">
          {content.workflow.intro}
        </p>
      </div>

      <div className="relative grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <span
          aria-hidden="true"
          className="absolute left-6 right-6 top-6 hidden h-px bg-gradient-to-r from-transparent via-signal-strong/45 to-transparent lg:block"
        />
        {content.workflow.steps.map((step) => (
          <article
            className="micro-lift relative overflow-hidden rounded-lg border border-ink/[0.12] bg-white/80 p-5 shadow-[0_14px_42px_rgba(9,17,15,0.055)] hover:-translate-y-1 hover:border-coral/35 hover:bg-white hover:shadow-[0_22px_58px_rgba(9,17,15,0.10)] sm:p-6"
            key={step.number}
          >
            <span
              aria-hidden="true"
              className="absolute inset-x-0 top-0 h-1 bg-gradient-to-r from-signal via-amber to-coral"
            />
            <div className="relative z-10">
              <span className="mb-5 grid h-12 w-12 place-items-center rounded-lg bg-night font-mono text-sm font-extrabold text-signal shadow-[8px_8px_0_rgba(87,224,177,0.16)]">
                {step.number}
              </span>
              <h3 className="mb-2.5 font-display text-lg font-[820] leading-[1.24] text-ink">
                {step.title}
              </h3>
              <p className="text-base leading-[1.72] text-muted">
                {step.body}
              </p>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}
