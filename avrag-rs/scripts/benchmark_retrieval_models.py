#!/usr/bin/env python3
"""对比实测：SiliconFlow vs 百炼（DashScope）embedding / rerank 延迟与吞吐。

对比对（键名从 avrag-rs/.env 读取，脚本自身解析 .env，绝不打印密钥）：
  text embedding : SF Pro/BAAI/bge-m3            vs 百炼 $EMBEDDING_MODEL（OpenAI 兼容 /embeddings）
  mm  embedding  : SF Qwen/Qwen3-VL-Embedding-8B vs 百炼 $MM_EMBEDDING_MODEL（DashScope 原生 contents）
  rerank         : SF Pro/BAAI/bge-reranker-v2-m3 + Qwen/Qwen3-VL-Reranker-8B
                   vs 百炼 $RERANK_MODEL（DashScope 原生 text-rerank）

用法:
  python3 avrag-rs/scripts/benchmark_retrieval_models.py [--suite text,mm,rerank,all]
      [--repeat 5] [--doc-chunks 100] [--skip-doc-scale]

口径：每组 1 次预热 + N 次计时，报 min/mean/max；doc-scale 模拟生产
batch=10 串行模式（embedding.rs TEXT_EMBEDDING_BATCH_SIZE）测整文档墙钟。
"""
from __future__ import annotations

import argparse
import base64
import json
import statistics
import struct
import sys
import time
import urllib.error
import urllib.request
import zlib
from pathlib import Path

ENV_PATH = Path(__file__).resolve().parents[1] / ".env"
SF_BASE = "https://api.siliconflow.cn/v1"
SF_TEXT_MODEL = "Pro/BAAI/bge-m3"
SF_VL_EMBED_MODEL = "Qwen/Qwen3-VL-Embedding-8B"
SF_RERANK_MODELS = ["Pro/BAAI/bge-reranker-v2-m3", "Qwen/Qwen3-VL-Reranker-8B"]
BATCH = 10  # 生产 TEXT_EMBEDDING_BATCH_SIZE


def load_env(path: Path) -> dict[str, str]:
    out: dict[str, str] = {}
    for line in path.read_text().splitlines():
        line = line.strip()
        if line and not line.startswith("#") and "=" in line:
            k, _, v = line.partition("=")
            out[k.strip()] = v.strip()
    return out


def post_json(url: str, key: str, body: dict, timeout: float = 60.0) -> tuple[dict, float]:
    """POST JSON，返回 (parsed_body, wall_seconds)；HTTP/网络错误抛 RuntimeError。"""
    req = urllib.request.Request(
        url,
        data=json.dumps(body).encode(),
        headers={
            "Authorization": f"Bearer {key}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    start = time.perf_counter()
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            payload = json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        detail = e.read().decode(errors="replace")[:300]
        raise RuntimeError(f"HTTP {e.code}: {detail}") from e
    except Exception as e:  # noqa: BLE001 - 网络层错误原样上报
        raise RuntimeError(f"{type(e).__name__}: {e}") from e
    return payload, time.perf_counter() - start


# ---------- 载荷 ----------

BASE_PARA = (
    "本项目采用两台4T/H与一台3T/H生物质锅炉并联供汽，年运行时间约六千小时。"
    "制粒车间配置粉碎机、烘干机与制粒机各四台，输送与除尘系统按防火防爆规范设计。"
    "原料以木屑与秸秆为主，含水率控制在百分之十二以下，成品颗粒热值不低于四千大卡。"
    "公用工程包括循环水、压缩空气与变配电系统，自控系统采用集散控制架构。"
)


def make_chunks(n: int, chars: int) -> list[str]:
    out = []
    for i in range(n):
        text = f"[块{i:03d}]" + BASE_PARA
        while len(text) < chars:
            text += BASE_PARA[(i * 7) % len(BASE_PARA):] + BASE_PARA
        out.append(text[:chars])
    return out


def make_png_b64(size: int = 256) -> str:
    """stdlib 生成一张确定性噪点 PNG（LCG），base64 返回。~几百 KB 级真实感。"""
    def chunk(tag: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data))

    seed = 42
    rows = bytearray()
    for y in range(size):
        rows.append(0)
        for x in range(size):
            seed = (seed * 1103515245 + 12345) & 0x7FFFFFFF
            v = (seed >> 8) & 0xFF
            rows += bytes(((x + v) % 256, (y + v) % 256, v))
    ihdr = struct.pack(">IIBBBBB", size, size, 8, 2, 0, 0, 0)
    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(bytes(rows))) + chunk(b"IEND", b"")
    return base64.b64encode(png).decode()


