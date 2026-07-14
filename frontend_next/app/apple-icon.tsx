import { readFileSync } from "node:fs";
import { join } from "node:path";

/** Serve Full mark SVG as apple-touch / legacy apple-icon route. */
export const dynamic = "force-static";

export default function AppleIcon() {
  const svg = readFileSync(join(process.cwd(), "app/icon.svg"), "utf8");
  return new Response(svg, {
    headers: {
      "Content-Type": "image/svg+xml; charset=utf-8",
      "Cache-Control": "public, max-age=86400",
    },
  });
}
