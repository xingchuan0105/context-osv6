#!/usr/bin/env python3
"""One-shot triplet extraction probe on a short doc for prompt calibration.

Uses avrag-rs/.env INGESTION_LLM_* (OpenAI-compatible chat) + current
triplet-extraction.system.md, then applies host closed-set normalize (ontology 6).

Usage:
  python3 scripts/probe-triplet-ontology.py [path/to/short.md]
"""
from __future__ import annotations

import json
import os
import re
import sys
import uuid
from pathlib import Path

import urllib.request

ROOT = Path(__file__).resolve().parents[1]
ENV = ROOT / "avrag-rs" / ".env"
PROMPT = ROOT / "avrag-rs" / "prompts" / "pipeline" / "triplet-extraction.system.md"
USER_TMPL = ROOT / "avrag-rs" / "prompts" / "templates" / "triplet-extraction-user.tmpl"
DEFAULT_DOC = (
    ROOT
    / "avrag-rs/crates/app/tests/e2e_output/markitdown_out/adr-0004-rag-agent-loop.md.md"
)
OUT_DIR = ROOT / "output" / "runtime-logs"
OUT_DIR.mkdir(parents=True, exist_ok=True)

CANONICAL = {"类型", "部分", "参与", "依赖", "位于", "标识"}

# Compact synonym map mirroring predicate_normalize.rs (not full table).
SYNONYMS: list[tuple[list[str], str]] = [
    (
        [
            "类型",
            "属于",
            "隶属于",
            "is a",
            "is-a",
            "instance of",
            "type of",
            "belongs to",
            "member of",
        ],
        "类型",
    ),
    (
        [
            "部分",
            "part of",
            "包含",
            "包括",
            "contains",
            "includes",
            "has part",
            "组成于",
        ],
        "部分",
    ),
    (
        [
            "参与",
            "执行",
            "实现",
            "调用",
            "implements",
            "performs",
            "participates in",
            "撰写",
            "设计",
        ],
        "参与",
    ),
    (
        [
            "依赖",
            "depends on",
            "requires",
            "需要",
            "基于",
            "使用",
            "uses",
            "用于",
            "适用于",
            "支持",
        ],
        "依赖",
    ),
    (
        ["位于", "located in", "based in", "发生在", "发生于"],
        "位于",
    ),
    (
        [
            "标识",
            "标识为",
            "maps to",
            "映射到",
            "对应",
            "名为",
            "denotes",
        ],
        "标识",
    ),
]


def load_env(path: Path) -> dict[str, str]:
    out: dict[str, str] = {}
    if not path.is_file():
        return out
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        k, v = line.split("=", 1)
        out[k.strip()] = v.strip().strip('"').strip("'")
    return out


def normalize_predicate(p: str) -> tuple[str, str | None]:
    t = p.strip()
    if not t:
        return "", None
    if t in CANONICAL:
        return t, None
    key = t.lower()
    for variants, target in SYNONYMS:
        for v in variants:
            if v == t or v.lower() == key:
                return target, (None if target == t else t)
    # strict drop
    return "", t


def chunk_text(text: str, max_chars: int = 1200) -> list[tuple[str, str]]:
    """Return list of (chunk_id, text). Prefer ## sections."""
    parts = re.split(r"(?=^##\s)", text, flags=re.M)
    parts = [p.strip() for p in parts if p.strip()]
    if len(parts) <= 1:
        # fixed windows
        parts = [text[i : i + max_chars] for i in range(0, len(text), max_chars)]
    chunks = []
    for p in parts:
        if len(p) > max_chars * 2:
            for i in range(0, len(p), max_chars):
                chunks.append((str(uuid.uuid4()), p[i : i + max_chars]))
        else:
            chunks.append((str(uuid.uuid4()), p))
    return chunks[:6]  # keep probe small