# ---------- 各平台调用 ----------

def bl_text_embed(env, texts):
    return post_json(
        env["EMBEDDING_BASE_URL"].rstrip("/") + "/embeddings",
        env["EMBEDDING_API_KEY"],
        {"model": env["EMBEDDING_MODEL"], "input": texts, "dimensions": int(env.get("EMBEDDING_DIMENSIONS", "1024"))},
    )


def sf_text_embed(env, texts, model=SF_TEXT_MODEL):
    return post_json(SF_BASE + "/embeddings", env["SILICONFLOW_API_KEY"], {"model": model, "input": texts})


def bl_mm_embed(env, text, image_b64=None):
    contents = ([{"image": f"data:image/png;base64,{image_b64}"}] if image_b64 else []) + [{"text": text}]
    return post_json(
        env["MM_EMBEDDING_BASE_URL"],
        env["MM_EMBEDDING_API_KEY"],
        {
            "model": env["MM_EMBEDDING_MODEL"],
            "input": {"contents": contents},
            "parameters": {"output_type": "dense", "dimension": int(env.get("MM_EMBEDDING_DIMENSIONS", "1024"))},
        },
    )


def sf_vl_embed(env, text, image_b64=None):
    # VL Embedding 请求：文本用字符串；图文混合按 contents 风格对象数组试。
    inp: object = text if image_b64 is None else [{"image": f"data:image/png;base64,{image_b64}"}, {"text": text}]
    return post_json(SF_BASE + "/embeddings", env["SILICONFLOW_API_KEY"], {"model": SF_VL_EMBED_MODEL, "input": inp})


def bl_rerank(env, query, docs):
    return post_json(
        env["RERANK_BASE_URL"],
        env["RERANK_API_KEY"],
        {
            "model": env["RERANK_MODEL"],
            "input": {"query": {"text": query}, "documents": [{"text": d} for d in docs]},
            "parameters": {"return_documents": False, "top_n": min(10, len(docs))},
        },
    )


def sf_rerank(env, query, docs, model):
    return post_json(
        SF_BASE + "/rerank",
        env["SILICONFLOW_API_KEY"],
        {"model": model, "query": query, "documents": docs, "top_n": min(10, len(docs))},
    )


# ---------- 计时驱动 ----------

def timed(label: str, fn, repeat: int, rows: list):
    try:
        fn()  # 预热（不计时）
    except RuntimeError as e:
        rows.append((label, 0, [], f"预热失败: {e}"))
        return
    samples = []
    note = ""
    for _ in range(repeat):
        try:
            _, dt = fn()
            samples.append(dt)
        except RuntimeError as e:
            note = f"部分失败: {e}"
            break
        time.sleep(0.2)
    rows.append((label, len(samples), samples, note))


