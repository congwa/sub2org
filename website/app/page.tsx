// 业务职责：根据访问者语言偏好把官网根路径导向对应语言页，减少首次进入时的选择成本。
import { headers } from "next/headers";
import { redirect } from "next/navigation";

export const dynamic = "force-dynamic";

/**
 * 业务职责：用请求头识别中文或英文偏好，并把静态官网入口稳定落到可 SSR 的 locale 路由。
 */
export default async function HomeRedirect() {
  const headerList = await headers();
  const acceptLanguage = headerList.get("accept-language")?.toLowerCase() ?? "";
  const locale = acceptLanguage.includes("zh") ? "zh" : "en";

  redirect(`/${locale}`);
}
