import type * as MockProviders from "./mock-providers";

declare global {
  var __mockProviders: typeof MockProviders;
}

export {};
