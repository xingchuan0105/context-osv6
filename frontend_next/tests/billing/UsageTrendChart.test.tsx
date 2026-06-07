import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { UsageTrendChart } from "../../components/billing/UsageTrendChart";

const daily = [
  { date: "2026-06-01", tokens: 50000 },
  { date: "2026-06-02", tokens: 75000 },
  { date: "2026-06-03", tokens: 60000 },
  { date: "2026-06-04", tokens: 90000 },
];

describe("UsageTrendChart", () => {
  it("renders an SVG with one polyline per data point", () => {
    const { container } = render(<UsageTrendChart daily={daily} />);
    const polyline = container.querySelector("polyline");
    expect(polyline).toBeTruthy();
    expect(polyline?.getAttribute("points")?.split(" ").length).toBe(4);
  });

  it("renders date labels on x-axis", () => {
    render(<UsageTrendChart daily={daily} />);
    expect(screen.getByText("06-01")).toBeTruthy();
    expect(screen.getByText("06-04")).toBeTruthy();
  });
});
