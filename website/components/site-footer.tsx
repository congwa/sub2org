// 业务职责：提供官网收尾 CTA、仓库入口和语言切换，保证访问者读完页面后能继续进入代码或另一语言版本。
import { ArrowUpRight, Globe2 } from "lucide-react";
import type { Locale, SiteDictionary } from "@/lib/content";

type SiteFooterProps = {
  alternateLocale: Locale;
  content: SiteDictionary;
  locale: Locale;
};

const primaryButtonClass =
  "inline-flex min-h-[46px] items-center justify-center gap-2 rounded-[7px] border border-coral/40 bg-coral px-4 text-sm font-extrabold text-[#fffaf6] shadow-[0_18px_34px_rgba(255,116,72,0.20)] transition hover:-translate-y-0.5 hover:bg-[#ff6636] sm:px-5";

const secondaryButtonClass =
  "inline-flex min-h-[46px] items-center justify-center gap-2 rounded-[7px] border border-ink/[0.12] bg-white/70 px-4 text-sm font-extrabold text-ink transition hover:-translate-y-0.5 hover:border-signal-strong/[0.35] hover:bg-white sm:px-5";

/**
 * 业务职责：在页面结尾重复关键行动入口，让部署者能从品牌叙事自然进入 GitHub 仓库和文档。
 */
export function SiteFooter({
  alternateLocale,
  content,
  locale
}: SiteFooterProps) {
  return (
    <footer className="mx-auto flex w-[calc(100%-1.5rem)] max-w-[1180px] flex-col gap-6 border-t border-ink/[0.12] py-8 text-ink sm:w-[calc(100%-2rem)] sm:flex-row sm:items-center sm:justify-between sm:py-10">
      <div className="max-w-[560px]">
        <a className="mb-3 inline-flex items-center gap-2.5 font-extrabold" href={`/${locale}`}>
          <img
            alt=""
            aria-hidden="true"
            className="block h-[34px] w-[34px] shrink-0 rounded-lg object-contain shadow-[5px_5px_0_rgba(87,224,177,0.18)]"
            height={40}
            src="/codexbuy-logo.png"
            width={40}
          />
          <span className="font-display text-base font-extrabold">sub2org</span>
        </a>
        <p className="text-base leading-[1.72] text-muted">{content.footer.tagline}</p>
      </div>

      <div className="flex flex-col gap-2.5 sm:flex-row sm:items-center">
        <a
          className={secondaryButtonClass}
          href={`/${alternateLocale}`}
          hrefLang={alternateLocale === "zh" ? "zh-CN" : "en"}
        >
          <Globe2 aria-hidden="true" size={17} strokeWidth={2} />
          <span>{content.footer.switchLanguage}</span>
        </a>
        <a
          className={primaryButtonClass}
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
