const UNSAFE_PROTOCOL = /^(javascript|data|vbscript|blob|file):/i;
const SAFE_PROTOCOL = /^(https?:|mailto:)/i;

export function isSafeHttpUrl(value: string | null | undefined): boolean {
  if (typeof value !== "string") {
    return false;
  }

  const trimmed = value.trim();
  if (!trimmed) {
    return false;
  }

  if (UNSAFE_PROTOCOL.test(trimmed)) {
    return false;
  }

  return SAFE_PROTOCOL.test(trimmed);
}

export function toSafeHttpUrl(value: string | null | undefined): string | null {
  if (!isSafeHttpUrl(value)) {
    return null;
  }

  return value!.trim();
}
