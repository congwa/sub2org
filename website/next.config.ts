import type { NextConfig } from "next";

/**
 * 业务职责：保持官网以 Next App Router 运行，既能承载静态营销内容，也保留 SSR 路由与服务端元数据能力。
 */
const nextConfig: NextConfig = {
  output: "standalone",
  reactStrictMode: true
};

export default nextConfig;
