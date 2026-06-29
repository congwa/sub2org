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

/**
 * 业务职责：把 sub2org 从浏览器自动化到可运营 API 的关键能力拆成可扫描卡片，支撑官网转化判断。
 */
export function FeatureGrid({ content }: FeatureGridProps) {
  return (
    <section className="feature-section" aria-labelledby="feature-title">
      <div className="section-heading">
        <p className="eyebrow">{content.features.eyebrow}</p>
        <h2 id="feature-title">{content.features.title}</h2>
        <p>{content.features.kicker}</p>
      </div>

      <div className="feature-grid">
        {content.features.items.map((feature) => {
          const Icon = featureIconMap[feature.icon];

          return (
            <article className="feature-card" key={feature.title}>
              <div className="feature-icon">
                <Icon aria-hidden="true" size={22} strokeWidth={2} />
              </div>
              <h3>{feature.title}</h3>
              <p>{feature.body}</p>
            </article>
          );
        })}
      </div>
    </section>
  );
}
