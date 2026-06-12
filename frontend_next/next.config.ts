import { fileURLToPath } from "node:url";

import type { NextConfig } from "next";
import createNextIntlPlugin from "next-intl/plugin";

const projectRoot = fileURLToPath(new URL(".", import.meta.url));
const withNextIntl = createNextIntlPlugin();

// 桌面构建：BUILD_TARGET=desktop
const isDesktopBuild = process.env.BUILD_TARGET === "desktop";

const apiProxyTarget =
  process.env.API_PROXY_TARGET?.trim() || process.env.NEXT_PUBLIC_API_BASE_URL?.trim() || "http://127.0.0.1:8080";

const nextConfig: NextConfig = {
  // 桌面端：静态导出；Web 端：standalone
  output: isDesktopBuild ? "export" : "standalone",

  reactStrictMode: true,
  allowedDevOrigins: ["127.0.0.1", "localhost"],

  // 桌面端构建时忽略 TypeScript 错误（playwright 配置等非关键错误）
  ...(isDesktopBuild
    ? {
        typescript: {
          ignoreBuildErrors: true,
        },
      }
    : {}),

  turbopack: {
    root: projectRoot,
  },

  // 桌面端：不支持 rewrites（静态导出）
  // Web 端：保持 API 代理
  ...(isDesktopBuild
    ? {}
    : {
        async rewrites() {
          return [
            {
              source: "/api/auth/:path*",
              destination: `${apiProxyTarget}/api/auth/:path*`,
            },
            {
              source: "/api/v1/:path*",
              destination: `${apiProxyTarget}/api/v1/:path*`,
            },
            {
              source: "/api/shared/:path*",
              destination: `${apiProxyTarget}/api/shared/:path*`,
            },
            {
              source: "/api/e2e/:path*",
              destination: `${apiProxyTarget}/api/e2e/:path*`,
            },
          ];
        },
      }),

  // 桌面端：禁用图片优化（静态导出不支持）
  ...(isDesktopBuild
    ? {
        images: {
          unoptimized: true,
        },
      }
    : {}),
};

export default withNextIntl(nextConfig);
