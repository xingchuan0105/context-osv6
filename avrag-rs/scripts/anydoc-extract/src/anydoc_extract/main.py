#!/usr/bin/env python3
"""anydoc 子进程入口（avrag-rs Anydoc 后端）。

用法:
    anydoc-extract <input> <output.md>

将任意 anydoc 支持的文档（非 PDF 由产品路由保证）转为 GFM markdown。
失败：非零退出 + stderr（hard-fail，无降级）。

环境:
    无（超时由 Rust 侧 ANYDOC_TIMEOUT_MS 管）
"""
from __future__ import annotations

import sys
from pathlib import Path


def convert(src: Path) -> str:
    import anydoc

    return anydoc.to_markdown(str(src))


def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    if len(args) != 2:
        print("usage: anydoc-extract <input> <output.md>", file=sys.stderr)
        return 2
    src = Path(args[0])
    out = Path(args[1])
    if not src.is_file():
        print(f"anydoc-extract: input not found: {src}", file=sys.stderr)
        return 1
    try:
        md = convert(src)
    except Exception as exc:  # noqa: BLE001 — surface anydoc errors to worker
        print(f"anydoc-extract: {type(exc).__name__}: {exc}", file=sys.stderr)
        return 1
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(md, encoding="utf-8")
    print(f"[anydoc-extract] {src} -> {out} ({len(md)} chars)", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