def report(title: str, rows: list) -> None:
    print(f"\n### {title}\n")
    print("| 用例 | 成功次数 | min | mean | max | 备注 |")
    print("|---|---|---|---|---|---|")
    for label, n, samples, note in rows:
        if samples:
            print(f"| {label} | {n} | {min(samples):.2f}s | {statistics.mean(samples):.2f}s | {max(samples):.2f}s | {note} |")
        else:
            print(f"| {label} | 0 | - | - | - | {note} |")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--suite", default="all", help="text,mm,rerank,all")
    ap.add_argument("--repeat", type=int, default=5)
    ap.add_argument("--doc-chunks", type=int, default=100)
    ap.add_argument("--skip-doc-scale", action="store_true")
    args = ap.parse_args()

    env = load_env(ENV_PATH)
    has_sf = bool(env.get("SILICONFLOW_API_KEY"))
    if not has_sf:
        print("!! SILICONFLOW_API_KEY 未配置——只跑百炼侧。", file=sys.stderr)

    suites = {"text", "mm", "rerank"} if args.suite == "all" else set(args.suite.split(","))
    r = args.repeat

    if "text" in suites:
        rows: list = []
        c800_1 = make_chunks(1, 800)
        c800_10 = make_chunks(10, 800)
        c2000_10 = make_chunks(10, 2000)
        timed(f"百炼 {env['EMBEDDING_MODEL']} 1×800字", lambda: bl_text_embed(env, c800_1), r, rows)
        timed(f"百炼 {env['EMBEDDING_MODEL']} 10×800字", lambda: bl_text_embed(env, c800_10), r, rows)
        timed(f"百炼 {env['EMBEDDING_MODEL']} 10×2000字", lambda: bl_text_embed(env, c2000_10), max(2, r - 2), rows)
        if has_sf:
            timed(f"SF {SF_TEXT_MODEL} 1×800字", lambda: sf_text_embed(env, c800_1), r, rows)
            timed(f"SF {SF_TEXT_MODEL} 10×800字", lambda: sf_text_embed(env, c800_10), r, rows)
            timed(f"SF {SF_TEXT_MODEL} 10×2000字", lambda: sf_text_embed(env, c2000_10), max(2, r - 2), rows)
        report("text embedding 单次延迟", rows)

        if not args.skip_doc_scale:
            n = args.doc_chunks
            docs = make_chunks(n, 800)
            rows = []

            def doc_scale(fn):
                start = time.perf_counter()
                for i in range(0, n, BATCH):
                    fn(docs[i:i + BATCH])
                return time.perf_counter() - start

            timed(f"百炼 {env['EMBEDDING_MODEL']} 整文档 {n}块串行batch10",
                  lambda: (lambda dt: (None, dt))(doc_scale(lambda t: bl_text_embed(env, t))), 1, rows)
            if has_sf:
                timed(f"SF {SF_TEXT_MODEL} 整文档 {n}块串行batch10",
                      lambda: (lambda dt: (None, dt))(doc_scale(lambda t: sf_text_embed(env, t))), 1, rows)
            report("text embedding 整文档吞吐（生产串行 batch=10 模式，含 1 次预热）", rows)

    if "mm" in suites:
        rows = []
        img = make_png_b64()
        caption = "厂区总平面布置图：制粒车间、原料库与成品库呈U形布置。"
        timed(f"百炼 {env['MM_EMBEDDING_MODEL']} 纯文本", lambda: bl_mm_embed(env, caption), r, rows)
        timed(f"百炼 {env['MM_EMBEDDING_MODEL']} 图+文", lambda: bl_mm_embed(env, caption, img), r, rows)
        if has_sf:
            timed(f"SF {SF_VL_EMBED_MODEL} 纯文本", lambda: sf_vl_embed(env, caption), r, rows)
            timed(f"SF {SF_VL_EMBED_MODEL} 图+文", lambda: sf_vl_embed(env, caption, img), r, rows)
        report("多模态 embedding 单次延迟", rows)

    if "rerank" in suites:
        rows = []
        query = "生物质锅炉的额定蒸发量与年运行时间是多少？"
        docs10 = make_chunks(10, 400)
        docs32 = make_chunks(32, 400)
        timed(f"百炼 {env['RERANK_MODEL']} 10文档", lambda: bl_rerank(env, query, docs10), r, rows)
        timed(f"百炼 {env['RERANK_MODEL']} 32文档", lambda: bl_rerank(env, query, docs32), max(2, r - 2), rows)
        if has_sf:
            for m in SF_RERANK_MODELS:
                timed(f"SF {m} 10文档", lambda m=m: sf_rerank(env, query, docs10, m), r, rows)
                timed(f"SF {m} 32文档", lambda m=m: sf_rerank(env, query, docs32, m), max(2, r - 2), rows)
        report("rerank 单次延迟", rows)


if __name__ == "__main__":
    main()
