#!/usr/bin/env python3
"""UI drift guard checks for frontend_rust.

Default mode reports findings without failing.
Use --strict to fail on violations.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import argparse
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "crates/web-ui/src"
LAYOUT_DIR = SRC / "components/layout"
ALLOWLIST_PATH = ROOT / "scripts/ui_drift_allowlist.txt"

ALLOW_INLINE_STYLE = {
    SRC / "routes/preview.rs",  # pixel-mapping prototype file
}

ALLOW_HEX_IN_RS = {
    SRC / "routes/preview.rs",  # pixel-mapping prototype file
}

INLINE_STYLE_RE = re.compile(r"\bstyle\s*=")
HEX_COLOR_RE = re.compile(r"#[0-9A-Fa-f]{3,8}\b")
SIGNAL_RE = re.compile(r"\b(create_signal|signal\(|RwSignal::new|Signal::derive)")


@dataclass
class Violation:
    rule: str
    path: Path
    line: int
    snippet: str


def rel(path: Path) -> str:
    return str(path.relative_to(ROOT)).replace("\\", "/")


def load_allowlist() -> set[tuple[str, str]]:
    if not ALLOWLIST_PATH.exists():
        return set()
    entries: set[tuple[str, str]] = set()
    for raw in ALLOWLIST_PATH.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if ":" not in line:
            continue
        rule, path = line.split(":", 1)
        entries.add((rule.strip(), path.strip()))
    return entries


def iter_files(base: Path, pattern: str):
    if not base.exists():
        return []
    return sorted(base.rglob(pattern))


def line_violations(path: Path, regex: re.Pattern[str], rule: str) -> list[Violation]:
    findings: list[Violation] = []
    text = path.read_text(encoding="utf-8", errors="replace")
    for idx, line in enumerate(text.splitlines(), start=1):
        if regex.search(line):
            findings.append(Violation(rule=rule, path=path, line=idx, snippet=line.strip()))
    return findings


def collect_violations() -> list[Violation]:
    findings: list[Violation] = []

    for rs_file in iter_files(SRC, "*.rs"):
        if rs_file not in ALLOW_INLINE_STYLE:
            findings.extend(line_violations(rs_file, INLINE_STYLE_RE, "inline-style-disallowed"))
        if rs_file not in ALLOW_HEX_IN_RS:
            findings.extend(line_violations(rs_file, HEX_COLOR_RE, "hex-color-in-rs-disallowed"))

    if LAYOUT_DIR.exists():
        for rs_file in iter_files(LAYOUT_DIR, "*.rs"):
            findings.extend(line_violations(rs_file, SIGNAL_RE, "layout-contains-signal"))

    return findings


def main() -> int:
    parser = argparse.ArgumentParser(description="Check UI drift guard rules.")
    parser.add_argument("--strict", action="store_true", help="Exit non-zero on violations.")
    args = parser.parse_args()

    allowlist = load_allowlist()
    findings = [
        f
        for f in collect_violations()
        if (f.rule, rel(f.path)) not in allowlist
    ]
    if not findings:
        print("UI drift guard: clean")
        return 0

    print(f"UI drift guard: {len(findings)} finding(s)")
    grouped: dict[str, list[Violation]] = {}
    for item in findings:
        grouped.setdefault(item.rule, []).append(item)

    for rule, items in sorted(grouped.items(), key=lambda kv: kv[0]):
        print(f"\n[{rule}] {len(items)}")
        for v in items[:20]:
            print(f"- {rel(v.path)}:{v.line}  {v.snippet}")
        if len(items) > 20:
            print(f"- ... and {len(items) - 20} more")

    if args.strict:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
