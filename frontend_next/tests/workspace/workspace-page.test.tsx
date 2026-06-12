import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";


vi.mock("../../components/workspace/workspace-surface", () => ({
  WorkspaceSurface: (props: { workspaceId: string }) => mockedSurface(props),
}));

import WorkspacePage from "../../app/(app)/dashboard/[workspace_id]/page";

const mockedSurface = vi.hoisted(() => globalThis.__mockProviders.createComponentMock());



describe("WorkspacePage", () => {
  it("forwards the workspace_id route param to the workspace surface", async () => {
    render(await WorkspacePage({ params: Promise.resolve({ workspace_id: "ws-1" }) }));

    expect(mockedSurface).toHaveBeenCalledWith({ workspaceId: "ws-1" });
  });
});
