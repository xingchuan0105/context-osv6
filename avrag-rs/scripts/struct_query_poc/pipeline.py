#!/usr/bin/env python3
"""struct_query 灌入 pipeline(P1a):doc.md → 提取 → 续表合并 → 校验 → per-doc DuckDB + 健康报告.

确定性层职责(对照 docs/plans/2026-07-31-struct-query-virtual-tables.md §3):
  ② markdown-it-py 提取 grids(带源行号)
  ③ 同表头签名相邻 grid 合并 + 跨页重复表头行剔除
  ④ 建表: 业务列 VARCHAR 原值 + row_ord + __src_line
  ⑤ 校验套件 → 健康报告(每表 checks + status)
  ⑦ _meta 落库(confidence 初值/notes); auto rotate_header(带守卫, 记 note, 待 supervision 复核)

不作: LLM 监督(P1b)、数值规整(v1.1)、chunk 证据映射(切块后接 __src_line 换算)。

CLI: pipeline.py <input.md> [--out <doc>.duckdb] [--doc-id <uuid>]
库:   from pipeline import run_pipeline
产物: <out> + <out>.evidence.json(每表 evidence_chunk_id + 表 md,供 PG chunks 证据装载)
"""
import json
import os
import re
import sys
import uuid
from dataclasses import dataclass, field

import duckdb
from markdown_it import MarkdownIt

NUM_RE = re.compile(r"^[+-]?\d+(\.\d+)?$")
TOTAL_LABEL_RE = re.compile(r"合计|总计|小计")
JUNK_CELL_RE = re.compile(r"^[-:\s]+$")
PAGE_NO_RE = re.compile(r"^\d{1,4}$")
PURE_NUM_RE = re.compile(r"^\d+$")


@dataclass
class Grid:
    start_line: int
    rows: list  # [{"line": int, "cells": [str]}]
    notes: list = field(default_factory=list)

    @property
    def header(self):
        return self.rows[0]["cells"] if self.rows else []

    @property
    def data(self):
        return self.rows[1:]


def extract_grids(md_text: str) -> list:
    """markdown-it-py gfm-like → grids,行带源行号(token.map)。"""
    tokens = MarkdownIt("gfm-like").parse(md_text)
    grids, cur, cur_cells, cur_line, in_table, t_start = [], None, None, 0, False, 0
    for t in tokens:
        if t.type == "table_open":
            in_table, cur, t_start = True, [], (t.map[0] if t.map else 0)
        elif t.type == "table_close":
            in_table = False
            if cur:
                grids.append(Grid(start_line=t_start, rows=cur))
        elif in_table and t.type == "tr_open":
            cur_cells, cur_line = [], (t.map[0] if t.map else 0)
        elif in_table and t.type == "tr_close":
            cur.append({"line": cur_line, "cells": [c.strip() for c in cur_cells]})
        elif in_table and t.type == "inline" and cur_cells is not None:
            cur_cells.append(t.content)
    return grids


def header_sig(cells) -> tuple:
    return tuple(c.strip() for c in cells)


def merge_continuations(grids: list) -> list:
    """同表头签名的后续 grid 并入首见 grid(跨页续表);数据行与表头相同者剔除(页重复表头)。"""
    merged = []
    by_sig = {}
    for g in grids:
        if not g.rows:
            continue
        sig = header_sig(g.header)
        if sig in by_sig:
            tgt = by_sig[sig]
            tgt.rows.extend(g.rows[1:])  # 丢掉续表重复表头
            tgt.notes.append(f"merged_continuation@{g.start_line}")
        else:
            by_sig[sig] = g
            merged.append(g)
    for g in merged:
        before = len(g.rows)
        g.rows = g.rows[:1] + [r for r in g.data if header_sig(r["cells"]) != header_sig(g.header)]
        if len(g.rows) != before:
            g.notes.append(f"dropped_repeated_header_x{before - len(g.rows)}")
        # 分隔行残迹(PDF 重建在中部再出 | --- | 行)→ 确定性剔除
        before = len(g.rows)
        g.rows = g.rows[:1] + [r for r in g.data if not all(JUNK_CELL_RE.match(c or "-") for c in r["cells"])]
        if len(g.rows) != before:
            g.notes.append(f"dropped_delimiter_artifact_x{before - len(g.rows)}")
    return merged


