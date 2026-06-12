import type { UiMessageDescriptor } from "./types";

export const commonMessages = {
  commonCancel: {
    zh: "取消",
    en: "Cancel",
  },
  commonSave: {
    zh: "保存",
    en: "Save",
  },
  commonUnlimited: {
    zh: "不限",
    en: "Unlimited",
  },
} satisfies Record<string, UiMessageDescriptor>;
