// 业务职责：提供官网顶层导航和语言切换入口，让访问者快速跳到产品叙事、链路说明、运维能力与文档。
import { ArrowUpRight, GitFork, Globe2 } from "lucide-react";
import type { Locale, SiteDictionary } from "@/lib/content";

type SiteHeaderProps = {
  alternateLocale: Locale;
  content: SiteDictionary;
  locale: Locale;
};

const navLinkClass =
  "inline-flex min-h-11 shrink-0 items-center gap-1 rounded-[7px] px-3 text-sm font-bold text-white/72 transition hover:-translate-y-0.5 hover:bg-signal/[0.14] hover:text-white";

const headerButtonClass =
  "inline-flex min-h-11 items-center justify-center gap-2 rounded-[7px] border border-white/[0.14] bg-white/[0.07] px-3 text-sm font-extrabold text-[#f4fff8] shadow-[inset_0_1px_0_rgba(255,255,255,0.08)] transition hover:-translate-y-0.5 hover:border-signal/40 hover:bg-signal/[0.14] hover:text-white";

/**
 * 业务职责：在所有语言页面上保持一致的品牌入口、锚点导航和跨语言切换，降低官网首屏的探索成本。
 */
export function SiteHeader({
  alternateLocale,
  content,
  locale
}: SiteHeaderProps) {
  return (
    <header className="sticky top-2 z-30 mx-auto -mb-16 mt-2 grid w-[calc(100%-1rem)] max-w-[1180px] grid-cols-[auto_1fr] items-center gap-x-3 gap-y-2 rounded-lg border border-white/[0.16] bg-[#06110e]/82 px-3 py-2 text-[#f4fff8] shadow-[0_18px_48px_rgba(3,10,8,0.28)] backdrop-blur-xl sm:w-[calc(100%-2rem)] md:top-3 md:-mb-20 md:mt-3 md:grid-cols-[auto_1fr_auto] md:gap-x-5">
      <a
        className="inline-flex min-w-max items-center gap-2.5 font-extrabold"
        href={`/${locale}`}
        aria-label="sub2org home"
      >
        <img
          alt=""
          aria-hidden="true"
          className="block h-8 w-8 shrink-0 rounded-lg object-contain shadow-[5px_5px_0_rgba(87,224,177,0.18)] ring-1 ring-white/10 sm:h-[34px] sm:w-[34px]"
          height={40}
          src="/codexbuy-logo.png"
          width={40}
        />
        <span className="font-display text-base font-extrabold">sub2org</span>
      </a>

      <nav
        className="col-span-2 flex min-w-0 gap-1 overflow-x-auto pt-2 [scrollbar-width:none] md:col-span-1 md:justify-center md:overflow-visible md:pt-0 [&::-webkit-scrollbar]:hidden"
        aria-label="Primary navigation"
      >
        <a className={navLinkClass} href="#product">
          {content.nav.product}
        </a>
        <a className={navLinkClass} href="#flow">
          {content.nav.flow}
        </a>
        <a className={navLinkClass} href="#ops">
          {content.nav.ops}
        </a>
        <a
          className={navLinkClass}
          href="https://github.com/congwa/sub2org/blob/main/docs/USAGE.md"
          rel="noreferrer"
          target="_blank"
        >
          {content.nav.docs}
          <ArrowUpRight aria-hidden="true" size={15} strokeWidth={2.1} />
        </a>
      </nav>

      <div className="col-start-2 row-start-1 flex items-center justify-end gap-2 md:col-start-auto md:row-start-auto">
        <a
          className="hidden h-11 w-11 items-center justify-center rounded-[7px] border border-white/[0.14] bg-white/[0.07] text-[#e9fff5] shadow-[inset_0_1px_0_rgba(255,255,255,0.08)] transition hover:-translate-y-0.5 hover:border-signal/40 hover:bg-signal/[0.14] hover:text-white sm:inline-flex"
          href="https://github.com/congwa/sub2org"
          rel="noreferrer"
          target="_blank"
          aria-label={content.footer.repo}
        >
          <GitFork aria-hidden="true" size={18} strokeWidth={2} />
        </a>
        <a
          className={headerButtonClass}
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
