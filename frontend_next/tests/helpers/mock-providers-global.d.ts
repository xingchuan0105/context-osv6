import type * as MockProviders from "./mock-providers";

declare global {
  var __mockProviders: typeof MockProviders;
  var __workspaceChatPaneHarnessMocks: ReturnType<
    typeof MockProviders.createWorkspaceChatPaneMocks
  >;
  var __workspaceSurfaceHarnessMocks: ReturnType<
    typeof MockProviders.createWorkspaceSurfaceMocks
  >;
}

export {};
