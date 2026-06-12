import type { UiLocale } from "../config";
import type { UiMessageDescriptor } from "./types";
import { adminMessages } from "./admin";
import { authMessages } from "./auth";
import { commonMessages } from "./common";
import { dashboardMessages } from "./dashboard";
import { gateMessages } from "./gate";
import { helpMessages } from "./help";
import { homeMessages } from "./home";
import { paywallMessages } from "./paywall";
import { pricingMessages } from "./pricing";
import { settingsMessages } from "./settings";
import { shareMessages } from "./share";
import { upgradeMessages } from "./upgrade";
import { usageMessages } from "./usage";
import { workspaceMessages } from "./workspace";

export type { UiMessageDescriptor };

export const UI_MESSAGES = {
  ...adminMessages,
  ...authMessages,
  ...commonMessages,
  ...dashboardMessages,
  ...gateMessages,
  ...helpMessages,
  ...homeMessages,
  ...paywallMessages,
  ...pricingMessages,
  ...settingsMessages,
  ...shareMessages,
  ...upgradeMessages,
  ...usageMessages,
  ...workspaceMessages,
} satisfies Record<string, UiMessageDescriptor>;

export type UiMessageKey = keyof typeof UI_MESSAGES;

type UiMessageCatalog = {
  [key: string]: string | UiMessageCatalog;
};

function insertCatalogValue(catalog: UiMessageCatalog, key: string, value: string) {
  if (!key.includes(".")) {
    catalog[key] = value;
    return;
  }

  const segments = key.split(".");
  let cursor = catalog;

  for (const segment of segments.slice(0, -1)) {
    const current = cursor[segment];

    if (!current || typeof current === "string") {
      cursor[segment] = {};
    }

    cursor = cursor[segment] as UiMessageCatalog;
  }

  cursor[segments[segments.length - 1]!] = value;
}

function buildLocaleCatalog(locale: UiLocale): UiMessageCatalog {
  const catalog: UiMessageCatalog = {};

  for (const [key, descriptor] of Object.entries(UI_MESSAGES)) {
    insertCatalogValue(catalog, key, locale === "zh-CN" ? descriptor.zh : descriptor.en);
  }

  return catalog;
}

const MESSAGE_CATALOG_BY_LOCALE: Record<UiLocale, UiMessageCatalog> = {
  "zh-CN": buildLocaleCatalog("zh-CN"),
  en: buildLocaleCatalog("en"),
};

export function getMessageCatalog(locale: UiLocale) {
  return MESSAGE_CATALOG_BY_LOCALE[locale];
}

function interpolate(template: string, values?: Record<string, string | number>) {
  if (!values) {
    return template;
  }

  return template.replace(/\{(\w+)\}/g, (_match, key: string) => String(values[key] ?? ""));
}

export function formatUiMessage(
  locale: UiLocale,
  key: UiMessageKey,
  values?: Record<string, string | number>,
) {
  const descriptor = UI_MESSAGES[key];
  const template = locale === "zh-CN" ? descriptor.zh : descriptor.en;

  return interpolate(template, values);
}
