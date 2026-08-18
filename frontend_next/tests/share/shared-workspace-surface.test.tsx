import type { AnchorHTMLAttributes } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("next/link", () => ({
  default: ({
    children,
    href,
    ...props
  }: AnchorHTMLAttributes<HTMLAnchorElement> & { href: string }) => (
    <a href={href} {...props}>
      {children}
    </a>
  ),
}));

vi.mock("../../lib/ui-preferences", () => ({
  useUiPreferences: () => ({
    locale: "zh-CN" as const,
  }),
}));

vi.mock("../../lib/auth/context", () => ({
  useAuth: () => ({
    initialized: true,
    token: "token-123",
    user: { id: "u1" },
  }),
}));

vi.mock("../../lib/share/client", async () => {
  const actual = await vi.importActual("../../lib/share/client");
  return {
    ...actual,
    getSharedWorkspace: mocks.getSharedWorkspaceMock,
    streamSharedChat: mocks.streamSharedChatMock,
    getPublicOwnerProfile: mocks.getPublicOwnerProfileMock,
  };
});

vi.mock("../../hooks/use-chat-session", () => ({
  useChatSession: () => ({
    messages: [],
    isStreaming: false,
    progress: {
      activities: [],
      mode: null,
      collapsed: false,
      startedAtMs: null,
      endedAtMs: null,
    },
    error: null,
    send: vi.fn(),
    stop: vi.fn(),
    toggleProgressCollapsed: vi.fn(),
  }),
}));

import { SharedWorkspaceSurface } from "../../components/share/shared-workspace-surface";

const mocks = vi.hoisted(() => globalThis.__mockProviders.createSharedWorkspaceSurfaceMocks());

function buildPayload(overrides?: Partial<Awaited<ReturnType<typeof mocks.getSharedWorkspaceMock>>>) {
  return {
    knowledge_base: {
      id: "kb-1",
      title: "Shared KB",
      description: "Shared description",
    },
    share: {
      permission: "partial",
      expires_at: "2026-04-30T18:00:00Z",
      allow_download: false,
      scope: "partial",
    },
    sources: [
      {
        id: "src-1",
        file_name: "Plan.pdf",
        status: "ready",
      },
      {
        id: "src-2",
        file_name: "Appendix.txt",
        status: "processing",
      },
    ],
    ...overrides,
  };
}

