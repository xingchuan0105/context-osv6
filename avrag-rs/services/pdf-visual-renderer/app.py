#!/usr/bin/env python3
"""PDF visual renderer sidecar — PyMuPDF page rasterization."""

from __future__ import annotations

import atexit
import os
import tempfile
from collections import OrderedDict
from hashlib import sha256
from threading import Lock
from typing import Any

import fitz
from fastapi import FastAPI, File, Form, HTTPException, UploadFile
from fastapi.responses import JSONResponse

MAX_PAGES_PER_REQUEST = int(os.environ.get("PDF_RENDERER_MAX_PAGES", "20"))
MAX_OUTPUT_BYTES = int(os.environ.get("PDF_RENDERER_MAX_OUTPUT_BYTES", str(8 * 1024 * 1024)))
MAX_PDF_OBJECTS = int(os.environ.get("PDF_RENDERER_MAX_OBJECTS", "50000"))
DEFAULT_MATRIX = float(os.environ.get("PDF_RENDERER_MATRIX", "0.75"))
MAX_PDF_CACHE_ENTRIES = int(os.environ.get("PDF_RENDERER_PDF_CACHE_ENTRIES", "8"))

_PDF_CACHE: OrderedDict[str, str] = OrderedDict()
_PDF_CACHE_LOCK = Lock()

app = FastAPI(title="pdf-visual-renderer", version="1.0.0")


def _cache_pdf_bytes(raw: bytes) -> str:
    key = sha256(raw).hexdigest()
    with _PDF_CACHE_LOCK:
        cached = _PDF_CACHE.get(key)
        if cached and os.path.isfile(cached):
            _PDF_CACHE.move_to_end(key)
            return cached

        while len(_PDF_CACHE) >= MAX_PDF_CACHE_ENTRIES:
            _, old_path = _PDF_CACHE.popitem(last=False)
            try:
                os.unlink(old_path)
            except OSError:
                pass

        fd, path = tempfile.mkstemp(suffix=".pdf")
        try:
            os.write(fd, raw)
        finally:
            os.close(fd)
        _PDF_CACHE[key] = path
        return path


def _cleanup_pdf_cache() -> None:
    with _PDF_CACHE_LOCK:
        for path in _PDF_CACHE.values():
            try:
                os.unlink(path)
            except OSError:
                pass
        _PDF_CACHE.clear()


atexit.register(_cleanup_pdf_cache)


@app.get("/v1/healthz")
def healthz() -> dict[str, Any]:
    return {"ok": True, "service": "pdf-visual-renderer"}


def _render_page(doc: fitz.Document, page_number: int, strategy: str) -> tuple[bytes, str, int, int]:
    if page_number < 1 or page_number > len(doc):
        raise HTTPException(status_code=400, detail=f"page {page_number} out of range 1..{len(doc)}")

    page = doc[page_number - 1]
    if strategy == "embedded_jpeg":
        images = page.get_images(full=True)
        if images:
            xref = images[0][0]
            try:
                extracted = doc.extract_image(xref)
                if extracted and extracted.get("image"):
                    ext = extracted.get("ext", "jpeg")
                    mime = "image/jpeg" if ext in ("jpg", "jpeg") else f"image/{ext}"
                    w = extracted.get("width", 0)
                    h = extracted.get("height", 0)
                    return extracted["image"], mime, w, h
            except Exception:
                pass

    matrix = fitz.Matrix(DEFAULT_MATRIX, DEFAULT_MATRIX)
    pix = page.get_pixmap(matrix=matrix, alpha=False)
    return pix.tobytes("jpeg"), "image/jpeg", pix.width, pix.height


@app.post("/v1/render-pages")
async def render_pages(
    file: UploadFile = File(...),
    page_start: int = Form(1),
    page_end: int = Form(...),
    strategy: str = Form("pixmap_72dpi"),
) -> JSONResponse:
    if page_start < 1 or page_end < page_start:
        raise HTTPException(status_code=400, detail="invalid page range")
    if page_end - page_start + 1 > MAX_PAGES_PER_REQUEST:
        raise HTTPException(
            status_code=400,
            detail=f"page range exceeds max {MAX_PAGES_PER_REQUEST} pages per request",
        )
    if strategy not in ("embedded_jpeg", "pixmap_72dpi"):
        raise HTTPException(status_code=400, detail="strategy must be embedded_jpeg or pixmap_72dpi")

    raw = await file.read()
    if not raw:
        raise HTTPException(status_code=400, detail="empty pdf upload")

    pdf_path = _cache_pdf_bytes(raw)
    try:
        doc = fitz.open(pdf_path)
    except Exception as exc:
        raise HTTPException(status_code=400, detail=f"invalid pdf: {exc}") from exc

    try:
        if doc.xref_length() > MAX_PDF_OBJECTS:
            raise HTTPException(status_code=400, detail="pdf object count exceeds limit")

        pages_out: list[dict[str, Any]] = []
        total_bytes = 0
        end = min(page_end, len(doc))
        for page_num in range(page_start, end + 1):
            jpeg, mime, w, h = _render_page(doc, page_num, strategy)
            total_bytes += len(jpeg)
            if total_bytes > MAX_OUTPUT_BYTES:
                raise HTTPException(status_code=400, detail="rendered output exceeds byte limit")
            import base64

            pages_out.append(
                {
                    "page_number": page_num,
                    "mime_type": mime,
                    "width": w,
                    "height": h,
                    "bytes": len(jpeg),
                    "image_base64": base64.b64encode(jpeg).decode("ascii"),
                }
            )
    finally:
        doc.close()

    return JSONResponse(
        {
            "pages": pages_out,
            "stats": {
                "page_count": len(pages_out),
                "total_bytes": sum(p["bytes"] for p in pages_out),
                "strategy": strategy,
            },
        }
    )


if __name__ == "__main__":
    import uvicorn

    bind = os.environ.get("PDF_RENDERER_BIND", "127.0.0.1:9091")
    host, _, port_s = bind.partition(":")
    uvicorn.run(app, host=host or "127.0.0.1", port=int(port_s or 9091), log_level="info")
