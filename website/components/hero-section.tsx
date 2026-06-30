// 业务职责：承载官网第一屏价值主张，把 sub2org 的 API relay 定位、核心信任点和产品视觉集中呈现。
import Image from "next/image";
import { ArrowRight, BookOpen, TerminalSquare } from "lucide-react";
import type { SiteDictionary } from "@/lib/content";

type HeroSectionProps = {
  content: SiteDictionary;
};

const primaryButtonClass =
  "inline-flex min-h-[46px] items-center justify-center gap-2 rounded-[7px] border border-coral/40 bg-coral px-4 text-sm font-extrabold text-[#fffaf6] shadow-[0_18px_34px_rgba(255,116,72,0.22)] transition hover:-translate-y-0.5 hover:bg-[#ff6636] hover:shadow-[0_22px_42px_rgba(255,116,72,0.28)] sm:px-5";

const secondaryButtonClass =
  "inline-flex min-h-[46px] items-center justify-center gap-2 rounded-[7px] border border-white/20 bg-white/[0.08] px-4 text-sm font-extrabold text-[#f5fff8] transition hover:-translate-y-0.5 hover:border-signal/40 hover:bg-signal/[0.15] hover:text-white sm:px-5";

/**
 * 业务职责：用可复制的协议请求、关键能力标签和产品主视觉解释 sub2org 如何把浏览器 worker 池产品化。
 */
export function HeroSection({ content }: HeroSectionProps) {
  return (
    <section
      className="hero-shell relative flex min-h-[690px] w-full flex-col justify-end overflow-hidden bg-night px-4 pb-10 pt-[8.5rem] text-[#f4fff8] sm:min-h-[min(760px,calc(100svh-108px))] sm:px-6 sm:pb-14 sm:pt-[9.5rem] lg:min-h-[min(790px,calc(100svh-96px))] lg:px-[max(1rem,calc((100%-1180px)/2))] lg:pb-16 xl:pb-[76px]"
      id="product"
    >
      <div className="relative z-10 mx-auto grid w-full max-w-[1180px] items-end gap-8 lg:grid-cols-[minmax(0,0.98fr)_minmax(320px,0.72fr)]">
        <div className="min-w-0">
          <p className="mb-3.5 inline-flex rounded-full border border-signal/25 bg-[#04100d]/55 px-3 py-1.5 font-mono text-xs font-extrabold uppercase text-signal backdrop-blur-sm sm:text-[0.78rem]">
            {content.hero.eyebrow}
          </p>
          <h1
            aria-label={content.hero.title}
            className="mb-6 max-w-[820px] break-words font-display text-[2.55rem] font-[860] leading-[0.97] tracking-normal text-white text-balance sm:text-[3.65rem] lg:text-[4.82rem] lg:leading-[0.94]"
          >
            {content.hero.titleLines.map((line) => (
              <span className="block" key={line}>
                {line}
              </span>
            ))}
          </h1>
          <p className="mb-7 max-w-[31ch] text-base leading-[1.66] text-white/[0.84] [overflow-wrap:anywhere] sm:max-w-[710px] sm:text-[1.13rem] sm:leading-[1.72]">
            {content.hero.lead}
          </p>

          <div className="flex w-full max-w-[360px] flex-col gap-3 sm:max-w-none sm:flex-row sm:items-center">
            <a
              className={primaryButtonClass}
              href="https://github.com/congwa/sub2org/blob/main/docs/USAGE.md"
              rel="noreferrer"
              target="_blank"
            >
              <BookOpen aria-hidden="true" size={18} strokeWidth={2.2} />
              <span>{content.hero.primaryCta}</span>
            </a>
            <a
              className={secondaryButtonClass}
              href="https://github.com/congwa/sub2org"
              rel="noreferrer"
              target="_blank"
            >
              <span>{content.hero.secondaryCta}</span>
              <ArrowRight aria-hidden="true" size={18} strokeWidth={2.2} />
            </a>
          </div>

          <div
            className="mt-5 grid max-w-[390px] grid-cols-2 gap-2 sm:flex sm:max-w-[660px] sm:flex-wrap lg:hidden"
            aria-label="sub2org capabilities"
          >
            {content.hero.proof.map((item) => (
              <span
                className="min-h-8 rounded-[7px] border border-signal/25 bg-[#040e0b]/45 px-2.5 py-2 font-mono text-[0.72rem] leading-tight text-white/[0.82] sm:text-[0.74rem]"
                key={item}
              >
                {item}
              </span>
            ))}
          </div>
        </div>

        <aside
          className="switchboard-panel relative hidden min-h-[420px] overflow-hidden rounded-lg border border-white/[0.16] bg-[#040d0a]/70 p-4 shadow-strong backdrop-blur-xl lg:block"
          aria-label="sub2org capabilities"
        >
          <div className="grid gap-2">
            {content.hero.proof.map((item, index) => (
              <div
                className="micro-lift rounded-[7px] border border-signal/20 bg-white/[0.06] p-3 hover:-translate-y-0.5 hover:border-signal/45 hover:bg-signal/[0.10]"
                key={item}
              >
                <span className="font-mono text-[0.68rem] font-extrabold uppercase text-white/45">
                  {String(index + 1).padStart(2, "0")}
                </span>
                <strong className="mt-2 block text-sm font-extrabold leading-tight text-white">
                  {item}
                </strong>
              </div>
            ))}
          </div>

          {content.hero.showCodePanel ? (
            <figure className="mt-3 overflow-hidden rounded-lg border border-signal/25 bg-[#020806]/80 shadow-[0_18px_50px_rgba(0,0,0,0.28)]">
              <figcaption className="flex items-center gap-2 border-b border-white/10 px-3.5 py-2.5 text-sm font-extrabold text-[#dffbed]">
                <TerminalSquare aria-hidden="true" size={17} strokeWidth={2} />
                {content.hero.codeLabel}
              </figcaption>
              <pre className="m-0 overflow-x-auto p-3.5">
                <code className="whitespace-pre font-mono text-[0.72rem] leading-[1.55] text-[#e5fff1]">{`curl -N localhost:3300/v1/chat/completions \\
  -H "authorization: Bearer sk-sub2org-dev" \\
  -d '{"model":"gpt-4o","stream":true,"messages":[{"role":"user","content":"ping"}]}'`}</code>
              </pre>
            </figure>
          ) : (
            <div aria-hidden="true" className="mt-4 grid grid-cols-5 gap-2">
              {content.hero.proof.map((item, index) => (
                <span
                  className="h-16 rounded-[7px] border border-white/10 bg-[linear-gradient(180deg,rgba(87,224,177,0.2),rgba(255,255,255,0.04))]"
                  key={`${item}-${index}`}
                />
              ))}
              <span className="h-16 rounded-[7px] border border-coral/25 bg-coral/15" />
            </div>
          )}
        </aside>
      </div>

      <div
        className="relay-image absolute inset-0 z-0 min-w-0"
        aria-label={content.hero.imageAlt}
      >
        <Image
          alt={content.hero.imageAlt}
          className="block h-full w-full border-0 object-cover object-[60%_50%] opacity-[0.62] saturate-[0.9] contrast-[1.08] sm:opacity-[0.82]"
          height={1003}
          priority
          src="/assets/relay-control-surface.jpg"
          width={1568}
        />
      </div>
    </section>
  );
}
