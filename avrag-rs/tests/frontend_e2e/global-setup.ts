import { spawn, ChildProcess } from "child_process";
import fs from "fs";
import path from "path";

const URL_FILE = "/tmp/e2e-backend.url";
const BACKEND_TIMEOUT_MS = 180_000;
const FRONTEND_TIMEOUT_MS = 60_000;

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

function spawnBackend(avragRoot: string): ChildProcess {
  // Remove stale url file so we don't read a leftover from a previous run.
  try { fs.unlinkSync(URL_FILE); } catch {}

  const proc = spawn(
    "cargo",
    [
      "test", "-p", "app", "--test", "product_e2e",
      "smoke::backend_launcher",
      "--", "--ignored", "--nocapture",
    ],
    { cwd: avragRoot, stdio: "pipe" }
  );

  proc.stdout?.on("data", (d) => {
    const line = d.toString();
    if (line.includes("[backend_launcher]")) {
      process.stdout.write(line);
    }
  });
  proc.stderr?.on("data", (d) => {
    // Only forward backend-launcher lines to keep output clean.
    const line = d.toString();
    if (line.includes("[backend_launcher]") || line.includes("error")) {
      process.stderr.write(line);
    }
  });

  return proc;
}

function spawnFrontend(frontendRoot: string, apiProxyTarget: string): ChildProcess {
  const env = { ...process.env, API_PROXY_TARGET: apiProxyTarget };
  const proc = spawn("pnpm", ["start"], {
    cwd: frontendRoot,
    stdio: "pipe",
    env,
  });

  proc.stdout?.on("data", (d) => {
    const line = d.toString();
    if (line.includes("Ready") || line.includes("error")) {
      process.stdout.write(`[frontend] ${line}`);
    }
  });
  proc.stderr?.on("data", (d) => {
    process.stderr.write(`[frontend] ${d}`);
  });

  return proc;
}

async function waitForFile(filePath: string, timeoutMs: number): Promise<string> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (fs.existsSync(filePath)) {
      const content = fs.readFileSync(filePath, "utf-8").trim();
      if (content) return content;
    }
    await sleep(1000);
  }
  throw new Error(`Timed out waiting for ${filePath} after ${timeoutMs}ms`);
}

async function waitForUrl(url: string, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(url);
      if (res.status === 200) return;
    } catch {}
    await sleep(1000);
  }
  throw new Error(`Frontend did not become ready at ${url} within ${timeoutMs}ms`);
}

function buildFrontend(frontendRoot: string): Promise<void> {
  // Skip build if .next/standalone exists and is recent (< 1 hour)
  const standaloneDir = path.join(frontendRoot, ".next", "standalone");
  if (fs.existsSync(standaloneDir)) {
    const stat = fs.statSync(standaloneDir);
    const ageMs = Date.now() - stat.mtimeMs;
    if (ageMs < 3_600_000) {
      console.log("[setup] Reusing existing Next.js build (< 1h old)");
      return Promise.resolve();
    }
  }

  console.log("[setup] Building Next.js frontend (this may take 30-60s)...");
  return new Promise((resolve, reject) => {
    const proc = spawn("pnpm", ["build"], {
      cwd: frontendRoot,
      stdio: "inherit",
    });
    proc.on("close", (code) => {
      code === 0 ? resolve() : reject(new Error(`Next.js build exited with ${code}`));
    });
  });
}

// Store handles for teardown
let backendProc: ChildProcess | null = null;
let frontendProc: ChildProcess | null = null;

export default async function globalSetup() {
  const e2eDir = __dirname;
  const avragRoot = path.resolve(e2eDir, "../..");
  const frontendRoot = path.resolve(avragRoot, "../../frontend_next");

  console.log("[setup] Starting Rust backend...");
  backendProc = spawnBackend(avragRoot);

  const backendUrl = await waitForFile(URL_FILE, BACKEND_TIMEOUT_MS);
  console.log(`[setup] Backend ready at ${backendUrl}`);

  // Build frontend if needed
  await buildFrontend(frontendRoot);

  console.log("[setup] Starting Next.js frontend...");
  frontendProc = spawnFrontend(frontendRoot, backendUrl);

  await waitForUrl("http://localhost:3000", FRONTEND_TIMEOUT_MS);
  console.log("[setup] Frontend ready at http://localhost:3000");

  // Persist handles for teardown via a sidecar file
  fs.writeFileSync(
    path.join(e2eDir, "output", ".pids.json"),
    JSON.stringify({ backendPid: backendProc.pid, frontendPid: frontendProc.pid })
  );
}
