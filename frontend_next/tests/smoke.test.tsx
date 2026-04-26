import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it } from "vitest";

describe("test harness smoke", () => {
  it("renders and handles user interaction", async () => {
    const user = userEvent.setup();

    function Counter() {
      const [count, setCount] = useState(0);

      return (
        <button type="button" onClick={() => setCount((value) => value + 1)}>
          {count}
        </button>
      );
    }

    render(<Counter />);

    await user.click(screen.getByRole("button", { name: "0" }));

    expect(screen.getByRole("button", { name: "1" })).toBeTruthy();
  });
});
