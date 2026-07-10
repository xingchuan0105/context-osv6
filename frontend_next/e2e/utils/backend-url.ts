import fs from "fs";
import path from "path";

let cached: string | null = null;

export function defaultAuthHeaders(): Record<string, string> {
  return {
    "x-owner-user-id": "00000000-0000-0000-0000-000000000001",
    "x-user-id": "00000000-0000-0000-0000-000000000001",
    "x-permissions": "external_network",
  };
}

export function getBackendBaseUrl(): string {
  if (cached) return cached;

  const candidate = process.env.BACKEND_BASE_URL;
  if (candidate) {
    cached = candidate;
    return cached;
  }

  const urlFile = path.resolve(__dirname, "../../../output/.backend.url");
  if (fs.existsSync(urlFile)) {
    cached = fs.readFileSync(urlFile, "utf-8").trim();
    if (cached) return cached;
  }

  return "http://127.0.0.1:8080";
}
