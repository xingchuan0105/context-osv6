import { execFileSync } from "node:child_process";

type BrowserSnapshotEntry = {
  processId: number;
  commandLine: string;
  mainWindowTitle: string;
};

export function snapshotExternalBrowserCandidates(): BrowserSnapshotEntry[] {
  const script = [
    "$ErrorActionPreference = 'SilentlyContinue'",
    "$cim = Get-CimInstance Win32_Process -Filter \"Name = 'msedge.exe' OR Name = 'chrome.exe'\"",
    "$proc = @{}",
    "Get-Process msedge,chrome -ErrorAction SilentlyContinue | ForEach-Object { $proc[[int]$_.Id] = [string]$_.MainWindowTitle }",
    "$rows = foreach ($item in $cim) { [pscustomobject]@{ processId = [int]$item.ProcessId; commandLine = [string]$item.CommandLine; mainWindowTitle = [string]$proc[[int]$item.ProcessId] } }",
    "ConvertTo-Json -Compress -InputObject @($rows)",
  ].join("; ");
  const raw = execFileSync("powershell.exe", ["-NoProfile", "-Command", script], {
    encoding: "utf8",
    windowsHide: true,
  }).trim();
  if (!raw) {
    return [];
  }
  const parsed = JSON.parse(raw) as BrowserSnapshotEntry[] | BrowserSnapshotEntry | null;
  if (!parsed) {
    return [];
  }
  return Array.isArray(parsed) ? parsed : [parsed];
}

export function expectNoTauriExternalBrowser(
  before: BrowserSnapshotEntry[],
  after: BrowserSnapshotEntry[],
) {
  const beforeKeys = new Set(before.map(entryKey));
  const offenders = after.filter(
    (entry) =>
      !beforeKeys.has(entryKey(entry)) &&
      (/tauri\.localhost/i.test(entry.commandLine) || /tauri\.localhost/i.test(entry.mainWindowTitle)),
  );
  if (offenders.length > 0) {
    throw new Error(
      `new external browser process targets tauri.localhost: ${offenders
        .map((entry) => `${entry.processId}:${entry.commandLine}`)
        .join(" | ")}`,
    );
  }
}

function entryKey(entry: BrowserSnapshotEntry) {
  return `${entry.processId}|${entry.commandLine}|${entry.mainWindowTitle}`;
}
