#!/usr/bin/env python3
"""Extract Next UI_MESSAGES {zh,en} descriptors into web-ui JSON catalog."""

from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
NEXT_DIR = ROOT / "frontend_next" / "lib" / "i18n" / "messages"
OUT_DIR = ROOT / "frontend_rust" / "crates" / "web-ui" / "i18n"
EXTRAS = OUT_DIR / "extras.json"
MERGED = OUT_DIR / "messages.json"

SKIP = {"index.ts", "types.ts"}

BLOCK = re.compile(
    r'(?:["\'](?P<qkey>[\w.-]+)["\']|(?P<ukey>[A-Za-z_][\w.]*))\s*:\s*\{\s*zh:\s*"(?P<zh>(?:\\.|[^"\\])*)"\s*,\s*en:\s*"(?P<en>(?:\\.|[^"\\])*)"',
    re.S,
)


def unescape(value: str) -> str:
    return json.loads(f'"{value}"')


def extract_file(path: Path) -> dict[str, dict[str, str]]:
    text = path.read_text(encoding="utf-8")
    out: dict[str, dict[str, str]] = {}
    for match in BLOCK.finditer(text):
        key = match.group("qkey") or match.group("ukey")
        out[key] = {
            "zh": unescape(match.group("zh")),
            "en": unescape(match.group("en")),
        }
    return out


def main() -> None:
    catalog: dict[str, dict[str, str]] = {}
    for path in sorted(NEXT_DIR.glob("*.ts")):
        if path.name in SKIP:
            continue
        extracted = extract_file(path)
        overlap = set(catalog) & set(extracted)
        if overlap:
            raise SystemExit(f"duplicate keys in {path.name}: {sorted(overlap)[:8]}")
        catalog.update(extracted)

    extras = json.loads(EXTRAS.read_text(encoding="utf-8"))
    catalog.update(extras)
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    MERGED.write_text(
        json.dumps(catalog, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(f"wrote {MERGED} ({len(catalog)} keys, {len(extras)} extras)")


if __name__ == "__main__":
    main()
