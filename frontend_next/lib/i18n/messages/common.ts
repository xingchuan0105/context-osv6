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
} satisfies Record<string, UiMessageDescriptor>;
