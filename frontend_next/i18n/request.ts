import { cookies } from "next/headers";
import { getRequestConfig } from "next-intl/server";

import { DEFAULT_LOCALE, LOCALE_COOKIE_NAME, normalizeLocale } from "../lib/i18n/config";
import { getMessageCatalog } from "../lib/i18n/messages";

export default getRequestConfig(async ({ locale, requestLocale }) => {
  const cookieStore = await cookies();
  const cookieLocale = cookieStore.get(LOCALE_COOKIE_NAME)?.value;
  const resolvedRequestLocale = await requestLocale;
  const resolvedLocale = normalizeLocale(locale ?? resolvedRequestLocale ?? cookieLocale ?? DEFAULT_LOCALE);

  return {
    locale: resolvedLocale,
    messages: getMessageCatalog(resolvedLocale),
  };
});
