import fs from "fs";
import path from "path";

export default async function globalTeardown() {
  const e2eDir = __dirname;
  const pidFile = path.join(e2eDir, "output", ".pids.json");

  if (fs.existsSync(pidFile)) {
    try {
      const pids = JSON.parse(fs.readFileSync(pidFile, "utf-8"));
      if (pids.frontendPid) {
        process.kill(pids.frontendPid, "SIGTERM");
        console.log(`[teardown] Sent SIGTERM to frontend (${pids.frontendPid})`);
      }
      if (pids.backendPid) {
        process.kill(pids.backendPid, "SIGKILL");
        console.log(`[teardown] Sent SIGKILL to backend (${pids.backendPid})`);
      }
    } catch (e) {
      console.error("[teardown] Failed to kill processes:", e);
    }
    fs.unlinkSync(pidFile);
  }

  // Clean up stale backend url file
  try { fs.unlinkSync("/tmp/e2e-backend.url"); } catch {}
}