describe("SharedWorkspaceSurface", () => {
  beforeEach(() => {
    mocks.getSharedWorkspaceMock.mockReset();
    mocks.streamSharedChatMock.mockReset();
    mocks.getPublicOwnerProfileMock.mockReset();
    window.localStorage.clear();
  });

  it("shows the loading state while the shared payload is pending", () => {
    mocks.getSharedWorkspaceMock.mockImplementation(() => new Promise(() => undefined));

    render(<SharedWorkspaceSurface shareToken="share-loading" />);

    expect(screen.getByText(/正在加载共享内容/)).toBeTruthy();
  });

  it("renders the invalid link state without calling the share client for an empty token", async () => {
    render(<SharedWorkspaceSurface shareToken="" />);

    expect(await screen.findByText("共享链接不可用")).toBeTruthy();
    expect(mocks.getSharedWorkspaceMock).not.toHaveBeenCalled();
  });

  it("renders workspace-like shell with title-bar owner card, chat, sessions, sources", async () => {
    mocks.getSharedWorkspaceMock.mockResolvedValue(
      buildPayload({
        owner: {
          user_id: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
          display_name: "Ada Owner",
          bio: "Building second brains",
          contact_url: "https://example.com/ada",
          avatar_url: "/api/public/users/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee/media/avatar",
          banner_url: "/api/public/users/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee/media/banner",
        },
      }),
    );

    render(<SharedWorkspaceSurface shareToken="share-partial" />);

    expect(await screen.findByTestId("shared-workspace-shell")).toBeTruthy();
    expect(screen.getByTestId("share-owner-card")).toBeTruthy();
    expect(screen.getByTestId("share-owner-banner")).toBeTruthy();
    expect(screen.getByTestId("share-owner-avatar")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Shared KB" })).toBeTruthy();
    expect(screen.getByText("Ada Owner")).toBeTruthy();
    expect(screen.getByText("Building second brains")).toBeTruthy();
    expect(screen.getByText("Shared description")).toBeTruthy();
    const profileHref = "/shared/u/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
    expect(screen.getByTestId("share-owner-avatar").closest("a")).toHaveAttribute(
      "href",
      profileHref,
    );
    expect(screen.getByRole("link", { name: "Ada Owner" })).toHaveAttribute("href", profileHref);
    expect(screen.getByRole("link", { name: "联系" })).toHaveAttribute(
      "href",
      "https://example.com/ada",
    );
    expect(screen.getByText("Shared KB")).toBeTruthy();
    expect(screen.getByTestId("shared-history-rail")).toBeTruthy();
    expect(screen.getByTestId("shared-chat-pane")).toBeTruthy();
    expect(screen.getByTestId("shared-desktop-right-rail")).toBeTruthy();
    expect(screen.getAllByText("Plan.pdf").length).toBeGreaterThan(0);

    // No add-source control
    expect(screen.queryByRole("button", { name: /新建资料|New source/i })).toBeNull();
    // RAG chip present; search chip absent
    expect(screen.getByTestId("workspace-chat-cap-rag")).toBeTruthy();
    expect(screen.queryByTestId("workspace-chat-cap-search")).toBeNull();
  });

  it("renders hero without owner media using workspace title fallback", async () => {
    mocks.getSharedWorkspaceMock.mockResolvedValue(
      buildPayload({
        share: {
          permission: "full",
          expires_at: null,
          allow_download: true,
          scope: "full",
        },
      }),
    );

    render(<SharedWorkspaceSurface shareToken="share-full" />);

    expect(await screen.findByTestId("share-owner-card")).toBeTruthy();
    expect(screen.getByText("Shared KB")).toBeTruthy();
    expect(screen.getByText("允许下载")).toBeTruthy();
  });

  it("switches to the sources tab and opens detail only for ready sources", async () => {
    mocks.getSharedWorkspaceMock.mockResolvedValue(buildPayload());

    render(<SharedWorkspaceSurface shareToken="share-tabs" />);

    expect(await screen.findByTestId("share-tab-chat")).toBeTruthy();
    fireEvent.click(screen.getByTestId("share-tab-sources"));

    const sourcesTab = await screen.findByTestId("shared-sources-tab");
    expect(sourcesTab).toBeTruthy();

    const readyCard = screen.getByTestId("share-source-card-src-1");
    const processingCard = screen.getByTestId("share-source-card-src-2");
    expect(processingCard).toHaveProperty("disabled", true);

    // Ready source opens the detail modal with the ask CTA.
    fireEvent.click(readyCard);
    expect(await screen.findByTestId("shared-source-detail-modal")).toBeTruthy();
    expect(screen.getByTestId("shared-source-ask-action")).toBeTruthy();

    // Ask CTA prefills the composer and returns to the chat tab.
    fireEvent.click(screen.getByTestId("shared-source-ask-action"));
    expect(await screen.findByTestId("share-tab-chat")).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("lists the owner's other public shares in the more-shares tab", async () => {
    mocks.getSharedWorkspaceMock.mockResolvedValue(
      buildPayload({
        owner: {
          user_id: "owner-1",
          display_name: "Ada Owner",
        },
      }),
    );
    mocks.getPublicOwnerProfileMock.mockResolvedValue({
      owner: { user_id: "owner-1", display_name: "Ada Owner" },
      shares: [
        {
          workspace_id: "kb-1",
          title: "Current KB",
          share_token: "share-tabs",
          access_level: "public",
          allow_download: false,
          source_count: 2,
        },
        {
          workspace_id: "kb-2",
          title: "Second Brain",
          description: "Notes and papers",
          share_token: "token-2",
          access_level: "public",
          allow_download: true,
          source_count: 7,
        },
      ],
    });

    render(<SharedWorkspaceSurface shareToken="share-tabs" />);

    fireEvent.click(await screen.findByTestId("share-tab-shares"));

    expect(await screen.findByTestId("shared-more-shares-tab")).toBeTruthy();
    // Current share is filtered out; the other one links to its share page.
    expect(screen.queryByText("Current KB")).toBeNull();
    expect(await screen.findByText("Second Brain")).toBeTruthy();
    expect(screen.getByRole("link", { name: "打开" })).toHaveAttribute(
      "href",
      "/shared/kb/token-2",
    );
  });

  it("hides the more-shares tab when the owner has no public profile id", async () => {
    mocks.getSharedWorkspaceMock.mockResolvedValue(buildPayload());

    render(<SharedWorkspaceSurface shareToken="share-no-owner" />);

    expect(await screen.findByTestId("share-tab-chat")).toBeTruthy();
    expect(screen.queryByTestId("share-tab-shares")).toBeNull();
  });

  it("hides owner profile entry points when profile_enabled is false", async () => {
    mocks.getSharedWorkspaceMock.mockResolvedValue(
      buildPayload({
        owner: {
          user_id: "owner-1",
          display_name: "Ada Owner",
          profile_enabled: false,
        },
      }),
    );

    const { container } = render(<SharedWorkspaceSurface shareToken="share-private-owner" />);

    expect(await screen.findByTestId("share-tab-chat")).toBeTruthy();
    expect(screen.queryByTestId("share-tab-shares")).toBeNull();
    expect(container.querySelector('a[href="/shared/u/owner-1"]')).toBeNull();
    // Owner card itself still renders (name is share-page context, not the profile link).
    expect(screen.getByTestId("share-owner-card")).toBeTruthy();
  });
});
