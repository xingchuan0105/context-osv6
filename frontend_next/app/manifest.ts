import type { MetadataRoute } from "next";

export const dynamic = "force-static";

export default function manifest(): MetadataRoute.Manifest {
  return {
    name: "Context OS",
    short_name: "Context OS",
    description: "Second-brain workspace for organizing, distributing, and querying knowledge with AI.",
    start_url: "/",
    display: "standalone",
    background_color: "#F8FAFC",
    theme_color: "#0F1117",
    icons: [
      {
        src: "/icon.svg",
        sizes: "76x76",
        type: "image/svg+xml",
        purpose: "any",
      },
      {
        src: "/apple-icon",
        sizes: "180x180",
        type: "image/png",
        purpose: "any",
      },
    ],
  };
}
