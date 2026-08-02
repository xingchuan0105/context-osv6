// Twitter 卡片图：与 opengraph-image 同图（Cream × Void）；
// 桌面静态导出（BUILD_TARGET=desktop）下 ImageResponse 有兼容性问题，保持原重定向行为。

import { ImageResponse } from "next/og";
import { MetadataPreviewCard } from "./metadata-brand";

export const dynamic = "force-static";

export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

export default function TwitterImage() {
  if (process.env.BUILD_TARGET === "desktop") {
    return new Response(null, {
      status: 301,
      headers: { Location: "/twitter-image.png" },
    });
  }
  return new ImageResponse(<MetadataPreviewCard />, { ...size });
}
