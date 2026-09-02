import type { UiMessageDescriptor } from "./types";

export const gateMessages = {
  gateCheckingSession: {
    zh: "正在检查登录状态…",
    en: "Checking your session...",
  },
  gateRedirectingChat: {
    zh: "正在进入对话…",
    en: "Opening chat...",
  },
  gateRedirectingLogin: {
    zh: "正在跳转到登录页…",
    en: "Redirecting to sign in...",
  },
} satisfies Record<string, UiMessageDescriptor>;
