const FIRST_TOUCH_KEY = "contextlm.first_touch";

export type FirstTouchAttribution = {
  utm_source?: string;
  utm_medium?: string;
  utm_campaign?: string;
  utm_content?: string;
  utm_term?: string;
  referrer?: string;
  landing_path?: string;
};

type StoredFirstTouch = FirstTouchAttribution & { captured_at?: string };

const UTM_KEYS = ["utm_source", "utm_medium", "utm_campaign", "utm_content", "utm_term"] as const;

type SearchParamsLike = Pick<URLSearchParams, "get">;

function trimToUndefined(value: string | null): string | undefined {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}

function referrerWithoutQuery(): string | undefined {
  if (typeof document === "undefined" || !document.referrer) {
    return undefined;
  }
  try {
    const url = new URL(document.referrer);
    return url.origin + url.pathname;
  } catch {
    return undefined;
  }
}

function readStored(): StoredFirstTouch | null {
  if (typeof window === "undefined") {
    return null;
  }
  try {
    const raw = window.sessionStorage.getItem(FIRST_TOUCH_KEY);
    if (!raw) {
      return null;
    }
    const parsed = JSON.parse(raw) as StoredFirstTouch;
    return parsed && typeof parsed === "object" ? parsed : null;
  } catch {
    return null;
  }
}

export function captureFirstTouch(searchParams: SearchParamsLike): void {
  if (typeof window === "undefined") {
    return;
  }
  const hasUtm = UTM_KEYS.some((key) => trimToUndefined(searchParams.get(key)));
  if (!hasUtm) {
    return;
  }
  try {
    if (readStored()) {
      return;
    }
    const attribution: StoredFirstTouch = {};
    for (const key of UTM_KEYS) {
      const value = trimToUndefined(searchParams.get(key));
      if (value) {
        attribution[key] = value;
      }
    }
    const referrer = referrerWithoutQuery();
    if (referrer) {
      attribution.referrer = referrer;
    }
    attribution.landing_path = window.location.pathname;
    attribution.captured_at = new Date().toISOString();
    window.sessionStorage.setItem(FIRST_TOUCH_KEY, JSON.stringify(attribution));
  } catch {
    // sessionStorage unavailable (privacy mode, quota) — attribution skipped.
  }
}

export function readFirstTouch(): FirstTouchAttribution | null {
  return readStored();
}

export function resolveAttribution(searchParams: SearchParamsLike): FirstTouchAttribution | null {
  const fromUrl: FirstTouchAttribution = {};
  for (const key of UTM_KEYS) {
    const value = trimToUndefined(searchParams.get(key));
    if (value) {
      fromUrl[key] = value;
    }
  }
  if (Object.keys(fromUrl).length > 0) {
    return fromUrl;
  }
  return readStored();
}

export function buildAuthHref(path: string, searchParams: SearchParamsLike): string {
  const params = new URLSearchParams();
  const next = trimToUndefined(searchParams.get("next"));
  if (next) {
    params.set("next", next);
  }
  const ref = trimToUndefined(searchParams.get("ref") ?? searchParams.get("referral"));
  if (ref) {
    params.set("ref", ref);
  }
  for (const key of UTM_KEYS) {
    const value = trimToUndefined(searchParams.get(key));
    if (value) {
      params.set(key, value);
    }
  }
  const query = params.toString();
  return query ? `${path}?${query}` : path;
}
