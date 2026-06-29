// 业务职责：提供官网顶层导航和语言切换入口，让访问者快速跳到产品叙事、链路说明、运维能力与文档。
import { ArrowUpRight, GitFork, Globe2 } from "lucide-react";
import type { Locale, SiteDictionary } from "@/lib/content";

type SiteHeaderProps = {
  alternateLocale: Locale;
  content: SiteDictionary;
  locale: Locale;
};

/**
 * 业务职责：在所有语言页面上保持一致的品牌入口、锚点导航和跨语言切换，降低官网首屏的探索成本。
 */
export function SiteHeader({
  alternateLocale,
  content,
  locale
}: SiteHeaderProps) {
  return (
    <header className="site-header">
      <a className="brand-mark" href={`/${locale}`} aria-label="sub2org home">
        <span className="brand-glyph">s2o</span>
        <span className="brand-name">sub2org</span>
      </a>

      <nav className="nav-links" aria-label="Primary navigation">
        <a href="#product">{content.nav.product}</a>
        <a href="#flow">{content.nav.flow}</a>
        <a href="#ops">{content.nav.ops}</a>
        <a
          href="https://github.com/congwa/sub2org/blob/main/docs/USAGE.md"
          rel="noreferrer"
          target="_blank"
        >
          {content.nav.docs}
          <ArrowUpRight aria-hidden="true" size={15} strokeWidth={2.1} />
        </a>
      </nav>

      <div className="header-actions">
        <a
          className="icon-link"
          href="https://github.com/congwa/sub2org"
          rel="noreferrer"
          target="_blank"
          aria-label={content.footer.repo}
        >
          <GitFork aria-hidden="true" size={18} strokeWidth={2} />
        </a>
        <a
          className="language-switch"
          href={`/${alternateLocale}`}
          hrefLang={alternateLocale === "zh" ? "zh-CN" : "en"}
        >
          <Globe2 aria-hidden="true" size={17} strokeWidth={2} />
          <span>{content.alternateLanguageName}</span>
        </a>
      </div>
    </header>
  );
}
