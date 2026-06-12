#!/usr/bin/env python3
"""Add typeshare serialized_as hints for integer fields typeshare 1.13 rejects."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "contracts" / "src"

NUMBER = re.compile(
    r"^(?P<prefix>\s*)(?P<attr>(?:#\[[^\]]+\]\s*)*)"
    r"(?P<vis>pub\s+)?(?P<name>\w+):\s*"
    r"(?P<type>(?:Option<)?(?:i64|u64|usize|isize)(?:>)?)\s*,?\s*$"
)
HASHMAP_I64 = re.compile(
    r"^(?P<prefix>\s*)(?P<attr>(?:#\[[^\]]+\]\s*)*)"
    r"(?P<vis>pub\s+)?(?P<name>\w+):\s*"
    r"(?:std::collections::)?HashMap<String,\s*i64>\s*,?\s*$"
)
BTREEMAP_I64 = re.compile(
    r"^(?P<prefix>\s*)(?P<attr>(?:#\[[^\]]+\]\s*)*)"
    r"(?P<vis>pub\s+)?(?P<name>\w+):\s*"
    r"(?:std::collections::)?BTreeMap<String,\s*i64>\s*,?\s*$"
)
BTREEMAP_USIZE = re.compile(
    r"^(?P<prefix>\s*)(?P<attr>(?:#\[[^\]]+\]\s*)*)"
    r"(?P<vis>pub\s+)?(?P<name>\w+):\s*"
    r"(?:std::collections::)?BTreeMap<String,\s*usize>\s*,?\s*$"
)
VEC_USIZE = re.compile(
    r"^(?P<prefix>\s*)(?P<attr>(?:#\[[^\]]+\]\s*)*)"
    r"(?P<vis>pub\s+)?(?P<name>\w+):\s*"
    r"Vec<usize>\s*,?\s*$"
)
ENUM_START = re.compile(r"^\s*pub\s+enum\s+\w+")


def has_serialized_as(attr: str) -> bool:
    return "serialized_as" in attr


def annotate_line(line: str, ts_type: str, prev_line: str = "") -> str:
    for pattern in (NUMBER, HASHMAP_I64, BTREEMAP_I64, BTREEMAP_USIZE, VEC_USIZE):
        match = pattern.match(line)
        if not match:
            continue
        if has_serialized_as(match.group("attr")):
            return line
        if "serialized_as" in prev_line:
            return line
        indent = match.group("prefix")
        return (
            f'{indent}#[typeshare(serialized_as = "{ts_type}")]\n'
            f"{indent}{match.group('attr')}"
            f"{match.group('vis') or ''}{match.group('name')}: "
            f"{line.split(':', 1)[1].rstrip()}\n"
        )
    return line


def dedupe_typeshare_attrs(text: str) -> str:
    """Remove consecutive duplicate typeshare(serialized_as) attributes."""
    pattern = r'(    #\[typeshare\(serialized_as = "[^"]+"\)\]\n)+'
    return re.sub(
        pattern,
        lambda m: m.group(0).split("\n", 1)[0] + "\n",
        text,
    )


def process_file(path: Path) -> bool:
    original = path.read_text()
    lines = original.splitlines(keepends=True)
    changed = False
    out: list[str] = []
    enum_depth = 0

    for i, line in enumerate(lines):
        stripped = line.rstrip("\n")
        prev = lines[i - 1].rstrip("\n") if i > 0 else ""

        if enum_depth == 0 and ENUM_START.search(stripped):
            enum_depth = stripped.count("{") - stripped.count("}")
            if enum_depth <= 0 and "{" in stripped:
                enum_depth = 1
            out.append(line)
            continue

        if enum_depth > 0:
            enum_depth += stripped.count("{") - stripped.count("}")
            out.append(line)
            continue

        if "HashMap<String, i64>" in stripped or "HashMap<String,i64>" in stripped:
            new_line = annotate_line(stripped, "Record<string, number>", prev)
        elif "BTreeMap<String, i64>" in stripped:
            new_line = annotate_line(stripped, "Record<string, number>", prev)
        elif "BTreeMap<String, usize>" in stripped:
            new_line = annotate_line(stripped, "Record<string, number>", prev)
        elif "Vec<usize>" in stripped:
            new_line = annotate_line(stripped, "number[]", prev)
        elif re.search(r":\s*(?:Option<)?(?:i64|u64|usize|isize)(?:>)?\s*,?\s*$", stripped):
            new_line = annotate_line(stripped, "number", prev)
        else:
            new_line = stripped

        if new_line != stripped:
            changed = True
            if not new_line.endswith("\n"):
                new_line += "\n"
            out.append(new_line)
        else:
            out.append(line)

    if changed:
        path.write_text(dedupe_typeshare_attrs("".join(out)))
    return changed


def main() -> None:
    updated = []
    for path in sorted(ROOT.glob("*.rs")):
        text = dedupe_typeshare_attrs(path.read_text())
        if text != path.read_text():
            path.write_text(text)
            updated.append(path.name)
    for path in sorted(ROOT.glob("*.rs")):
        if process_file(path):
            updated.append(path.name)
    print("annotated:", ", ".join(sorted(set(updated))) if updated else "(none)")


if __name__ == "__main__":
    main()