def auto_rotate(g: Grid):
    """假表头信号(列名 ^Unnamed 或全空)→ rotate_header(header_row=1),带守卫:
    仅当数据第 1 行非空单元格过半才提升;Unnamed 列仅当数据区全空才丢。"""
    hdr = g.header
    if not hdr or not any(re.match(r"^Unnamed", h) or h == "" for h in hdr):
        return
    if len(g.data) < 1:
        return
    first = g.data[0]["cells"]
    nonempty = sum(1 for c in first if c)
    if nonempty <= len(hdr) / 2:
        return  # 守卫: 数据第 1 行不像真表头
    n = len(hdr)
    keep = [
        i for i in range(n)
        if not (re.match(r"^Unnamed", hdr[i]) and all(i >= len(r["cells"]) or r["cells"][i] == "" for r in g.data))
    ]
    g.rows = [g.rows[1]] + [
        {"line": r["line"], "cells": [r["cells"][i] if i < len(r["cells"]) else "" for i in keep]}
        for r in g.rows[2:]
    ]
    g.notes.append("auto:rotate_header(header_row=1)")


def to_num(s: str):
    v = s.replace(",", "").replace("，", "").strip()
    return float(v) if NUM_RE.match(v) else None


def checks_for(g: Grid) -> list:
    checks = []
    hdr, data = g.header, g.data
    n_cols = len(hdr)

    unnamed = [h for h in hdr if re.match(r"^Unnamed", h) or h == ""]
    checks.append({"name": "header_suspicious", "passed": not unnamed,
                   "detail": f"列名可疑: {unnamed}" if unnamed else ""})

    # banner 数字混进表头行(白药 638 案例: 数字在表头、标签在首行数据)
    num_hdrs = [h for h in hdr if PURE_NUM_RE.match(h)]
    checks.append({"name": "header_numeric_banner", "passed": not num_hdrs,
                   "detail": f"表头含纯数字列名: {num_hdrs}(疑似 banner/数据行混入表头)" if num_hdrs else ""})

    ragged = [r["line"] for r in data if len(r["cells"]) != n_cols]
    checks.append({"name": "column_count", "passed": not ragged,
                   "detail": f"{len(ragged)} 行列数不符, 源行 {ragged[:5]}" if ragged else ""})

    empty_rows = [r["line"] for r in data if all(c == "" for c in r["cells"])]
    checks.append({"name": "empty_rows", "passed": not empty_rows,
                   "detail": f"{len(empty_rows)} 全空行, 源行 {empty_rows[:5]}" if empty_rows else ""})

    empty_cols = [hdr[i] or f"col_{i}" for i in range(n_cols)
                  if data and all(i >= len(r["cells"]) or r["cells"][i] == "" for r in data)]
    checks.append({"name": "empty_columns", "passed": not empty_cols,
                   "detail": f"全空列: {empty_cols}" if empty_cols else ""})

    if data:
        col0 = [r["cells"][0] for r in data if r["cells"]]
        ints = [int(c) for c in col0 if c.isdigit()]
        if len(ints) == len(col0) and ints:
            lo, hi = min(ints), max(ints)
            ok = hi - lo + 1 == len(ints) == len(set(ints))
            checks.append({"name": "sequence", "passed": ok,
                           "detail": f"序号 {lo}..{hi}, count={len(ints)}" + ("" if ok else " 断号/重复")})

    total_rows = [(idx, r) for idx, r in enumerate(data) if r["cells"] and TOTAL_LABEL_RE.search(r["cells"][0])]
    for _, tr in total_rows[:1]:
        detail_rows = [r for j, r in enumerate(data) if not (r["cells"] and TOTAL_LABEL_RE.search(r["cells"][0]))]
        bad = []
        for i in range(1, n_cols):
            tv = to_num(tr["cells"][i]) if i < len(tr["cells"]) else None
            if tv is None:
                continue
            s = sum(v for v in (to_num(r["cells"][i]) if i < len(r["cells"]) else None for r in detail_rows) if v is not None)
            if abs(s - tv) > max(abs(tv) * 0.001, 1e-6):
                bad.append(f"{hdr[i] or f'col_{i}'}: sum={s} != 合计={tv}")
        checks.append({"name": "total_reconcile", "passed": not bad,
                       "detail": "; ".join(bad) if bad else "合计对账一致"})
    return checks


