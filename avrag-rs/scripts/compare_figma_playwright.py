#!/usr/bin/env python3
"""
Compare Figma screenshots (expected) and Playwright screenshots (actual).

Outputs:
- markdown summary report
- machine-readable JSON summary
- optional per-page diff images
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Iterable

try:
    from PIL import Image, ImageChops
except ImportError as exc:  # pragma: no cover
    raise SystemExit(
        "Pillow is required. Install with: pip install pillow"
    ) from exc


@dataclass
class CompareResult:
    name: str
    expected: str
    actual: str
    diff: str | None
    changed_pixels: int
    total_pixels: int
    ratio: float
    size_note: str | None
    status: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Compare figma screenshots against playwright screenshots."
    )
    parser.add_argument(
        "--expected-dir",
        required=True,
        help="Directory containing expected Figma png files.",
    )
    parser.add_argument(
        "--actual-dir",
        required=True,
        help="Directory containing actual Playwright png files.",
    )
    parser.add_argument(
        "--out-dir",
        required=True,
        help="Directory for report/json/diff outputs.",
    )
    parser.add_argument(
        "--threshold",
        type=float,
        default=0.02,
        help="Mismatch ratio threshold (default: 0.02 = 2%%).",
    )
    parser.add_argument(
        "--files",
        default="",
        help="Comma-separated page names without extension; empty means auto-discover.",
    )
    parser.add_argument(
        "--write-diff-images",
        action="store_true",
        help="Write visual diff png files.",
    )
    return parser.parse_args()


def discover_pages(expected_dir: Path, actual_dir: Path, selected: str) -> list[str]:
    if selected.strip():
        return sorted({item.strip() for item in selected.split(",") if item.strip()})
    expected_names = {p.stem for p in expected_dir.glob("*.png")}
    actual_names = {p.stem for p in actual_dir.glob("*.png")}
    return sorted(expected_names | actual_names)


def compare_one(
    name: str,
    expected_file: Path,
    actual_file: Path,
    diff_dir: Path,
    threshold: float,
    write_diff_images: bool,
) -> CompareResult:
    if not expected_file.exists():
        return CompareResult(
            name=name,
            expected=expected_file.as_posix(),
            actual=actual_file.as_posix(),
            diff=None,
            changed_pixels=1,
            total_pixels=1,
            ratio=1.0,
            size_note="missing expected image",
            status="fail",
        )
    if not actual_file.exists():
        return CompareResult(
            name=name,
            expected=expected_file.as_posix(),
            actual=actual_file.as_posix(),
            diff=None,
            changed_pixels=1,
            total_pixels=1,
            ratio=1.0,
            size_note="missing actual image",
            status="fail",
        )

    with Image.open(expected_file) as expected_raw, Image.open(actual_file) as actual_raw:
        expected = expected_raw.convert("RGBA")
        actual = actual_raw.convert("RGBA")
        size_note = None
        diff_path = None

        if expected.size != actual.size:
            total_pixels = max(
                expected.width * expected.height, actual.width * actual.height
            )
            changed_pixels = total_pixels
            ratio = 1.0
            size_note = (
                f"size mismatch expected={expected.width}x{expected.height}, "
                f"actual={actual.width}x{actual.height}"
            )
        else:
            diff = ImageChops.difference(expected, actual)
            alpha = diff.split()[3]
            total_pixels = expected.width * expected.height
            histogram = alpha.histogram()
            unchanged_pixels = histogram[0] if histogram else 0
            changed_pixels = max(total_pixels - unchanged_pixels, 0)
            ratio = changed_pixels / total_pixels if total_pixels else 0.0
            if write_diff_images:
                diff_path = diff_dir / f"{name}-diff.png"
                diff.save(diff_path)

        status = "pass" if ratio <= threshold else "fail"
        return CompareResult(
            name=name,
            expected=expected_file.as_posix(),
            actual=actual_file.as_posix(),
            diff=diff_path.as_posix() if diff_path else None,
            changed_pixels=changed_pixels,
            total_pixels=total_pixels,
            ratio=ratio,
            size_note=size_note,
            status=status,
        )


def write_json(path: Path, threshold: float, results: Iterable[CompareResult]) -> None:
    result_list = list(results)
    payload = {
        "generated_at": dt.datetime.now().isoformat(timespec="seconds"),
        "threshold": threshold,
        "total": len(result_list),
        "failed": sum(1 for item in result_list if item.status == "fail"),
        "results": [asdict(item) for item in result_list],
    }
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")


def write_markdown(path: Path, threshold: float, results: Iterable[CompareResult]) -> None:
    result_list = sorted(results, key=lambda item: item.ratio, reverse=True)
    failed = [item for item in result_list if item.status == "fail"]

    lines: list[str] = []
    lines.append("# Figma vs Playwright Visual Parity Report")
    lines.append("")
    lines.append(f"- Generated: `{dt.datetime.now().strftime('%Y-%m-%d %H:%M:%S')}`")
    lines.append(f"- Threshold: `{threshold * 100:.2f}%`")
    lines.append(f"- Compared pages: `{len(result_list)}`")
    lines.append(f"- Failed pages: `{len(failed)}`")
    lines.append("")

    if not result_list:
        lines.append("No pages were compared.")
    else:
        for item in result_list:
            lines.append(f"## {item.name}")
            lines.append(f"- Status: `{item.status}`")
            lines.append(
                f"- Mismatch: `{item.changed_pixels}/{item.total_pixels}` "
                f"(`{item.ratio * 100:.4f}%`)"
            )
            if item.size_note:
                lines.append(f"- Note: `{item.size_note}`")
            lines.append(f"- Expected: `{item.expected}`")
            lines.append(f"- Actual: `{item.actual}`")
            if item.diff:
                lines.append(f"- Diff: `{item.diff}`")
            lines.append("")

    path.write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    args = parse_args()
    expected_dir = Path(args.expected_dir).resolve()
    actual_dir = Path(args.actual_dir).resolve()
    out_dir = Path(args.out_dir).resolve()
    diff_dir = out_dir / "diff"
    out_dir.mkdir(parents=True, exist_ok=True)
    if args.write_diff_images:
        diff_dir.mkdir(parents=True, exist_ok=True)

    pages = discover_pages(expected_dir, actual_dir, args.files)
    results = [
        compare_one(
            name=name,
            expected_file=expected_dir / f"{name}.png",
            actual_file=actual_dir / f"{name}.png",
            diff_dir=diff_dir,
            threshold=args.threshold,
            write_diff_images=args.write_diff_images,
        )
        for name in pages
    ]

    json_path = out_dir / "summary.json"
    md_path = out_dir / "report.md"
    write_json(json_path, args.threshold, results)
    write_markdown(md_path, args.threshold, results)

    failed = sum(1 for item in results if item.status == "fail")
    print(
        json.dumps(
            {
                "threshold": args.threshold,
                "pages": len(results),
                "failed": failed,
                "json": json_path.as_posix(),
                "report": md_path.as_posix(),
            },
            ensure_ascii=False,
        )
    )
    return 1 if failed > 0 else 0


if __name__ == "__main__":
    raise SystemExit(main())
