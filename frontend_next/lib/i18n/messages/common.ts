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
  "productChrome.footerNavLabel": {
    zh: "产品与法律链接",
    en: "Product and legal links",
  },
  "productChrome.brandHome": {
    zh: "品牌官网",
    en: "Brand site",
  },
  "productChrome.productHome": {
    zh: "工作台",
    en: "App home",
  },
  "productChrome.help": {
    zh: "产品帮助",
    en: "Help",
  },
  "productChrome.pricing": {
    zh: "定价",
    en: "Pricing",
  },
  "productChrome.legalCenter": {
    zh: "法律中心",
    en: "Legal center",
  },
  "productChrome.terms": {
    zh: "用户协议",
    en: "Terms",
  },
  "productChrome.privacy": {
    zh: "隐私政策",
    en: "Privacy",
  },
  "productChrome.licenses": {
    zh: "开源声明",
    en: "Open source",
  },
} satisfies Record<string, UiMessageDescriptor>;
