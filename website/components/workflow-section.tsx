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
    <section className="workflow-section" id="flow" aria-labelledby="workflow-title">
      <div className="section-heading">
        <p className="eyebrow">{content.workflow.eyebrow}</p>
        <h2 id="workflow-title">{content.workflow.title}</h2>
        <p>{content.workflow.intro}</p>
      </div>

      <div className="timeline">
        {content.workflow.steps.map((step) => (
          <article className="timeline-item" key={step.number}>
            <span className="timeline-number">{step.number}</span>
            <div>
              <h3>{step.title}</h3>
              <p>{step.body}</p>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}
