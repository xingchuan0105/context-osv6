import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ToolResultCard, ToolResultsPanel } from "../../components/workspace/workspace-chat-pane";
import type { ToolResult } from "../../lib/workspace/stream";

function makeResult(tool: string, status: ToolResult["status"], data: Record<string, unknown> | null): ToolResult {
  return {
    tool,
    version: "1.0",
    status,
    data,
  };
}

describe("ToolResultsPanel", () => {
  it("returns null when no results", () => {
    const { container } = render(<ToolResultsPanel locale="en" results={[]} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders multiple tool result cards", () => {
    const results: ToolResult[] = [
      makeResult("calculator", "ok", { result: 42, expression: "6*7" }),
      makeResult("weather_query", "ok", { location: "Beijing", temperature: 25 }),
    ];
    render(<ToolResultsPanel locale="en" results={results} />);
    expect(screen.getByText("Calculator")).toBeTruthy();
    expect(screen.getByText("Weather")).toBeTruthy();
  });
});

describe("ToolResultCard — calculator", () => {
  it("renders expression and result", () => {
    render(
      <ToolResultCard
        locale="en"
        result={makeResult("calculator", "ok", { result: 42, expression: "6 * 7" })}
      />,
    );
    expect(screen.getByText("Calculator")).toBeTruthy();
    expect(screen.getByText("OK")).toBeTruthy();
    expect(screen.getByText("Expression")).toBeTruthy();
    expect(screen.getByText("6 * 7")).toBeTruthy();
    expect(screen.getByText("Result")).toBeTruthy();
    expect(screen.getByText("42")).toBeTruthy();
  });

  it("renders calculator error", () => {
    render(
      <ToolResultCard
        locale="en"
        result={makeResult("calculator", "error", { error: "bad expression" })}
      />,
    );
    expect(screen.getAllByText("Error").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("bad expression")).toBeTruthy();
  });

  it("renders Chinese labels when locale is zh-CN", () => {
    render(
      <ToolResultCard
        locale="zh-CN"
        result={makeResult("calculator", "ok", { result: 42, expression: "1+1" })}
      />,
    );
    expect(screen.getByText("计算器")).toBeTruthy();
    expect(screen.getByText("表达式")).toBeTruthy();
    expect(screen.getByText("结果")).toBeTruthy();
  });
});

describe("ToolResultCard — code_interpreter", () => {
  it("renders stdout, stderr and result", () => {
    render(
      <ToolResultCard
        locale="en"
        result={makeResult("code_interpreter", "ok", {
          stdout: "hello\n",
          stderr: "warning\n",
          result: "42",
          success: true,
          exit_code: 0,
        })}
      />,
    );
    expect(screen.getByText("Code Execution")).toBeTruthy();
    expect(screen.getByText("stdout")).toBeTruthy();
    expect(screen.getByText("hello")).toBeTruthy();
    expect(screen.getByText("stderr")).toBeTruthy();
    expect(screen.getByText("warning")).toBeTruthy();
    expect(screen.getByText("Result")).toBeTruthy();
    expect(screen.getByText("42")).toBeTruthy();
  });

  it("renders error when code failed", () => {
    render(
      <ToolResultCard
        locale="en"
        result={makeResult("code_interpreter", "error", {
          error: "SyntaxError",
        })}
      />,
    );
    expect(screen.getAllByText("Error").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("SyntaxError")).toBeTruthy();
  });
});

describe("ToolResultCard — weather_query", () => {
  it("renders weather grid", () => {
    render(
      <ToolResultCard
        locale="en"
        result={makeResult("weather_query", "ok", {
          location: "Beijing",
          description: "clear sky",
          temperature: 25,
          feels_like: 23,
          humidity: 60,
          wind_speed: 3.5,
          units: "metric",
        })}
      />,
    );
    expect(screen.getByText("Weather")).toBeTruthy();
    expect(screen.getByText(/Beijing.*clear sky/)).toBeTruthy();
    expect(screen.getByText("Temperature")).toBeTruthy();
    expect(screen.getByText("25°C")).toBeTruthy();
    expect(screen.getByText("Feels Like")).toBeTruthy();
    expect(screen.getByText("23°C")).toBeTruthy();
    expect(screen.getByText("Humidity")).toBeTruthy();
    expect(screen.getByText("60%")).toBeTruthy();
    expect(screen.getByText("Wind Speed")).toBeTruthy();
    expect(screen.getByText("3.5 m/s")).toBeTruthy();
  });

  it("renders imperial units", () => {
    render(
      <ToolResultCard
        locale="en"
        result={makeResult("weather_query", "ok", {
          location: "New York",
          temperature: 77,
          units: "imperial",
        })}
      />,
    );
    expect(screen.getByText("77°F")).toBeTruthy();
  });
});

describe("ToolResultCard — generic fallback", () => {
  it("renders unknown tool data as JSON", () => {
    render(
      <ToolResultCard
        locale="en"
        result={makeResult("custom_tool", "ok", { foo: "bar", count: 3 })}
      />,
    );
    expect(screen.getByText("custom_tool")).toBeTruthy();
    expect(screen.getByText(/"foo"/)).toBeTruthy();
    expect(screen.getByText(/"bar"/)).toBeTruthy();
  });
});

describe("ToolResultCard — collapsible", () => {
  it("collapses and expands on header click", () => {
    render(
      <ToolResultCard
        locale="en"
        result={makeResult("calculator", "ok", { result: 42, expression: "6*7" })}
      />,
    );

    const header = screen.getByRole("button", { name: /Calculator/ });
    expect(screen.getByText("Expression")).toBeTruthy();

    fireEvent.click(header);
    expect(screen.queryByText("Expression")).not.toBeTruthy();

    fireEvent.click(header);
    expect(screen.getByText("Expression")).toBeTruthy();
  });
});

describe("ToolResultCard — status badges", () => {
  it.each([
    ["ok", "OK"],
    ["error", "Error"],
    ["timeout", "Timeout"],
    ["not_found", "Not Found"],
    ["not_implemented", "Not Implemented"],
  ] as const)("renders '%s' badge as '%s'", (status, label) => {
    render(
      <ToolResultCard
        locale="en"
        result={makeResult("calculator", status, {})}
      />,
    );
    expect(screen.getByText(label)).toBeTruthy();
  });

  it("renders Chinese status labels", () => {
    render(
      <ToolResultCard
        locale="zh-CN"
        result={makeResult("calculator", "error", {})}
      />,
    );
    expect(screen.getByText("错误")).toBeTruthy();
  });
});
