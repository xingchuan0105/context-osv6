import { ImageResponse } from "next/og";

import { MetadataPreviewCard } from "./metadata-brand";

export const size = {
  width: 1200,
  height: 630,
};

export const contentType = "image/png";

export default function OpenGraphImage() {
  return new ImageResponse(<MetadataPreviewCard />, size);
}
