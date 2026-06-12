import type { UiMessageDescriptor } from "./types";

export const gateMessages = {
  gateCheckingSession: {
    zh: "正在检查登录状态...",
    en: "Checking your session...",
  },
  gateInitializingAuth: {
    zh: "正在初始化认证状态...",
    en: "Initializing authentication...",
  },
  gateRedirectingDashboard: {
    zh: "正在跳转到工作台...",
    en: "Redirecting to the dashboard...",
  },
  gateRedirectingLogin: {
    zh: "正在跳转到登录页...",
    en: "Redirecting to sign in...",
  },
} satisfies Record<string, UiMessageDescriptor>;
