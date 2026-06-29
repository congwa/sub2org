// 业务职责：提供官网收尾 CTA、仓库入口和语言切换，保证访问者读完页面后能继续进入代码或另一语言版本。
import { ArrowUpRight, Globe2 } from "lucide-react";
import type { Locale, SiteDictionary } from "@/lib/content";

type SiteFooterProps = {
  alternateLocale: Locale;
  content: SiteDictionary;
  locale: Locale;
};

/**
 * 业务职责：在页面结尾重复关键行动入口，让部署者能从品牌叙事自然进入 GitHub 仓库和文档。
 */
export function SiteFooter({
  alternateLocale,
  content,
  locale
}: SiteFooterProps) {
  return (
    <footer className="site-footer">
      <div>
        <a className="brand-mark footer-brand" href={`/${locale}`}>
          <span className="brand-glyph">s2o</span>
          <span className="brand-name">sub2org</span>
        </a>
        <p>{content.footer.tagline}</p>
      </div>

      <div className="footer-actions">
        <a
          className="secondary-button"
          href={`/${alternateLocale}`}
          hrefLang={alternateLocale === "zh" ? "zh-CN" : "en"}
        >
          <Globe2 aria-hidden="true" size={17} strokeWidth={2} />
          <span>{content.footer.switchLanguage}</span>
        </a>
        <a
          className="primary-button"
          href="https://github.com/congwa/sub2org"
          rel="noreferrer"
          target="_blank"
        >
          <span>{content.footer.repo}</span>
          <ArrowUpRight aria-hidden="true" size={18} strokeWidth={2.2} />
        </a>
      </div>
    </footer>
  );
}
