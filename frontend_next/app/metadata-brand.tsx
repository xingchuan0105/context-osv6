import type { CSSProperties, ReactElement } from "react";

const brandSquareStyle: CSSProperties = {
  display: "flex",
  width: "100%",
  height: "100%",
  borderRadius: "24%",
  background: "#0F1117",
  border: "1px solid rgba(255,255,255,0.14)",
  position: "relative",
  overflow: "hidden",
};

function strokeStyle(left: string, top: string, width: string, height: string, borderSide: "left" | "right"): CSSProperties {
  return {
    position: "absolute",
    left,
    top,
    width,
    height,
    borderRadius: "999px",
    borderLeft: borderSide === "left" ? "6px solid white" : undefined,
    borderRight: borderSide === "right" ? "6px solid white" : undefined,
  };
}

function lineStyle(left: string, top: string, width: string, height: string): CSSProperties {
  return {
    position: "absolute",
    left,
    top,
    width,
    height,
    borderRadius: "999px",
    background: "white",
  };
}

function dotStyle(left: string, top: string, size: string): CSSProperties {
  return {
    position: "absolute",
    left,
    top,
    width: size,
    height: size,
    borderRadius: "999px",
    background: "white",
  };
}

export function MetadataBrandIcon(): ReactElement {
  return (
    <div style={brandSquareStyle}>
      <div style={strokeStyle("27.6%", "21%", "14%", "58%", "left")} />
      <div style={strokeStyle("58.4%", "21%", "14%", "58%", "right")} />
      <div style={lineStyle("49.2%", "22%", "3.2%", "56%")} />
      <div style={lineStyle("38.8%", "41%", "12%", "2.7%")} />
      <div style={lineStyle("49.2%", "33.4%", "10%", "2.7%")} />
      <div style={lineStyle("49.2%", "58.4%", "12.5%", "2.7%")} />
      <div style={lineStyle("49.2%", "68.2%", "10%", "2.7%")} />
      <div style={dotStyle("34.6%", "39.4%", "6.6%")} />
      <div style={dotStyle("55.4%", "31.8%", "6.6%")} />
      <div style={dotStyle("59.4%", "56.8%", "6.6%")} />
      <div style={dotStyle("55.6%", "66.2%", "5.8%")} />
    </div>
  );
}

export function MetadataPreviewCard(): ReactElement {
  return (
    <div
      style={{
        display: "flex",
        width: "100%",
        height: "100%",
        flexDirection: "column",
        justifyContent: "space-between",
        background: "#F8FAFC",
        color: "#0F1117",
        padding: "56px 64px",
        fontFamily: "Inter, sans-serif",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: "20px",
        }}
      >
        <div
          style={{
            display: "flex",
            width: "92px",
            height: "92px",
          }}
        >
          <MetadataBrandIcon />
        </div>
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            gap: "8px",
          }}
        >
          <div
            style={{
              display: "flex",
              fontSize: "20px",
              fontWeight: 700,
              letterSpacing: "0.14em",
              textTransform: "uppercase",
              color: "#475569",
            }}
          >
            Context OS
          </div>
          <div
            style={{
              display: "flex",
              fontSize: "52px",
              fontWeight: 700,
              lineHeight: 1.02,
              letterSpacing: "-0.04em",
              maxWidth: "860px",
            }}
          >
            Organize and distribute knowledge with AI
          </div>
        </div>
      </div>

      <div
        style={{
          display: "flex",
          flexDirection: "column",
          gap: "18px",
          maxWidth: "860px",
        }}
      >
        <div
          style={{
            display: "flex",
            fontSize: "28px",
            lineHeight: 1.45,
            color: "#334155",
          }}
        >
          Second-brain workspace for collecting content, writing notes, sharing context, and querying knowledge through AI.
        </div>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "12px",
            fontSize: "20px",
            fontWeight: 600,
            color: "#0F1117",
          }}
        >
          <div
            style={{
              display: "flex",
              width: "12px",
              height: "12px",
              borderRadius: "999px",
              background: "#0F1117",
            }}
          />
          Second-brain workspace for organizing, distributing, and querying knowledge
        </div>
      </div>
    </div>
  );
}
