import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";
import * as mockProviders from "./helpers/mock-providers";

Object.assign(globalThis, { __mockProviders: mockProviders });

afterEach(() => {
  cleanup();
});
