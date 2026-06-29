// 业务职责：展示 sub2org 的生产运维、可观测和存储策略，回应部署者对可靠性与维护成本的核心顾虑。
import { Activity, DatabaseZap, ListChecks } from "lucide-react";
import type { SiteDictionary } from "@/lib/content";

type OperationsSectionProps = {
  content: SiteDictionary;
};

const panelIcons = [Activity, DatabaseZap, ListChecks];

/**
 * 业务职责：把里程碑、指标、日志和持久化策略放在同一段落，突出单实例架构下的运维可控性。
 */
export function OperationsSection({ content }: OperationsSectionProps) {
  return (
    <section className="operations-section" id="ops" aria-labelledby="ops-title">
      <div className="ops-layout">
        <div className="section-heading">
          <p className="eyebrow">Operate with confidence</p>
          <h2 id="ops-title">{content.operations.title}</h2>
          <p>{content.operations.intro}</p>
        </div>

        <div className="metric-grid" aria-label="sub2org operational metrics">
          {content.operations.metrics.map((metric) => (
            <div className="metric-tile" key={metric.label}>
              <strong>{metric.value}</strong>
              <span>{metric.label}</span>
            </div>
          ))}
        </div>
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
