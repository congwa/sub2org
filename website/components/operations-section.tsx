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
    <section className="operations-section" id="ops" aria-labelledby="ops-title">
      <div className="section-heading">
        <p className="eyebrow">{content.operations.eyebrow}</p>
        <h2 id="ops-title">{content.operations.title}</h2>
        <p>{content.operations.intro}</p>
      </div>

      <div className="ops-panels">
        {content.operations.panels.map((panel, index) => {
          const Icon = panelIcons[index];

          return (
            <article className="ops-panel" key={panel.title}>
              <Icon aria-hidden="true" size={21} strokeWidth={2} />
              <h3>{panel.title}</h3>
              <p>{panel.body}</p>
            </article>
          );
        })}
      </div>
    </section>
  );
}
