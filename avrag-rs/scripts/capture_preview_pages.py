#!/usr/bin/env python3
"""
Capture fixed preview routes as deterministic Playwright screenshots.
"""

from __future__ import annotations

import json
import os
from pathlib import Path

from playwright.sync_api import sync_playwright


def main() -> int:
    script_dir = Path(__file__).resolve().parent
    repo_root = script_dir.parent
    frontend_run_root = (
        repo_root.parent / "frontend_rust" / ".run" / "visual_compare"
    )
    base_url = os.environ.get("PARITY_BASE_URL", "http://127.0.0.1:4173")
    out_dir = Path(
        os.environ.get("PARITY_PLAYWRIGHT_DIR", str(frontend_run_root / "playwright"))
    ).resolve()
    out_dir.mkdir(parents=True, exist_ok=True)

    targets = [
        ("login", "/preview/login"),
        ("dashboard", "/preview/dashboard"),
        ("workspace", "/preview/workspace"),
        ("account", "/preview/account"),
        ("settings", "/preview/settings"),
        ("help", "/preview/help"),
    ]

    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page(viewport={"width": 1440, "height": 1024})
        for name, route in targets:
            page.goto(f"{base_url}{route}", wait_until="networkidle", timeout=30000)
            page.add_style_tag(
                content="""
                *, *::before, *::after {
                  transition-duration: 0s !important;
                  transition-delay: 0s !important;
                  animation-duration: 0s !important;
                  animation-delay: 0s !important;
                  caret-color: transparent !important;
                }
                """
            )
            page.wait_for_timeout(100)
            page.screenshot(path=str(out_dir / f"{name}.png"), full_page=False)
        browser.close()

    print(
        json.dumps(
            {
                "baseUrl": base_url,
                "outDir": out_dir.as_posix(),
                "captured": [f"{name}.png" for name, _ in targets],
            },
            ensure_ascii=False,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
