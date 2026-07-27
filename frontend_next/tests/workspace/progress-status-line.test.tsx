import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ProgressStatusLine } from "../../components/workspace/progress-status-line";
import type { ProgressEntry } from "../../hooks/use-chat-session";

function entry(id: string, title: string, detail: string | null = null): ProgressEntry {
  return {
    id,
    phase: "act",
    title,
    detail,
    counts: {},
    sourcesPreview: [],
    timestamp: null,
  };
}

describe("ProgressStatusLine", () => {
  it("renders only the latest 4 steps in a rolling window", () => {
    const activities = [
      entry("a1", "正在理解问题"),
      entry("a2", "正在检索网页"),
      entry("a3", "正在网页搜索"),
      entry("a4", "完成网页搜索"),
      entry("a5", "正在读取网页"),
      entry("a6", "正在整理回答"),
    ];
    render(
      <ProgressStatusLine
        activities={activities}
        locale="zh-CN"
        mode="search"
        startedAtMs={Date.now() - 5000}
      />,
    );

    // Rolling window: last 4 only.
    expect(screen.getByText("正在网页搜索")).toBeTruthy();
    expect(screen.getByText("完成网页搜索")).toBeTruthy();
    expect(screen.getByText("正在读取网页")).toBeTruthy();
    expect(screen.getByText("正在整理回答")).toBeTruthy();
    expect(screen.queryByText("正在理解问题")).toBeNull();
    expect(screen.queryByText("正在检索网页")).toBeNull();

    // Timer pinned in its fixed slot.
    expect(screen.getByTestId("workspace-progress-elapsed")).toBeTruthy();
  });

  it("shows fallback row before the first fact and done icon when completed", () => {
    const { container } = render(
      <ProgressStatusLine
        activities={[]}
        locale="zh-CN"
        mode="chat"
        startedAtMs={Date.now() - 1000}
        endedAtMs={Date.now()}
      />,
    );
    expect(screen.getByText("正在理解问题")).toBeTruthy();
    expect(
      screen.getByTestId("workspace-progress-status-line").getAttribute("data-progress-state"),
    ).toBe("completed");
    expect(container.querySelectorAll("strong").length).toBe(1);
  });

  it("keeps step detail inline with the title", () => {
    render(
      <ProgressStatusLine
        activities={[entry("a1", "正在网页搜索", "2025 市场规模 预测")]}
        locale="zh-CN"
        mode="search"
        startedAtMs={Date.now() - 2000}
      />,
    );
    expect(screen.getByText("正在网页搜索")).toBeTruthy();
    expect(screen.getByText("2025 市场规模 预测")).toBeTruthy();
  });
});
