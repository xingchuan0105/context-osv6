import type { UiMessageDescriptor } from "./types";

export const legalMessages = {
  legalConsentPrefix: {
    zh: "我已阅读并同意",
    en: "I have read and agree to the ",
  },
  legalConsentTerms: {
    zh: "《用户服务协议》",
    en: "Terms of Service",
  },
  legalConsentAnd: {
    zh: "与",
    en: " and ",
  },
  legalConsentPrivacy: {
    zh: "《隐私政策》",
    en: "Privacy Policy",
  },
  legalFooterTerms: {
    zh: "用户协议",
    en: "Terms",
  },
  legalFooterPrivacy: {
    zh: "隐私政策",
    en: "Privacy",
  },
  legalFooterLicenses: {
    zh: "开源声明",
    en: "Open source",
  },
  legalBackToCenter: {
    zh: "返回法律中心",
    en: "Back to legal center",
  },
  legalLastUpdated: {
    zh: "最后更新：{date}",
    en: "Last updated: {date}",
  },
  legalVersion: {
    zh: "版本：{version}",
    en: "Version: {version}",
  },
  legalTocTitle: {
    zh: "目录",
    en: "Contents",
  },
  legalTocAria: {
    zh: "文档目录",
    en: "Document contents",
  },
  legalReacceptanceTitle: {
    zh: "协议已更新",
    en: "Terms updated",
  },
  legalReacceptanceBody: {
    zh: "我们更新了用户服务协议或隐私政策。继续使用前，请阅读并确认最新版本。",
    en: "We updated the Terms of Service or Privacy Policy. Read and accept the latest versions before continuing.",
  },
  legalReacceptanceSubmitting: {
    zh: "提交中…",
    en: "Submitting...",
  },
  legalReacceptanceConfirm: {
    zh: "确认并继续",
    en: "Confirm and continue",
  },
  legalReacceptanceConsentRequired: {
    zh: "请先阅读并同意最新版用户协议与隐私政策。",
    en: "Read and accept the latest Terms of Service and Privacy Policy first",
  },
} satisfies Record<string, UiMessageDescriptor>;
