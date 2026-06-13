import type { UiMessageDescriptor } from "./types";

export const legalMessages = {
  legalReacceptanceTitle: {
    zh: "协议已更新",
    en: "Terms updated",
  },
  legalReacceptanceBody: {
    zh: "我们更新了用户服务协议或隐私政策。继续使用前，请阅读并确认最新版本。",
    en: "We updated the Terms of Service or Privacy Policy. Read and accept the latest versions before continuing.",
  },
  legalReacceptanceSubmitting: {
    zh: "提交中...",
    en: "Submitting...",
  },
  legalReacceptanceConfirm: {
    zh: "确认并继续",
    en: "Confirm and continue",
  },
  legalReacceptanceConsentRequired: {
    zh: "请先阅读并同意最新版用户协议与隐私政策",
    en: "Read and accept the latest Terms of Service and Privacy Policy first",
  },
  legalPaymentConsentRequired: {
    zh: "请先阅读并同意用户协议与隐私政策",
    en: "Read and accept the Terms of Service and Privacy Policy first",
  },
} satisfies Record<string, UiMessageDescriptor>;
