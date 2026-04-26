import { fileURLToPath } from "node:url";

import type { NextConfig } from "next";
import createNextIntlPlugin from "next-intl/plugin";

const projectRoot = fileURLToPath(new URL(".", import.meta.url));
const withNextIntl = createNextIntlPlugin();
const apiProxyTarget =
  process.env.API_PROXY_TARGET?.trim() || process.env.NEXT_PUBLIC_API_BASE_URL?.trim() || "http://127.0.0.1:8080";

const nextConfig: NextConfig = {
  output: "standalone",
  reactStrictMode: true,
  allowedDevOrigins: ["127.0.0.1", "localhost"],
  turbopack: {
    root: projectRoot,
  },
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
    ];
  },
};

export default withNextIntl(nextConfig);
