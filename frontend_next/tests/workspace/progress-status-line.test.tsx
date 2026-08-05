import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

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
  it("renders only the latest 4 steps in a rolling window while live", () => {
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

  it("auto-collapses when completed and toggles via the collapse button", () => {
    const onToggle = vi.fn();
    const activities = [entry("a1", "正在理解问题"), entry("a2", "正在整理回答")];
    const { rerender } = render(
      <ProgressStatusLine
        activities={activities}
        collapsed
        locale="zh-CN"
        mode="chat"
        onToggleCollapsed={onToggle}
        startedAtMs={Date.now() - 3000}
        endedAtMs={Date.now()}
      />,
    );

    expect(
      screen.getByTestId("workspace-progress-status-line").getAttribute("data-progress-state"),
    ).toBe("completed");
    expect(screen.getByTestId("workspace-progress-status-line").getAttribute("data-collapsed")).toBe(
      "true",
    );
    // Collapsed: summary title only, not step bodies.
    expect(screen.getByText("思考完成")).toBeTruthy();
    expect(screen.queryByText("正在整理回答")).toBeNull();

    fireEvent.click(screen.getByTestId("workspace-progress-collapse-toggle"));
    expect(onToggle).toHaveBeenCalledTimes(1);

    rerender(
      <ProgressStatusLine
        activities={activities}
        collapsed={false}
        locale="zh-CN"
        mode="chat"
        onToggleCollapsed={onToggle}
        startedAtMs={Date.now() - 3000}
        endedAtMs={Date.now()}
      />,
    );
    expect(screen.getByText("正在整理回答")).toBeTruthy();
  });

  it("keeps step detail inline with the title while live", () => {
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