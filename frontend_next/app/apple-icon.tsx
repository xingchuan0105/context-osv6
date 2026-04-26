import { ImageResponse } from "next/og";

import { MetadataBrandIcon } from "./metadata-brand";

export const size = {
  width: 180,
  height: 180,
};

export const contentType = "image/png";

export default function AppleIcon() {
  return new ImageResponse(
    (
      <div
        style={{
          display: "flex",
          width: "100%",
          height: "100%",
          background: "white",
          padding: "12px",
        }}
      >
        <MetadataBrandIcon />
      </div>
    ),
    size,
  );
}