def sanitize_headers(hdr: list) -> list:
    out, seen = [], {}
    for i, h in enumerate(hdr):
        name = h.strip() or f"col_{i}"
        if name in seen:
            seen[name] += 1
            name = f"{name}_{seen[name]}"
        else:
            seen[name] = 1
        out.append(name)
    return out


def quote_ident(name: str) -> str:
    return '"' + name.replace('"', '""') + '"'


def render_table_md(headers: list, rows: list) -> str:
    """表格 → pipe md（证据 chunk 内容；单元格 `|` 转义、换行压平）。"""

    def esc(c) -> str:
        return str(c).replace("|", "\\|").replace("\n", " ").strip()

    lines = [
        "| " + " | ".join(esc(h) for h in headers) + " |",
        "| " + " | ".join("---" for _ in headers) + " |",
    ]
    for r in rows:
        lines.append("| " + " | ".join(esc(c) for c in r) + " |")
    return "\n".join(lines)


def prepare(md_path: str):
    """提取 + 合并 + auto_rotate → (grids, 原文)。supervision loop 与 CLI 共用。"""
    text = open(md_path, encoding="utf-8").read()
    grids = merge_continuations(extract_grids(text))
    for g in grids:
        auto_rotate(g)
    return grids, text


def table_report(idx: int, g: Grid) -> dict:
    checks = checks_for(g)
    status = "high_candidate" if all(c["passed"] for c in checks) else "needs_diagnosis"
    return {
        "table_id": f"t{idx}", "start_line": g.start_line, "headers": sanitize_headers(g.header),
        "n_rows": len(g.data), "status": status,
        "checks": [c for c in checks if not c["passed"]] or "all_passed",
        "checks_full": checks,
        "notes": g.notes,
    }