def chat_complete(base: str, key: str, model: str, system: str, user: str) -> str:
    url = base.rstrip("/") + "/chat/completions"
    body = {
        "model": model,
        "temperature": 0.1,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ],
    }
    # dashscope qwen sometimes wants enable_thinking false via extra
    body["extra_body"] = {"enable_thinking": False}
    data = json.dumps(body).encode("utf-8")
    req = urllib.request.Request(
        url,
        data=data,
        headers={
            "Authorization": f"Bearer {key}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=120) as resp:
        payload = json.loads(resp.read().decode("utf-8"))
    return payload["choices"][0]["message"]["content"]


def strip_fence(s: str) -> str:
    s = s.strip()
    if s.startswith("```"):
        lines = s.splitlines()[1:]
        out = []
        for line in lines:
            if line.strip() == "```":
                break
            out.append(line)
        return "\n".join(out).strip()
    return s


def main() -> int:
    doc_path = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_DOC
    if not doc_path.is_file():
        print(f"missing doc: {doc_path}", file=sys.stderr)
        return 1

    env = load_env(ENV)
    base = env.get("INGESTION_LLM_BASE_URL") or env.get("OPENAI_BASE_URL")
    key = env.get("INGESTION_LLM_API_KEY") or env.get("DASHSCOPE_API_KEY")
    model = env.get("INGESTION_LLM_MODEL") or "qwen-plus"
    if not base or not key:
        print("need INGESTION_LLM_BASE_URL + API_KEY in avrag-rs/.env", file=sys.stderr)
        return 1

    system = PROMPT.read_text(encoding="utf-8")
    text = doc_path.read_text(encoding="utf-8")
    chunks = chunk_text(text)
    chunk_ids = [c[0] for c in chunks]
    chunks_json = json.dumps(
        {"chunks": [{"chunk_id": cid, "text": body} for cid, body in chunks]},
        ensure_ascii=False,
    )
    user = (
        USER_TMPL.read_text(encoding="utf-8")
        .replace("{chunk_ids}", ", ".join(chunk_ids))
        .replace("{chunks_json}", chunks_json)
    )

    print(f"[probe] doc={doc_path} chars={len(text)} chunks={len(chunks)} model={model}")
    raw = chat_complete(base, key, model, system, user)
    cleaned = strip_fence(raw)

    try:
        parsed = json.loads(cleaned)
    except json.JSONDecodeError as e:
        out_raw = OUT_DIR / "triplet_probe_raw.txt"
        out_raw.write_text(raw, encoding="utf-8")
        print(f"JSON parse failed: {e}; raw → {out_raw}")
        print(raw[:2000])
        return 1

    triples = parsed.get("triplets") or []
    kept = []
    dropped = []
    for t in triples:
        pred = t.get("predicate") or ""
        canon, orig = normalize_predicate(pred)
        row = {
            "subject": t.get("subject"),
            "predicate_raw": pred,
            "predicate": canon,
            "object": t.get("object"),
            "chunk_id": t.get("chunk_id"),
        }
        if not canon:
            dropped.append(row)
        else:
            if orig:
                row["normalized_from"] = orig
            kept.append(row)

    report = {
        "doc": str(doc_path),
        "model": model,
        "n_chunks": len(chunks),
        "n_raw": len(triples),
        "n_kept": len(kept),
        "n_dropped_unknown_pred": len(dropped),
        "predicate_hist": {},
        "kept": kept,
        "dropped": dropped,
        "raw_response": cleaned,
    }
    hist: dict[str, int] = {}
    for k in kept:
        hist[k["predicate"]] = hist.get(k["predicate"], 0) + 1
    report["predicate_hist"] = hist

    out_path = OUT_DIR / "triplet_probe_ontology.json"
    out_path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")

    print(f"[probe] raw={len(triples)} kept={len(kept)} dropped_pred={len(dropped)}")
    print(f"[probe] hist={hist}")
    print("--- kept triples ---")
    for k in kept:
        print(f"  ({k['subject']}) --{k['predicate']}--> ({k['object']})")
    if dropped:
        print("--- dropped (unknown pred after normalize) ---")
        for k in dropped[:20]:
            print(f"  ({k['subject']}) --{k['predicate_raw']}--> ({k['object']})")
    print(f"[probe] full report: {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
