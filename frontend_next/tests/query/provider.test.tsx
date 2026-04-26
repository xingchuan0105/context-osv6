import { render, screen } from "@testing-library/react";
import { useQueryClient } from "@tanstack/react-query";
import { describe, expect, it } from "vitest";

import { QueryProvider } from "../../lib/query/provider";

function QueryClientProbe() {
  const queryClient = useQueryClient();

  return <div data-testid="query-client">{queryClient ? "ready" : "missing"}</div>;
}

describe("QueryProvider", () => {
  it("provides a query client to descendants", () => {
    render(
      <QueryProvider>
        <QueryClientProbe />
      </QueryProvider>,
    );

    expect(screen.getByTestId("query-client").textContent).toBe("ready");
  });
});
