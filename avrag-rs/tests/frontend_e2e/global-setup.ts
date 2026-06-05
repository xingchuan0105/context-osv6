import { spawn, ChildProcess } from "child_process";
import fs from "fs";
import path from "path";

const URL_FILE = "/tmp/e2e-backend.url";
const BACKEND_TIMEOUT_MS = 180_000;
const FRONTEND_TIMEOUT_MS = 60_000;

// Resolve absolute paths because `spawn` does not see the shell PATH
// when launched from npx in some environments.
const NODE_BIN = process.env.NODE_BIN || process.execPath;
const PNPM_JS = process.env.PNPM_JS || "/home/chuan/.nvm/versions/node/v24.13.0/lib/node_modules/corepack/dist/pnpm.js";

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

function killProcessOnPort(port: number): void {
  try {
    const { spawnSync } = require("child_process");
    const result = spawnSync("ss", ["-tlnp"], { encoding: "utf-8" });
    if (result.status !== 0) return;
    const lines = result.stdout.split("\n");
    const pids = new Set<number>();
    for (const line of lines) {
      if (!line.includes(`:${port}`)) continue;
      const match = line.match(/pid=(\d+)/);
      if (match) pids.add(Number(match[1]));
    }
    for (const pid of pids) {
      try {
        process.kill(pid, "SIGKILL");
        console.log(`[setup] killed previous process on port ${port}: ${pid}`);
      } catch {}
    }
  } catch {}
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

function spawnDevFrontend(frontendRoot: string, apiProxyTarget: string): ChildProcess {
  const env = { ...process.env, API_PROXY_TARGET: apiProxyTarget };
  const proc = spawn(NODE_BIN, [PNPM_JS, "dev", "--port", "3001"], {
    cwd: frontendRoot,
    stdio: "pipe",
    env,
  });

  proc.stdout?.on("data", (d) => {
    const line = d.toString();
    if (line.includes("Ready") || line.includes("ready") || line.includes("error")) {
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

function installFrontend(frontendRoot: string): Promise<void> {
  if (fs.existsSync(path.join(frontendRoot, "node_modules"))) {
    return Promise.resolve();
  }
  console.log("[setup] Installing frontend dependencies...");
  return new Promise((resolve, reject) => {
    const proc = spawn(NODE_BIN, [PNPM_JS, "install"], {
      cwd: frontendRoot,
      stdio: "inherit",
    });
    proc.on("close", (code) => {
      code === 0 ? resolve() : reject(new Error(`pnpm install exited with ${code}`));
    });
  });
}

// Store handles for teardown
let backendProc: ChildProcess | null = null;
let frontendProc: ChildProcess | null = null;

export default async function globalSetup() {
  // Best-effort cleanup of stale frontend process from a previous run
  killProcessOnPort(3001);
  await sleep(500);

  const e2eDir = __dirname;
  const avragRoot = path.resolve(e2eDir, "../..");
  // In the worktree: e2e-analyzer/avrag-rs/ and e2e-analyzer/frontend_next/
  // are siblings, so we go up ONE level from avrag-rs/.
  const frontendRoot = path.resolve(avragRoot, "../frontend_next");

  console.log("[setup] Starting Rust backend...");
  backendProc = spawnBackend(avragRoot);

  const backendUrl = await waitForFile(URL_FILE, BACKEND_TIMEOUT_MS);
  console.log(`[setup] Backend ready at ${backendUrl}`);

  // Ensure frontend dependencies are installed
  await installFrontend(frontendRoot);

  // NOTE: production build is preferred, but the worktree's Next.js 16
  // apple-icon route fails during static generation. Use dev mode as a
  // pragmatic fallback so E2E can exercise runtime behavior while the
  // frontend build bug is fixed separately.
  console.log("[setup] Starting Next.js frontend (dev mode on port 3001)...");
  frontendProc = spawnDevFrontend(frontendRoot, backendUrl);

  await waitForUrl("http://localhost:3001", FRONTEND_TIMEOUT_MS);
  console.log("[setup] Frontend ready at http://localhost:3001");

  // Persist handles for teardown via a sidecar file
  fs.writeFileSync(
    path.join(e2eDir, "output", ".pids.json"),
    JSON.stringify({ backendPid: backendProc.pid, frontendPid: frontendProc.pid })
  );

  // Persist backend URL so tests can hit the API directly
  fs.writeFileSync(
    path.join(e2eDir, "output", ".backend.url"),
    backendUrl
  );

  // Also expose via env for the current Playwright process children
  process.env.BACKEND_BASE_URL = backendUrl;
}
