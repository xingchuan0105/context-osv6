#!/usr/bin/env python3
"""Phase 0: Paddle OCR job spike on a PDF slice (default Black Swan p1-20)."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import requests

# WSL 环境常设 https_proxy，会导致百度 API SSL 握手失败；spike 默认直连。
_SESSION = requests.Session()
_SESSION.trust_env = False

_BASE = os.environ.get(
    "PADDLE_OCR_BASE_URL", "https://paddleocr.aistudio-app.com/api/v2/ocr"
).rstrip("/")
JOB_URL = f"{_BASE}/jobs"
TOKEN = os.environ.get("PADDLE_OCR_API_TOKEN", "")
MODEL = os.environ.get("PADDLE_OCR_MODEL", "PaddleOCR-VL-1.6")
POLL_SECS = int(os.environ.get("PADDLE_OCR_POLL_INTERVAL_SECS", "5"))
TIMEOUT_SECS = int(os.environ.get("PADDLE_OCR_JOB_TIMEOUT_SECS", "3600"))

OPTIONAL_PAYLOAD = {
    "useDocOrientationClassify": False,
    "useDocUnwarping": False,
    "useChartRecognition": False,
}


def extract_page_range(input_pdf: Path, first: int, last: int, out_pdf: Path) -> None:
    work = out_pdf.parent / "pages"
    work.mkdir(parents=True, exist_ok=True)
    pattern = str(work / "page-%d.pdf")
    subprocess.run(
        [
            "pdfseparate",
            "-f",
            str(first),
            "-l",
            str(last),
            str(input_pdf),
            pattern,
        ],
        check=True,
        capture_output=True,
    )
    parts = [work / f"page-{n}.pdf" for n in range(first, last + 1)]
    subprocess.run(["pdfunite", *[str(p) for p in parts], str(out_pdf)], check=True, capture_output=True)


def submit_job(file_path: Path) -> str:
    headers = {"Authorization": f"bearer {TOKEN}"}
    data = {"model": MODEL, "optionalPayload": json.dumps(OPTIONAL_PAYLOAD)}
    with file_path.open("rb") as handle:
        response = _SESSION.post(
            JOB_URL, headers=headers, data=data, files={"file": handle}, timeout=120
        )
    response.raise_for_status()
    payload = response.json()
    return payload["data"]["jobId"]


def poll_job(job_id: str) -> dict:
    headers = {"Authorization": f"bearer {TOKEN}"}
    deadline = time.time() + TIMEOUT_SECS
    while time.time() < deadline:
        response = _SESSION.get(f"{JOB_URL}/{job_id}", headers=headers, timeout=60)
        response.raise_for_status()
        data = response.json()["data"]
        state = data.get("state")
        if state == "done":
            return data
        if state == "failed":
            raise RuntimeError(data.get("errorMsg") or "paddle job failed")
        progress = data.get("extractProgress") or {}
        extracted = progress.get("extractedPages")
        total = progress.get("totalPages")
        print(f"[paddle] state={state} pages={extracted}/{total}", flush=True)
        time.sleep(POLL_SECS)
    raise TimeoutError(f"paddle job {job_id} timed out after {TIMEOUT_SECS}s")


def sample_jsonl(json_url: str, max_pages: int = 3) -> list[dict]:
    response = _SESSION.get(json_url, timeout=120)
    response.raise_for_status()
    samples: list[dict] = []
    for line in response.text.strip().splitlines():
        if not line.strip():
            continue
        row = json.loads(line)
        result = row.get("result") or {}
        for layout in result.get("layoutParsingResults") or []:
            md = (layout.get("markdown") or {}).get("text") or ""
            samples.append(
                {
                    "text_chars": len(md.strip()),
                    "text_preview": md.strip()[:400],
                    "image_count": len((layout.get("markdown") or {}).get("images") or {}),
                }
            )
            if len(samples) >= max_pages:
                return samples
    return samples


def main() -> int:
    if not TOKEN:
        print("PADDLE_OCR_API_TOKEN is required", file=sys.stderr)
        return 1

    input_pdf = Path(
        os.environ.get(
            "SPIKE_PDF",
            "/mnt/e/OneDrive/桌面/知境笔记/the-black-swan_-the-impact-of-the-highly-improbable-second-edition-pdfdrive.com-.pdf",
        )
    )
    first_page = int(os.environ.get("SPIKE_FIRST_PAGE", "1"))
    last_page = int(os.environ.get("SPIKE_LAST_PAGE", "20"))
    out_dir = Path(os.environ.get("SPIKE_OUT_DIR", "docs/spike/paddle-black-swan-p1-20"))
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "spike.log").touch(exist_ok=True)

    if not input_pdf.is_file():
        print(f"PDF not found: {input_pdf}", file=sys.stderr)
        return 1

    started = time.time()
    with tempfile.TemporaryDirectory(prefix="paddle-spike-") as tmp:
        slice_pdf = Path(tmp) / f"pages_{first_page}_{last_page}.pdf"
        print(f"[paddle] extracting pages {first_page}-{last_page} from {input_pdf}")
        extract_page_range(input_pdf, first_page, last_page, slice_pdf)

        print(f"[paddle] submitting job model={MODEL}")
        job_id = submit_job(slice_pdf)
        print(f"[paddle] job_id={job_id}")

        done = poll_job(job_id)
        elapsed = time.time() - started
        progress = done.get("extractProgress") or {}
        json_url = (done.get("resultUrl") or {}).get("jsonUrl")
        if not json_url:
            raise RuntimeError("paddle job done but jsonUrl missing")

        samples = sample_jsonl(json_url, max_pages=5)
        report = {
            "input_pdf": str(input_pdf),
            "page_range": [first_page, last_page],
            "job_id": job_id,
            "elapsed_secs": round(elapsed, 1),
            "extract_progress": progress,
            "json_url": json_url,
            "sample_pages": samples,
        }
        report_path = out_dir / "report.json"
        report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps(report, ensure_ascii=False, indent=2))
        print(f"[paddle] wrote {report_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
