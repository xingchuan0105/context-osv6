import type { ReactNode } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  pathname: "/chat/personal-1",
  search: "view=compact",
  replace: vi.fn(),
  auth: {
    initialized: true,
    isAuthenticated: false,
  },
}));

vi.mock("next/navigation", () => ({
  usePathname: () => mocks.pathname,
  useRouter: () => ({ replace: mocks.replace }),
  useSearchParams: () => new URLSearchParams(mocks.search),
}));

vi.mock("@/lib/auth/context", () => ({
  useAuth: () => mocks.auth,
}));

vi.mock("@/lib/ui-preferences", () => ({
  useUiPreferences: () => ({ locale: "zh-CN" as const, theme: "system" as const }),
}));

vi.mock("@/components/command-palette/command-palette", () => ({
  CommandPaletteHost: () => null,
}));

vi.mock("@/components/legal/LegalReacceptanceGate", () => ({
  LegalReacceptanceGate: ({ children }: { children: ReactNode }) => children,
}));

import { GuestOnlyGate, ProtectedRouteGate } from "@/components/auth-gates";

beforeEach(() => {
  mocks.pathname = "/chat/personal-1";
  mocks.search = "view=compact";
  mocks.replace.mockReset();
  mocks.auth.initialized = true;
  mocks.auth.isAuthenticated = false;
});

describe("auth gates", () => {
  it("preserves a protected chat deep-link through login", async () => {
    render(
      <ProtectedRouteGate>
        <div>private content</div>
      </ProtectedRouteGate>,
    );

    await waitFor(() => {
      expect(mocks.replace).toHaveBeenCalledWith(
        "/login?next=%2Fchat%2Fpersonal-1%3Fview%3Dcompact",
      );
    });
    expect(screen.queryByText("private content")).toBeNull();
  });

  it("sends an already authenticated guest-page visitor to chat", async () => {
    mocks.auth.isAuthenticated = true;
    render(
      <GuestOnlyGate>
        <div>login form</div>
      </GuestOnlyGate>,
    );

    await waitFor(() => {
      expect(mocks.replace).toHaveBeenCalledWith("/chat");
    });
    expect(screen.queryByText("login form")).toBeNull();
  });
});
