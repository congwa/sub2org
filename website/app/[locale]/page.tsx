// 业务职责：渲染 sub2org 官网的双语 SSR 页面，把项目架构、价值主张和运维能力以同一模板呈现。
import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { FeatureGrid } from "@/components/feature-grid";
import { HeroSection } from "@/components/hero-section";
import { OperationsSection } from "@/components/operations-section";
import { ProtocolStrip } from "@/components/protocol-strip";
import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";
import { WorkflowSection } from "@/components/workflow-section";
import {
  getAlternateLocale,
  getDictionary,
  getHtmlLang,
  isLocale
} from "@/lib/content";

export const dynamic = "force-dynamic";

type LocalePageProps = {
  params: Promise<{
    locale: string;
  }>;
};

/**
 * 业务职责：为不同语言版本生成本地化 SEO 信息和 hreflang，使官网分享与搜索结果能准确表达 relay API 定位。
 */
export async function generateMetadata({
  params
}: LocalePageProps): Promise<Metadata> {
  const { locale } = await params;

  if (!isLocale(locale)) {
    return {};
  }

  const content = getDictionary(locale);

  return {
    title: content.meta.title,
    description: content.meta.description,
    alternates: {
      canonical: `/${locale}`,
      languages: {
        "zh-CN": "/zh",
        en: "/en"
      }
    }
  };
}

/**
 * 业务职责：根据 URL 中的 locale 输出完整官网内容，未知语言直接 404，避免错误语言混入正式传播链路。
 */
export default async function LocaleHomePage({ params }: LocalePageProps) {
  const { locale } = await params;

  if (!isLocale(locale)) {
    notFound();
  }

  const content = getDictionary(locale);
  const alternateLocale = getAlternateLocale(locale);

  return (
    <main className="min-h-screen overflow-x-clip" lang={getHtmlLang(locale)}>
      <SiteHeader
        alternateLocale={alternateLocale}
        content={content}
        locale={locale}
      />
      <HeroSection content={content} />
      <ProtocolStrip content={content} />
      <FeatureGrid content={content} />
      <WorkflowSection content={content} />
      <OperationsSection content={content} />
      <SiteFooter
        alternateLocale={alternateLocale}
        content={content}
        locale={locale}
      />
    </main>
  );
}
