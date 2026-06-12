import type * as MockProviders from "./mock-providers";

declare global {
  var __mockProviders: typeof MockProviders;
  var __workspaceChatPaneHarnessMocks: ReturnType<
    typeof MockProviders.createWorkspaceChatPaneMocks
  >;
}

export {};
