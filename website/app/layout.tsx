// 业务职责：定义官网全局 HTML 外壳、字体变量和默认 SEO 信息，保障所有语言页面拥有一致的服务端渲染基础。
import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: {
    default: "sub2org",
    template: "%s | sub2org"
  },
  description:
    "sub2org turns browser-driven AI sessions into an OpenAI-compatible relay API with worker scheduling, streaming, billing, and operations visibility.",
  icons: {
    icon: [{ url: "/codexbuy-logo.png", type: "image/png" }],
    apple: [{ url: "/codexbuy-logo.png", type: "image/png" }]
  }
};

type RootLayoutProps = Readonly<{
  children: React.ReactNode;
}>;

/**
 * 业务职责：为官网所有页面注入统一字体、背景和可访问的页面根节点，让中英文 SSR 页面共享同一品牌框架。
 */
export default function RootLayout({ children }: RootLayoutProps) {
  return (
    <html lang="zh-CN">
      <body>{children}</body>
    </html>
  );
}
