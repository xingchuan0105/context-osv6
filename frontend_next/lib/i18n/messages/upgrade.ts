import type { UiMessageDescriptor } from "./types";

export const upgradeMessages = {
  upgradeButton: {
    zh: "升级 Plus",
    en: "Upgrade Plus",
  },
  upgradeContinueFree: {
    zh: "继续 Free",
    en: "Continue Free",
  },
  upgradeSuccessTitle: {
    zh: "升级成功",
    en: "Upgrade successful",
  },
  upgradeSuccessSubtitle: {
    zh: "新档位已立即生效，祝你用得开心。",
    en: "Your new plan is active. Enjoy!",
  },
  upgradeSuccessBack: {
    zh: "返回工作区",
    en: "Back to workspace",
  },
  upgradeSuccessViewUsage: {
    zh: "查看用量",
    en: "View usage",
  },
} satisfies Record<string, UiMessageDescriptor>;