def write_duckdb(grids: list, metas: list, out_path: str):
    """grids + 每表 meta(caption/unit/table_kind/confidence/status/checks/notes/excluded)写 per-doc 库。
    excluded(quarantine)的表不写入——查询侧 catalog 自然不可见。
    每表生成 evidence_chunk_id(UUID)入 _meta,并把「入库后内容」渲染成 md 作为
    证据 chunk 文本返回(→ PG chunks 表,chunk_type='table_evidence',仅水合不进检索面)。"""
    if os.path.exists(out_path):
        os.remove(out_path)
    con = duckdb.connect(out_path)
    try:
        con.execute("INSTALL fts")
        con.execute("LOAD fts")
    except Exception:
        pass  # 老版本/离线: FTS 索引缺失,查询侧 match_bm25 报 schema 不存在(有容错)
    con.execute("CREATE TABLE _meta (table_name VARCHAR, caption VARCHAR, unit VARCHAR, table_kind VARCHAR, "
                "confidence VARCHAR, start_line INTEGER, n_rows INTEGER, n_cols INTEGER, status VARCHAR, "
                "checks JSON, notes JSON, evidence_chunk_id VARCHAR)")
    evidence = []
    for idx, (g, m) in enumerate(zip(grids, metas)):
        if m.get("excluded"):
            continue
        name = f"t{idx}"
        hdr = sanitize_headers(g.header)
        cols = ", ".join(f"{quote_ident(h)} VARCHAR" for h in hdr)
        con.execute(f'CREATE TABLE {quote_ident(name)} (row_ord INTEGER, __src_line INTEGER, {cols})')
        rows = [[r["cells"][i] if i < len(r["cells"]) else "" for i in range(len(hdr))] for r in g.data]
        if rows:
            con.executemany(
                f'INSERT INTO {quote_ident(name)} VALUES ({", ".join(["?"] * (len(hdr) + 2))})',
                [[j, r["line"]] + row for j, (r, row) in enumerate(zip(g.data, rows))],
            )
        chunk_id = str(uuid.uuid4())
        evidence.append({"chunk_id": chunk_id, "table": name, "start_line": g.start_line,
                         "n_rows": len(g.data), "md": render_table_md(hdr, rows)})
        con.execute("INSERT INTO _meta VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    [name, m.get("caption"), m.get("unit"), m.get("table_kind"), m.get("confidence"),
                     g.start_line, len(g.data), len(hdr), m["status"],
                     json.dumps(m["checks"], ensure_ascii=False), json.dumps(g.notes, ensure_ascii=False),
                     chunk_id])
        # FTS 索引（fts 表内值发现；与 struct-supervision Rust 产物对齐）。
        # 查询侧: SELECT * FROM {name} WHERE fts_main_{name}.match_bm25(row_ord, 'x') IS NOT NULL
        try:
            col_list = ", ".join(f"'{h.replace(chr(39), chr(39)*2)}'" for h in hdr)
            con.execute(f"PRAGMA create_fts_index('{name}', 'row_ord', {col_list})")
        except Exception as e:
            print(f"WARN: create_fts_index({name}) failed: {e}")
    con.close()
    return evidence


def run_pipeline(md_path: str, out_path: str, doc_id: str | None = None) -> dict:
    grids, _ = prepare(md_path)
    reports = [table_report(i, g) for i, g in enumerate(grids)]
    metas = [{"status": r["status"], "checks": r["checks_full"]} for r in reports]
    evidence = write_duckdb(grids, metas, out_path)
    # 证据 chunk sidecar:doc_id 默认取 out 文件名干(<doc_id>.duckdb 约定)。
    doc_id = doc_id or os.path.splitext(os.path.basename(out_path))[0]
    sidecar = out_path + ".evidence.json"
    with open(sidecar, "w", encoding="utf-8") as f:
        json.dump({"doc_id": doc_id, "chunks": evidence}, f, ensure_ascii=False, indent=1)
    return {"doc": md_path, "duckdb": out_path, "evidence": sidecar,
            "tables": [{k: v for k, v in r.items() if k != "checks_full"} for r in reports]}


def main() -> int:
    md_path = sys.argv[1]
    out = sys.argv[sys.argv.index("--out") + 1] if "--out" in sys.argv else "/tmp/struct_poc_doc.duckdb"
    doc_id = sys.argv[sys.argv.index("--doc-id") + 1] if "--doc-id" in sys.argv else None
    # --emit-grids <path>: 导出与 prepare() 一致的 grids JSON 中间表示
    # (struct-supervision Rust 侧的输入;schema 见 docs/plans/2026-07-31-struct-query-p2-handoff.md S0)。
    emit_grids = sys.argv[sys.argv.index("--emit-grids") + 1] if "--emit-grids" in sys.argv else None
    if emit_grids:
        grids, text = prepare(md_path)
        payload = {
            "doc_id": doc_id,
            "source_text": text,
            "grids": [
                {"start_line": g.start_line, "notes": g.notes,
                 "rows": [{"line": r["line"], "cells": r["cells"]} for r in g.rows]}
                for g in grids
            ],
        }
        with open(emit_grids, "w", encoding="utf-8") as f:
            json.dump(payload, f, ensure_ascii=False, indent=1)
    rep = run_pipeline(md_path, out, doc_id)
    print(json.dumps(rep, ensure_ascii=False, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
