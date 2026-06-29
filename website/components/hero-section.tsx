// 业务职责：承载官网第一屏价值主张，把 sub2org 的 API relay 定位、核心信任点和产品视觉集中呈现。
import Image from "next/image";
import { ArrowRight, BookOpen, TerminalSquare } from "lucide-react";
import type { SiteDictionary } from "@/lib/content";

type HeroSectionProps = {
  content: SiteDictionary;
};

/**
 * 业务职责：用可复制的协议请求、关键能力标签和产品主视觉解释 sub2org 如何把浏览器 worker 池产品化。
 */
export function HeroSection({ content }: HeroSectionProps) {
  return (
    <section className="hero-section" id="product">
      <div className="hero-copy">
        <p className="eyebrow">{content.hero.eyebrow}</p>
        <h1>{content.hero.title}</h1>
        <p className="hero-lead">{content.hero.lead}</p>

        <div className="cta-row">
          <a
            className="primary-button"
            href="https://github.com/congwa/sub2org/blob/main/docs/USAGE.md"
            rel="noreferrer"
            target="_blank"
          >
            <BookOpen aria-hidden="true" size={18} strokeWidth={2.2} />
            <span>{content.hero.primaryCta}</span>
          </a>
          <a
            className="secondary-button"
            href="https://github.com/congwa/sub2org"
            rel="noreferrer"
            target="_blank"
          >
            <span>{content.hero.secondaryCta}</span>
            <ArrowRight aria-hidden="true" size={18} strokeWidth={2.2} />
          </a>
        </div>

        <div className="proof-grid" aria-label="sub2org capabilities">
          {content.hero.proof.map((item) => (
            <span key={item}>{item}</span>
          ))}
        </div>

      </div>

      <div className="hero-visual" aria-label={content.hero.imageAlt}>
        <Image
          alt={content.hero.imageAlt}
          className="hero-image"
          height={1003}
          priority
          src="/assets/relay-control-surface.jpg"
          width={1568}
        />
        <figure className="code-panel">
          <figcaption>
            <TerminalSquare aria-hidden="true" size={17} strokeWidth={2} />
            {content.hero.codeLabel}
          </figcaption>
          <pre>
            <code>{`curl -N localhost:3300/v1/chat/completions \\
  -H "authorization: Bearer sk-sub2org-dev" \\
  -d '{"model":"gpt-4o","stream":true,"messages":[{"role":"user","content":"ping"}]}'`}</code>
          </pre>
        </figure>
      </div>
    </section>
  );
}
