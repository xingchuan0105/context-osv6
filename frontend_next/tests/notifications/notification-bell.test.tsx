import type { ReactElement } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  listNotificationsMock: vi.fn(),
  markNotificationReadMock: vi.fn(),
  authState: {
    token: "token-123" as string | null,
    user: { id: "u1", email: "a@b.c", full_name: "A" },
  },
}));

vi.mock("../../lib/auth/context", () => ({
  useAuth: () => mocks.authState,
}));

vi.mock("../../lib/settings/client", () => ({
  listNotifications: mocks.listNotificationsMock,
  markNotificationRead: mocks.markNotificationReadMock,
}));

import { NotificationBell } from "../../components/notifications/notification-bell";

function renderBell() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <NotificationBell locale="en" />
    </QueryClientProvider>,
  );
}

describe("NotificationBell", () => {
  beforeEach(() => {
    mocks.listNotificationsMock.mockReset();
    mocks.markNotificationReadMock.mockReset();
    mocks.authState.token = "token-123";
    mocks.listNotificationsMock.mockResolvedValue({
      notifications: [
        {
          id: "n1",
          owner_user_id: "u1",
          user_id: "u1",
          event_type: "share.enabled",
          title: "Share enabled",
          body: "Your workspace is shared.",
          data: {},
          read_at: null,
          created_at: "2026-08-06T00:00:00Z",
          updated_at: "2026-08-06T00:00:00Z",
        },
      ],
    });
  });

  it("shows unread badge and lists notifications in flyout", async () => {
    const user = userEvent.setup();
    renderBell();

    await waitFor(() => {
      expect(screen.getByTestId("notification-bell-unread")).toHaveTextContent("1");
    });

    await user.click(screen.getByTestId("notification-bell"));
    expect(await screen.findByTestId("notification-bell-panel")).toBeTruthy();
    expect(screen.getByText("Share enabled")).toBeTruthy();
  });

  it("marks a notification as read", async () => {
    const user = userEvent.setup();
    mocks.markNotificationReadMock.mockResolvedValue(undefined);
    renderBell();

    await user.click(await screen.findByTestId("notification-bell"));
    await user.click(screen.getByRole("button", { name: /Mark as read|标为已读/i }));

    await waitFor(() => {
      expect(mocks.markNotificationReadMock).toHaveBeenCalledWith("token-123", "n1");
    });
  });
});
