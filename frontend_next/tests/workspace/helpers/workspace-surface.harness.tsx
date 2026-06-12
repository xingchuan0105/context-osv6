import { render } from "@testing-library/react";

import { WorkspaceSurface } from "../../../components/workspace/workspace-surface";
import { QueryProvider } from "../../../lib/query/provider";

export function renderWorkspaceSurface(workspaceId = "ws-1") {
  return render(
    <QueryProvider>
      <WorkspaceSurface workspaceId={workspaceId} />
    </QueryProvider>,
  );
}
