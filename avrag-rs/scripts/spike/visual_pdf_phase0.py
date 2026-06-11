#!/usr/bin/env python3
"""Phase 0 spike: PyMuPDF render + qwen3-vl-embedding (1-page vs 4-page fusion).

Reads embedding config from avrag-rs/.env (no hardcoded secrets).
Outputs metrics JSON for docs/spike/phase0-metrics-*.md population.

Usage:
  cd avrag-rs
  python3 scripts/spike/visual_pdf_phase0.py --pages 40 --pdf "$E2E_LLM_REAL_BLACK_SWAN_PDF"
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import sys
import time
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

import fitz  # PyMuPDF
import requests

ROOT = Path(__file__).resolve().parents[2]
ENV_PATH = ROOT / ".env"
DEFAULT_PDF = os.environ.get(
    "E2E_LLM_REAL_BLACK_SWAN_PDF",
    "/mnt/e/OneDrive/桌面/the-black-swan_-the-impact-of-the-highly-improbable-second-edition-pdfdrive.com-.pdf",
)

SPIKE_QUERIES = [
    "What is a black swan event according to Taleb?",
    "How does Taleb explain the problem of induction and prediction?",
    "What role does narrative fallacy play in misunderstanding history?",
    "Compare mediocristan and extremistan in Taleb's framework.",
    "Where does Taleb discuss silent evidence or hidden failures?",
]


def load_env(path: Path) -> dict[str, str]:
    out: dict[str, str] = {}
    if not path.is_file():
        return out
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        k, _, v = line.partition("=")
        out[k.strip()] = v.strip()
    return out


def render_page_jpeg(doc: fitz.Document, page_num: int, matrix_scale: float = 0.75) -> bytes:
    page = doc[page_num - 1]
    images = page.get_images(full=True)
    if images:
        xref = images[0][0]
        try:
            base = doc.extract_image(xref)
            if base and base.get("image"):
                return base["image"]
        except Exception:
            pass
    pix = page.get_pixmap(matrix=fitz.Matrix(matrix_scale, matrix_scale), alpha=False)
    return pix.tobytes("jpeg")


def chunk_pages(page_nums: list[int], pages_per_chunk: int) -> list[list[int]]:
    chunks: list[list[int]] = []
    for i in range(0, len(page_nums), pages_per_chunk):
        chunks.append(page_nums[i : i + pages_per_chunk])
    return chunks


def b64_data_url(jpeg_bytes: bytes) -> str:
    return "data:image/jpeg;base64," + base64.b64encode(jpeg_bytes).decode("ascii")


@dataclass
class EmbedResult:
    chunk_pages: list[int]
    latency_ms: float
    status_code: int
    image_tokens: int | None = None
    error: str | None = None


@dataclass
class SpikeReport:
    pdf_path: str
    page_count_rendered: int
    render_total_ms: float
    strategies: dict[str, Any] = field(default_factory=dict)
    recall_probe: list[dict[str, Any]] = field(default_factory=list)


def embed_fusion(
    base_url: str,
    api_key: str,
    model: str,
    dimension: int,
    caption: str,
    image_data_urls: list[str],
    timeout_s: int = 60,
) -> tuple[dict[str, Any], float, int]:
    # DashScope qwen3-vl-embedding fusion: each modality is a separate contents entry.
    contents: list[dict[str, Any]] = []
    if caption.strip():
        contents.append({"text": caption})
    for url in image_data_urls:
        contents.append({"image": url})
    if not contents:
        raise ValueError("embed_fusion requires text and/or at least one image")

    params: dict[str, Any] = {
        "output_type": "dense",
        "dimension": dimension,
    }
    if len(contents) > 1:
        params["enable_fusion"] = True

    body = {
        "model": model,
        "input": {"contents": contents},
        "parameters": params,
    }
    t0 = time.perf_counter()
    resp = requests.post(
        base_url,
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
        },
        json=body,
        timeout=timeout_s,
    )
    latency_ms = (time.perf_counter() - t0) * 1000
    data: dict[str, Any] = {}
    if resp.headers.get("content-type", "").startswith("application/json"):
        try:
            data = resp.json()
        except Exception:
            data = {"raw": resp.text[:500]}
    return data, latency_ms, resp.status_code


def extract_image_tokens(data: dict[str, Any]) -> int | None:
    usage = data.get("usage") or {}
    for key in ("image_tokens", "total_tokens"):
        if key in usage and isinstance(usage[key], int):
            return usage[key]
    output = data.get("output") or {}
    embeddings = output.get("embeddings") or []
    if embeddings and isinstance(embeddings[0], dict):
        for key in ("image_tokens", "tokens"):
            if key in embeddings[0]:
                return int(embeddings[0][key])
    return None


def cosine(a: list[float], b: list[float]) -> float:
    dot = sum(x * y for x, y in zip(a, b))
    na = sum(x * x for x in a) ** 0.5
    nb = sum(x * x for x in b) ** 0.5
    if na == 0 or nb == 0:
        return 0.0
    return dot / (na * nb)


def run_strategy(
    env: dict[str, str],
    doc: fitz.Document,
    page_nums: list[int],
    pages_per_chunk: int,
    label: str,
) -> dict[str, Any]:
    base_url = env.get(
        "MM_EMBEDDING_BASE_URL",
        "https://dashscope.aliyuncs.com/api/v1/services/embeddings/multimodal-embedding/multimodal-embedding",
    )
    api_key = env.get("MM_EMBEDDING_API_KEY") or env.get("DASHSCOPE_API_KEY", "")
    model = env.get("MM_EMBEDDING_MODEL", "qwen3-vl-embedding")
    dimension = int(env.get("MM_EMBEDDING_DIMENSIONS", "1024"))

    if not api_key:
        raise SystemExit("MM_EMBEDDING_API_KEY or DASHSCOPE_API_KEY required in .env")

    render_t0 = time.perf_counter()
    page_jpegs: dict[int, bytes] = {}
    for p in page_nums:
        page_jpegs[p] = render_page_jpeg(doc, p)
    render_ms = (time.perf_counter() - render_t0) * 1000

    embed_results: list[EmbedResult] = []
    vectors: list[tuple[list[int], list[float]]] = []

    for group in chunk_pages(page_nums, pages_per_chunk):
        urls = [b64_data_url(page_jpegs[p]) for p in group]
        caption = f"PDF pages {group[0]}-{group[-1]}"
        try:
            data, latency_ms, status = embed_fusion(
                base_url, api_key, model, dimension, caption, urls
            )
            if status >= 400:
                embed_results.append(
                    EmbedResult(
                        chunk_pages=group,
                        latency_ms=latency_ms,
                        status_code=status,
                        error=json.dumps(data)[:300],
                    )
                )
                continue
            tokens = extract_image_tokens(data)
            emb = (
                (data.get("output") or {})
                .get("embeddings", [{}])[0]
                .get("embedding")
            )
            if not emb:
                embed_results.append(
                    EmbedResult(
                        chunk_pages=group,
                        latency_ms=latency_ms,
                        status_code=status,
                        error="no embedding in response",
                    )
                )
                continue
            vectors.append((group, emb))
            embed_results.append(
                EmbedResult(
                    chunk_pages=group,
                    latency_ms=latency_ms,
                    status_code=status,
                    image_tokens=tokens,
                )
            )
        except Exception as exc:
            embed_results.append(
                EmbedResult(
                    chunk_pages=group,
                    latency_ms=0,
                    status_code=0,
                    error=str(exc),
                )
            )
        time.sleep(0.3)

    recall_rows: list[dict[str, Any]] = []
    if vectors:
        for query in SPIKE_QUERIES:
            q_data, q_lat, q_status = embed_fusion(
                base_url, api_key, model, dimension, query, []
            )
            q_emb = (
                (q_data.get("output") or {})
                .get("embeddings", [{}])[0]
                .get("embedding")
            )
            if not q_emb:
                recall_rows.append({"query": query, "error": "query embed failed", "status": q_status})
                continue
            scored = sorted(
                (
                    {
                        "pages": g,
                        "score": cosine(q_emb, v),
                    }
                    for g, v in vectors
                ),
                key=lambda x: x["score"],
                reverse=True,
            )[:3]
            recall_rows.append(
                {
                    "query": query,
                    "top3": scored,
                    "query_latency_ms": q_lat,
                }
            )

    ok_embeds = [r for r in embed_results if r.error is None]
    token_samples = [r.image_tokens for r in ok_embeds if r.image_tokens is not None]

    return {
        "label": label,
        "pages_per_chunk": pages_per_chunk,
        "render_ms": round(render_ms, 1),
        "embed_calls": len(embed_results),
        "embed_ok": len(ok_embeds),
        "embed_fail": len(embed_results) - len(ok_embeds),
        "avg_embed_latency_ms": round(
            sum(r.latency_ms for r in ok_embeds) / max(len(ok_embeds), 1), 1
        ),
        "image_tokens_samples": token_samples[:10],
        "image_tokens_avg": round(sum(token_samples) / max(len(token_samples), 1), 1)
        if token_samples
        else None,
        "embed_results": [asdict(r) for r in embed_results],
        "recall_probe": recall_rows,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description="Visual PDF Phase 0 spike")
    parser.add_argument("--pdf", default=DEFAULT_PDF)
    parser.add_argument("--pages", type=int, default=40, help="First N pages")
    parser.add_argument("--out", default=str(ROOT / "docs/spike/phase0-metrics-2026-06-10.json"))
    args = parser.parse_args()

    pdf_path = Path(args.pdf)
    if not pdf_path.is_file():
        print(f"PDF not found: {pdf_path}", file=sys.stderr)
        sys.exit(1)

    env = {**load_env(ENV_PATH), **os.environ}
    doc = fitz.open(pdf_path)
    total = min(args.pages, len(doc))
    page_nums = list(range(1, total + 1))

    report = SpikeReport(
        pdf_path=str(pdf_path),
        page_count_rendered=total,
        render_total_ms=0,
    )

    for label, ppc in [("one_page", 1), ("four_page_fusion", 4)]:
        print(f"Running strategy {label} (pages_per_chunk={ppc})...")
        report.strategies[label] = run_strategy(env, doc, page_nums, ppc, label)

    doc.close()

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(asdict(report), indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"Wrote {out_path}")

    md_path = out_path.with_suffix(".md")
    lines = [
        "# Phase 0 Metrics (auto-generated)",
        "",
        f"- PDF: `{report.pdf_path}`",
        f"- Pages rendered: {report.page_count_rendered}",
        "",
        "## Strategies",
        "",
    ]
    for key, strat in report.strategies.items():
        lines.append(f"### {key}")
        lines.append(f"- pages_per_chunk: {strat['pages_per_chunk']}")
        lines.append(f"- render_ms: {strat['render_ms']}")
        lines.append(f"- embed_ok/fail: {strat['embed_ok']}/{strat['embed_fail']}")
        lines.append(f"- avg_embed_latency_ms: {strat['avg_embed_latency_ms']}")
        lines.append(f"- image_tokens_avg: {strat.get('image_tokens_avg')}")
        lines.append("")
    md_path.write_text("\n".join(lines), encoding="utf-8")
    print(f"Wrote {md_path}")


if __name__ == "__main__":
    main()
