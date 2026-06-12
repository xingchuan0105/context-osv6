#!/usr/bin/env python3
"""R1–R3 threshold calibration: per-page readable_ratio, figure_area_ratio, route prediction.

Requires: pip install pymupdf

Example:
  python scripts/spike/probe_page_stats.py \\
    --pdf "/mnt/e/.../Taleb_Antifragile__2012.pdf" \\
    --sample 30 --out docs/spike/probe-antifragile-30.json
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter
from pathlib import Path
from typing import Any

# Thresholds — keep in sync with docs/ingestion-routing-discussion-2026-06-10.md §1.5
TEXT_QUAL_THRESHOLD = 0.3
BIGRAM_REPEAT_THRESHOLD = 0.30
PAGE_TEXT_THRESHOLD = 100
FIG_RATIO_THRESHOLD = 0.15
FIG_RATIO_ALT = 0.10
FIG_COUNT_THRESHOLD = 2
DECORATIVE_AREA_RATIO = 0.02
HEADER_FOOTER_MARGIN = 0.10
TABLE_GARBLE_THRESHOLD = 0.30

WORD_RE = re.compile(r"[A-Za-z]{2,}|[\u4e00-\u9fff]")
GARBLED_RE = re.compile(r"[^\x20-\x7E\u4e00-\u9fff\s.,;:!?\'\"()\[\]{}%\-–—/\\@#&+]")
PIPE_TAB_RE = re.compile(r"[|\t]{2,}")
WATERMARK_PATTERNS = (
    re.compile(r"ePub Converter", re.I),
    re.compile(r"processtext\.com", re.I),
    re.compile(r"ABC Amber", re.I),
)
UNIQUE_TOKEN_THRESHOLD = 0.4


def tokenize(text: str) -> list[str]:
    return WORD_RE.findall(text)


def readable_ratio(text: str) -> float:
    tokens = tokenize(text)
    if not tokens:
        return 0.0
    # All matched tokens count as readable for this spike heuristic.
    return min(1.0, len(tokens) / max(1, len(text.split())))


def bigram_repeat_ratio(text: str) -> float:
    words = text.lower().split()
    if len(words) < 4:
        return 0.0
    bigrams = [f"{words[i]} {words[i+1]}" for i in range(len(words) - 1)]
    if not bigrams:
        return 0.0
    counts = Counter(bigrams)
    most_common = counts.most_common(1)[0][1]
    return most_common / len(bigrams)


def garbled_ratio(text: str) -> float:
    if not text:
        return 0.0
    garbled = len(GARBLED_RE.findall(text))
    return garbled / max(1, len(text))


def table_hint_count(text: str) -> int:
    pipe_lines = sum(1 for line in text.splitlines() if "|" in line)
    tab_lines = sum(1 for line in text.splitlines() if line.count("\t") >= 2)
    delim_hits = len(PIPE_TAB_RE.findall(text))
    return pipe_lines + tab_lines + delim_hits


def is_decorative(img_area: float, page_area: float, y0: float, page_h: float) -> bool:
    ratio = img_area / page_area if page_area else 0.0
    if ratio >= DECORATIVE_AREA_RATIO:
        return False
    rel_y = y0 / page_h if page_h else 0.5
    in_header = rel_y < HEADER_FOOTER_MARGIN
    in_footer = rel_y > (1.0 - HEADER_FOOTER_MARGIN)
    return in_header or in_footer


def unique_token_ratio(text: str) -> float:
    tokens = tokenize(text)
    if not tokens:
        return 0.0
    return len(set(tokens)) / len(tokens)


def watermark_hit(text: str) -> bool:
    return any(p.search(text) for p in WATERMARK_PATTERNS)


def predict_route(
    text_chars: int,
    readable: float,
    bigram_rep: float,
    unique_ratio: float,
    has_watermark: bool,
    figure_area_ratio: float,
    non_decorative_images: int,
    table_hint: int,
    table_garbled: float,
) -> str:
    if (
        text_chars == 0
        or readable < TEXT_QUAL_THRESHOLD
        or bigram_rep > BIGRAM_REPEAT_THRESHOLD
        or has_watermark
        or unique_ratio < UNIQUE_TOKEN_THRESHOLD
        or (text_chars < PAGE_TEXT_THRESHOLD and readable < 0.5)
    ):
        return "C"
    if table_hint > 0 and table_garbled > TABLE_GARBLE_THRESHOLD:
        return "C_prime"
    fig_trigger = (
        figure_area_ratio > FIG_RATIO_THRESHOLD
        and non_decorative_images >= FIG_COUNT_THRESHOLD
    ) or (
        figure_area_ratio > FIG_RATIO_ALT
        and non_decorative_images >= FIG_COUNT_THRESHOLD
    )
    if fig_trigger:
        return "B"
    return "A"


def analyze_pdf(pdf_path: Path, sample: int, page_start: int) -> dict[str, Any]:
    try:
        import fitz  # PyMuPDF
    except ImportError:
        print("Install pymupdf: pip install pymupdf", file=sys.stderr)
        sys.exit(1)

    doc = fitz.open(pdf_path)
    total_pages = doc.page_count
    indices = list(range(page_start - 1, min(total_pages, page_start - 1 + sample)))

    pages: list[dict[str, Any]] = []
    route_counts: Counter[str] = Counter()

    for i in indices:
        page = doc.load_page(i)
        text = page.get_text("text") or ""
        text_chars = len(text)
        readable = readable_ratio(text)
        bigram_rep = bigram_repeat_ratio(text)
        unique_ratio = unique_token_ratio(text)
        wm = watermark_hit(text)
        garbled = garbled_ratio(text)
        t_hint = table_hint_count(text)

        rect = page.rect
        page_area = rect.width * rect.height
        page_h = rect.height

        img_area_sum = 0.0
        decorative = 0
        non_decorative = 0
        image_xobjects = 0

        for img in page.get_images(full=True):
            image_xobjects += 1
            try:
                xref = img[0]
                for info in page.get_image_info(xrefs=True):
                    if info.get("xref") != xref:
                        continue
                    bbox = fitz.Rect(info["bbox"])
                    area = bbox.width * bbox.height
                    if is_decorative(area, page_area, bbox.y0, page_h):
                        decorative += 1
                    else:
                        non_decorative += 1
                        img_area_sum += area
            except Exception:
                non_decorative += 1
                img_area_sum += page_area * 0.05

        figure_area_ratio = img_area_sum / page_area if page_area else 0.0
        route = predict_route(
            text_chars, readable, bigram_rep, unique_ratio, wm,
            figure_area_ratio, non_decorative, t_hint, garbled,
        )
        route_counts[route] += 1

        pages.append({
            "page_number": i + 1,
            "text_chars": text_chars,
            "readable_ratio": round(readable, 3),
            "bigram_repeat_ratio": round(bigram_rep, 3),
            "unique_token_ratio": round(unique_ratio, 3),
            "watermark_hit": wm,
            "garbled_ratio": round(garbled, 3),
            "image_xobjects": image_xobjects,
            "decorative_images": decorative,
            "non_decorative_images": non_decorative,
            "figure_area_ratio": round(figure_area_ratio, 3),
            "table_hint_count": t_hint,
            "predicted_route": route,
            "text_preview": text[:120].replace("\n", " "),
        })

    doc.close()

    naive_b = sum(1 for p in pages if p["image_xobjects"] > 0)
    smart_b = sum(1 for p in pages if p["predicted_route"] == "B")

    return {
        "pdf": str(pdf_path),
        "total_pages": total_pages,
        "sampled_pages": len(pages),
        "route_distribution": dict(route_counts),
        "naive_figure_pages_image_gt_0": naive_b,
        "smart_figure_pages_route_B": smart_b,
        "b_trigger_reduction_pct": round(
            (1 - smart_b / naive_b) * 100 if naive_b else 0.0, 1
        ),
        "pages": pages,
        "thresholds": {
            "TEXT_QUAL_THRESHOLD": TEXT_QUAL_THRESHOLD,
            "FIG_RATIO_THRESHOLD": FIG_RATIO_THRESHOLD,
            "FIG_COUNT_THRESHOLD": FIG_COUNT_THRESHOLD,
        },
    }


def main() -> None:
    parser = argparse.ArgumentParser(description="PDF page probe stats for routing thresholds")
    parser.add_argument("--pdf", required=True, type=Path)
    parser.add_argument("--sample", type=int, default=30)
    parser.add_argument("--page-start", type=int, default=1)
    parser.add_argument("--out", type=Path, default=None)
    args = parser.parse_args()

    if not args.pdf.exists():
        print(f"PDF not found: {args.pdf}", file=sys.stderr)
        sys.exit(1)

    report = analyze_pdf(args.pdf, args.sample, args.page_start)
    text = json.dumps(report, ensure_ascii=False, indent=2)

    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(text, encoding="utf-8")
        print(f"Wrote {args.out}")
    else:
        print(text)

    print(
        f"\nSummary: routes={report['route_distribution']} "
        f"naive_B={report['naive_figure_pages_image_gt_0']} "
        f"smart_B={report['smart_figure_pages_route_B']} "
        f"reduction={report['b_trigger_reduction_pct']}%",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
