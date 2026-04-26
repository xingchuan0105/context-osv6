import { beforeEach, describe, expect, it } from "vitest";

import {
  clearResetFlowState,
  clearResetEmail,
  clearResetTicket,
  readResetEmail,
  readResetTicket,
  storeResetEmail,
  storeResetTicket,
} from "../../lib/auth/reset-state";

beforeEach(() => {
  window.sessionStorage.clear();
});

describe("reset-state session helpers", () => {
  it("stores and reads reset email and ticket from sessionStorage", () => {
    storeResetEmail("user@example.com");
    storeResetTicket("ticket-123");

    expect(window.sessionStorage.getItem("context_os.reset.email.v1")).toBe("user@example.com");
    expect(window.sessionStorage.getItem("context_os.reset.ticket.v1")).toBe("ticket-123");
    expect(readResetEmail()).toBe("user@example.com");
    expect(readResetTicket()).toBe("ticket-123");
  });

  it("clears email and ticket independently and together", () => {
    storeResetEmail("user@example.com");
    storeResetTicket("ticket-123");

    clearResetEmail();

    expect(readResetEmail()).toBeNull();
    expect(readResetTicket()).toBe("ticket-123");

    clearResetTicket();

    expect(readResetTicket()).toBeNull();

    storeResetEmail("user@example.com");
    storeResetTicket("ticket-123");

    clearResetFlowState();

    expect(readResetEmail()).toBeNull();
    expect(readResetTicket()).toBeNull();
  });
});
