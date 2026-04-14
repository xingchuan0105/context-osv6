#!/usr/bin/env python3
"""
Generate a markdown report from Playwright screenshot diff artifacts.

The script scans a test-results directory for files ending with:
  -actual.png
  -expected.png
  -diff.png
and computes per-snapshot mismatch ratios with Pillow.
"""

from __future__ import annotations

import argparse
import datetime as dt
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

try:
    from PIL import Image, ImageChops
except ImportError as exc:  # pragma: no cover
    raise SystemExit(
        "Pillow is required for visual diff analysis. Install with: pip install pillow"
    ) from exc


@dataclass
class DiffEntry:
    case_name: str
    snapshot_name: str
    expected: Path
    actual: Path
    diff: Path
    changed_pixels: int
    total_pixels: int
    ratio: float
    note: str | None


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build a visual regression markdown report.")
    parser.add_argument(
        "--test-results-dir",
        default="test-results",
        help="Directory that contains Playwright test artifacts (default: test-results).",
    )
    parser.add_argument(
        "--output",
        default="test-results/visual-diff-report.md",
        help="Markdown output path.",
    )
    parser.add_argument(
        "--show-images",
        action="store_true",
        help="Embed markdown image links for expected/actual/diff.",
    )
    return parser.parse_args()


def collect_diff_entries(test_results_dir: Path) -> list[DiffEntry]:
    entries: list[DiffEntry] = []
    for diff_path in sorted(test_results_dir.rglob("*-diff.png")):
        prefix = diff_path.name[: -len("-diff.png")]
        expected_path = diff_path.with_name(f"{prefix}-expected.png")
        actual_path = diff_path.with_name(f"{prefix}-actual.png")

        if not expected_path.exists() or not actual_path.exists():
            continue

        changed, total, ratio, note = compute_mismatch(expected_path, actual_path)
        entries.append(
            DiffEntry(
                case_name=diff_path.parent.name,
                snapshot_name=f"{prefix}.png",
                expected=expected_path,
                actual=actual_path,
                diff=diff_path,
                changed_pixels=changed,
                total_pixels=total,
                ratio=ratio,
                note=note,
            )
        )
    return entries


def compute_mismatch(expected_path: Path, actual_path: Path) -> tuple[int, int, float, str | None]:
    with Image.open(expected_path) as expected_raw, Image.open(actual_path) as actual_raw:
        expected = expected_raw.convert("RGBA")
        actual = actual_raw.convert("RGBA")

        if expected.size != actual.size:
            max_pixels = max(expected.width * expected.height, actual.width * actual.height)
            note = (
                f"image size changed: expected={expected.width}x{expected.height}, "
                f"actual={actual.width}x{actual.height}"
            )
            return max_pixels, max_pixels, 1.0, note

        diff = ImageChops.difference(expected, actual).convert("L")
        histogram = diff.histogram()
        unchanged_pixels = histogram[0]
        total_pixels = expected.width * expected.height
        changed_pixels = max(total_pixels - unchanged_pixels, 0)
        ratio = changed_pixels / total_pixels if total_pixels > 0 else 0.0
        return changed_pixels, total_pixels, ratio, None


def ratio_band(ratio: float) -> str:
    if ratio >= 0.02:
        return "high"
    if ratio >= 0.005:
        return "medium"
    return "low"


def to_display_path(path: Path, output_parent: Path) -> str:
    try:
        return path.relative_to(output_parent).as_posix()
    except ValueError:
        return path.as_posix()


def write_report(
    output_path: Path,
    entries: Iterable[DiffEntry],
    test_results_dir: Path,
    show_images: bool,
) -> None:
    output_path.parent.mkdir(parents=True, exist_ok=True)

    entry_list = sorted(entries, key=lambda item: item.ratio, reverse=True)
    now = dt.datetime.now().strftime("%Y-%m-%d %H:%M:%S")

    lines: list[str] = []
    lines.append("# Visual Diff Report")
    lines.append("")
    lines.append(f"- Generated: `{now}`")
    lines.append(f"- Artifacts root: `{test_results_dir.as_posix()}`")
    lines.append(f"- Diff cases: `{len(entry_list)}`")
    lines.append("")

    if not entry_list:
        lines.append("No visual diffs were found. The current run matches the baseline snapshots.")
    else:
        for index, entry in enumerate(entry_list, start=1):
            lines.append(f"## {index}. {entry.snapshot_name}")
            lines.append(f"- Case: `{entry.case_name}`")
            lines.append(f"- Severity: `{ratio_band(entry.ratio)}`")
            lines.append(
                f"- Mismatch: `{entry.changed_pixels}/{entry.total_pixels}` "
                f"(`{entry.ratio * 100:.4f}%`)"
            )
            if entry.note:
                lines.append(f"- Note: `{entry.note}`")
            lines.append(
                f"- Expected: `{to_display_path(entry.expected, output_path.parent)}`"
            )
            lines.append(
                f"- Actual: `{to_display_path(entry.actual, output_path.parent)}`"
            )
            lines.append(
                f"- Diff: `{to_display_path(entry.diff, output_path.parent)}`"
            )
            if show_images:
                lines.append("")
                lines.append(
                    f"![expected]({to_display_path(entry.expected, output_path.parent)})"
                )
                lines.append(
                    f"![actual]({to_display_path(entry.actual, output_path.parent)})"
                )
                lines.append(f"![diff]({to_display_path(entry.diff, output_path.parent)})")
            lines.append("")

    output_path.write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    args = parse_args()
    test_results_dir = Path(args.test_results_dir).resolve()
    output_path = Path(args.output).resolve()

    if not test_results_dir.exists():
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(
            "# Visual Diff Report\n\n"
            "No diff artifacts were found.\n\n"
            f"- Checked directory: `{test_results_dir.as_posix()}`\n"
            "- This is expected when all visual comparisons pass.\n",
            encoding="utf-8",
        )
        print(f"[visual-diff] no diff artifacts found under {test_results_dir}")
        return 0

    entries = collect_diff_entries(test_results_dir)
    write_report(output_path, entries, test_results_dir, args.show_images)
    print(f"[visual-diff] report written to {output_path}")
    print(f"[visual-diff] diff cases: {len(entries)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
