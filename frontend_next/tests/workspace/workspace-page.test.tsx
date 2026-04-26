import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const mockedSurface = vi.hoisted(() => vi.fn());

vi.mock("../../components/workspace/workspace-surface", () => ({
  WorkspaceSurface: (props: { workspaceId: string }) => mockedSurface(props),
}));

import WorkspacePage from "../../app/(app)/dashboard/[workspace_id]/page";

describe("WorkspacePage", () => {
  it("forwards the workspace_id route param to the workspace surface", async () => {
    render(await WorkspacePage({ params: Promise.resolve({ workspace_id: "ws-1" }) }));

    expect(mockedSurface).toHaveBeenCalledWith({ workspaceId: "ws-1" });
  });
});
