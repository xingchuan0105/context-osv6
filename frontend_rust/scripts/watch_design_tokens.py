#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
TOKENS = ROOT / "crates" / "web-ui" / "src" / "styles" / "design_tokens.css"
SYNC_SCRIPT = ROOT / "scripts" / "sync_design_tokens.py"
POLL_INTERVAL_SECONDS = 0.5


def run_sync() -> None:
    result = subprocess.run([sys.executable, str(SYNC_SCRIPT)], cwd=ROOT)
    if result.returncode != 0:
        raise SystemExit(result.returncode)


def main() -> None:
    run_sync()
    last_mtime = TOKENS.stat().st_mtime_ns
    while True:
        time.sleep(POLL_INTERVAL_SECONDS)
        current_mtime = TOKENS.stat().st_mtime_ns
        if current_mtime == last_mtime:
            continue
        last_mtime = current_mtime
        run_sync()


if __name__ == "__main__":
    main()
