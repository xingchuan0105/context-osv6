#!/usr/bin/env python3
"""Sync design tokens into index.css.

The canonical token source is:
  crates/web-ui/src/styles/design_tokens.css

The generated section in index.css is wrapped by:
  /* DESIGN_TOKENS_START */
  /* DESIGN_TOKENS_END */
"""

from __future__ import annotations

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
TOKENS_PATH = ROOT / "crates/web-ui/src/styles/design_tokens.css"
INDEX_PATH = ROOT / "crates/web-ui/src/index.css"

START = "/* DESIGN_TOKENS_START */"
END = "/* DESIGN_TOKENS_END */"


def main() -> int:
    tokens = TOKENS_PATH.read_text(encoding="utf-8").rstrip() + "\n"
    index_text = INDEX_PATH.read_text(encoding="utf-8")

    section = f"{START}\n{tokens}{END}"

    if START in index_text and END in index_text:
        before, remainder = index_text.split(START, 1)
        _, after = remainder.split(END, 1)
        next_text = f"{before}{section}{after}"
    else:
        anchor = "@tailwind utilities;\n\n"
        if anchor not in index_text:
            print("ERROR: Could not find tailwind header anchor in index.css", file=sys.stderr)
            return 2
        next_text = index_text.replace(anchor, f"{anchor}{section}\n\n", 1)

    if next_text != index_text:
        INDEX_PATH.write_text(next_text, encoding="utf-8")
        print("Synced design tokens into index.css")
    else:
        print("Design tokens already in sync")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
