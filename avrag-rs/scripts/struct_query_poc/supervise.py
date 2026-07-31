#!/usr/bin/env python3
"""supervision loop v1(P1b):6 工具薄 loop,消费健康报告,产出语义标注/修复/终态.

架构(docs/plans/2026-07-31-struct-query-virtual-tables.md §4):
  prompts: prompts/pipeline/table-supervision/(system + obs 模板,第三人称)
  安全: 指令过 schema + 确定性守卫 + SQL 复验;LLM 永不提供单元格值;
        confidence=high 仅当全部校验通过(守卫,不用嘴宣布真理);
        quarantine 的表不写入 duckdb。
  LLM: INGESTION_LLM_*(.env, OpenAI 兼容);stdlib urllib,零新依赖。

CLI: supervise.py <input.md> --out <doc>.duckdb [--report sup.json] [--max-turns 40] [--dry-run]
  --dry-run: 不调 LLM,只打印简报(验证 briefing 组装)。
"""
import json
import os
import re
import sys
import urllib.request

import duckdb

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pipeline

PROMPTS = os.path.join(os.path.dirname(os.path.abspath(__file__)),
                       "..", "..", "prompts", "pipeline", "table-supervision")
CELL_BR = 60       # 简报单元格截断
CELL_SLICE = 200   # 切片单元格截断
MAX_SLICE_ROWS = 40
MAX_CHECK_ROWS = 50

FORBIDDEN_SQL = re.compile(r"\b(attach|copy|read_csv|read_json|insert|update|delete|create|drop|alter|pragma|set|install|load)\b", re.I)


def env_cfg() -> dict:
    cfg = {}
    for line in open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".env")):
        m = re.match(r"^([A-Z_]+)=(.*)$", line.strip())
        if m:
            cfg[m.group(1)] = m.group(2)
    return {"base_url": cfg["INGESTION_LLM_BASE_URL"].rstrip("/"),
            "api_key": cfg["INGESTION_LLM_API_KEY"],
            "model": cfg.get("INGESTION_LLM_MODEL", "deepseek-v4-flash")}


def llm_chat(cfg: dict, messages: list, tools: list) -> dict:
    body = {"model": cfg["model"], "messages": messages, "tools": tools,
            "temperature": 0.2, "tool_choice": "auto"}
    req = urllib.request.Request(
        cfg["base_url"] + "/v1/chat/completions",
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json", "Authorization": "Bearer " + cfg["api_key"]})
    with urllib.request.urlopen(req, timeout=180) as resp:
        return json.loads(resp.read())["choices"][0]["message"]


def clip(s: str, n: int) -> str:
    s = str(s)
    return s if len(s) <= n else s[:n] + "…"


TOOL_SCHEMAS = [
    {"type": "function", "function": {"name": "annotate", "description": "批量语义标注并给出终态置信度",
        "parameters": {"type": "object", "properties": {"tables": {"type": "array", "items": {"type": "object",
            "properties": {"table_id": {"type": "string"}, "caption": {"type": "string"},
                "unit": {"type": "string"}, "column_semantics": {"type": "object"},
                "table_kind": {"type": "string", "enum": ["detail", "summary", "kv", "layout"]},
                "confidence": {"type": "string", "enum": ["high", "low"]}},
            "required": ["table_id", "table_kind", "confidence"]}}}, "required": ["tables"]}}},
    {"type": "function", "function": {"name": "fetch_slice", "description": "取表的有界切片",
        "parameters": {"type": "object", "properties": {"table_id": {"type": "string"},
            "row_range": {"type": "array", "items": {"type": "integer"}},
            "source_lines": {"type": "array", "items": {"type": "integer"}}}, "required": ["table_id"]}}},
    {"type": "function", "function": {"name": "run_check", "description": "在表存储上跑只读校验 SQL",
        "parameters": {"type": "object", "properties": {"sql": {"type": "string"}}, "required": ["sql"]}}},
    {"type": "function", "function": {"name": "apply_directive", "description": "应用修复指令并重跑复验",
        "parameters": {"type": "object", "properties": {"table_id": {"type": "string"},
            "directive": {"type": "object"}}, "required": ["table_id", "directive"]}}},
    {"type": "function", "function": {"name": "quarantine", "description": "隔离表(不入查询侧)",
        "parameters": {"type": "object", "properties": {"table_id": {"type": "string"},
            "reason": {"type": "string"}}, "required": ["table_id", "reason"]}}},
    {"type": "function", "function": {"name": "done", "description": "全部表有终态后结束",
        "parameters": {"type": "object", "properties": {"summary": {"type": "string"}}}}},
]


class Session:
    def __init__(self, md_path: str):
        self.grids, self.text = pipeline.prepare(md_path)
        self.lines = self.text.splitlines()
        self.reports = {f"t{i}": pipeline.table_report(i, g) for i, g in enumerate(self.grids)}
        self.final = {}          # table_id -> annotation/quarantine
        self.log = []            # (tool, args, outcome_brief)
        self.con = None
        self._rebuild_db()

    def _rebuild_db(self):
        if self.con:
            self.con.close()
        self.con = duckdb.connect()
        for i, g in enumerate(self.grids):
            hdr = pipeline.sanitize_headers(g.header)
            cols = ", ".join(f"{pipeline.quote_ident(h)} VARCHAR" for h in hdr)
            self.con.execute(f'CREATE TABLE t{i} (row_ord INTEGER, {cols})')
            self.con.executemany(
                f'INSERT INTO t{i} VALUES ({", ".join(["?"] * (len(hdr) + 1))})',
                [[j] + [r["cells"][k] if k < len(r["cells"]) else "" for k in range(len(hdr))]
                 for j, r in enumerate(g.data)])

    # ---------- briefing ----------
    def briefing(self) -> str:
        parts = [f'文档「{os.path.basename(self.grids and sys.argv[1] or "")}」的表格提取与校验已完成。'
                 f"共 {len(self.grids)} 张表。校验由 SQL 确定性执行,其数值即事实。\n"]
        for tid, r in self.reports.items():
            g = self.grids[int(tid[1:])]
            parts.append(f"---\n表 {tid} | {len(r['headers'])} 列 × {r['n_rows']} 行 | 状态:{r['status']}")
            parts.append(f"表头:{r['headers']}")
            samples = g.data[:2] + (g.data[-1:] if r["n_rows"] > 2 else [])
            for s in samples:
                parts.append("  采样: " + " | ".join(clip(c, CELL_BR) for c in s["cells"]))
            if r["checks"] == "all_passed":
                parts.append("校验:全部通过")
            else:
                for c in r["checks"]:
                    parts.append(f"校验失败:{c['name']} — {c['detail']}")
            if g.notes:
                parts.append(f"管线备注:{g.notes}")
        parts.append("---\n状态为「待诊断」的表存在至少一项失败校验。全部表给出终态(high/low/quarantine)"
                     "并完成语义标注后调用 done。")
        return "\n".join(parts)

    # ---------- tools ----------
    def t_annotate(self, args: dict) -> str:
        out = []
        for t in args.get("tables", []):
            tid = t.get("table_id", "")
            if tid not in self.reports:
                out.append(f"{tid}: 不存在,标注未记录")
                continue
            if self.final.get(tid, {}).get("excluded"):
                out.append(f"{tid}: 已处于隔离/排除终态,标注未生效(终态不被后续标注覆盖)")
                continue
            failing = self.reports[tid]["checks"] != "all_passed"
            if t.get("confidence") == "high" and failing:
                out.append(f"{tid}: 校验未全部通过,confidence=high 未生效(守卫);请以 low 终态或先修复")
                continue
            self.final[tid] = {**t, "excluded": False}
            out.append(f"{tid}: 已标注 table_kind={t.get('table_kind')}, confidence={t.get('confidence')}")
        return "\n".join(out) or "未提供 tables"

    def t_fetch_slice(self, args: dict) -> str:
        tid = args.get("table_id", "")
        if tid not in self.reports:
            return f"{tid}: 不存在"
        g = self.grids[int(tid[1:])]
        if args.get("source_lines"):
            a, b = (args["source_lines"] + [0, 0])[:2]
            a, b = max(1, a), min(len(self.lines), b or a + MAX_SLICE_ROWS)
            rows = [f"L{i + 1}: {clip(self.lines[i], CELL_SLICE)}" for i in range(a - 1, min(b, a - 1 + MAX_SLICE_ROWS))]
            return f"源行 {a}–{b}(共 {len(self.lines)} 行)原文切片;未覆盖部分仍处于未观察状态:\n" + "\n".join(rows)
        a, b = (args.get("row_range") or [0, MAX_SLICE_ROWS])[:2]
        data = g.data[a:b or a + MAX_SLICE_ROWS]
        rows = [f"row {a + j}: " + " | ".join(clip(c, CELL_SLICE) for c in r["cells"]) for j, r in enumerate(data)]
        return f"表 {tid} 第 {a}–{a + len(data)} 行(共 {len(g.data)} 行)切片;未覆盖行仍处于未观察状态:\n" + "\n".join(rows)

    def t_run_check(self, args: dict) -> str:
        sql = (args.get("sql") or "").strip().rstrip(";")
        if not sql.lower().startswith("select") or FORBIDDEN_SQL.search(sql):
            return f"校验 SQL 未通过只读守卫,未执行:{clip(sql, 120)}"
        try:
            rows = self.con.execute(sql).fetchall()
        except Exception as e:
            return f"SQL 执行失败:{e}"
        trunc = len(rows) > MAX_CHECK_ROWS
        body = "\n".join(clip(str(r), 300) for r in rows[:MAX_CHECK_ROWS])
        return f"run_check 完成,返回 {min(len(rows), MAX_CHECK_ROWS)} 行" + \
               ("(已截断)" if trunc else "") + f":\n{body or '(空结果)'}"

    # ---------- directives ----------
    def t_apply_directive(self, args: dict) -> str:
        tid = args.get("table_id", "")
        d = args.get("directive") or {}
        action = d.get("action", "")
        if tid not in self.reports:
            return f"指令未通过校验,未被应用。表 {tid} 不存在。"
        g = self.grids[int(tid[1:])]
        ok, reason = self._apply(g, tid, action, d)
        if not ok:
            return f"指令 {action} 未通过校验,未被应用。表 {tid} 状态未变。\n拒绝原因:{reason}"
        self.reports[tid] = pipeline.table_report(int(tid[1:]), g)
        self._rebuild_db()
        r = self.reports[tid]
        checks = "全部通过" if r["checks"] == "all_passed" else \
            "; ".join(f"{c['name']}: {c['detail']}" for c in r["checks"])
        return (f"指令 {action} 已通过 schema 校验与确定性守卫,应用于表 {tid};确定性重跑已完成。\n"
                f"新健康报告:{len(r['headers'])} 列 × {r['n_rows']} 行 | 状态:{r['status']}\n"
                f"表头:{r['headers']}\n校验:{checks}")

    def _apply(self, g, tid, action, d):
        if action == "rotate_header":
            hr = int(d.get("header_row", 1))
            if hr < 1 or hr > len(g.data):
                return False, f"header_row={hr} 超出数据行范围(1..{len(g.data)})"
            pat = d.get("drop_columns_matching")
            hdr = g.header
            keep = list(range(len(hdr)))
            if pat:
                keep = [i for i in range(len(hdr)) if not (
                    re.match(pat, hdr[i]) and
                    all(i >= len(r["cells"]) or r["cells"][i] == "" for r in g.data))]
                if len(keep) < len(hdr) and not keep:
                    return False, "守卫:drop 后无剩余列"
            body = g.rows[1:]
            new_header = {"line": body[hr - 1]["line"],
                          "cells": [body[hr - 1]["cells"][i] if i < len(body[hr - 1]["cells"]) else "" for i in keep]}
            g.rows = [new_header] + [
                {"line": r["line"], "cells": [r["cells"][i] if i < len(r["cells"]) else "" for i in keep]}
                for r in body[hr:]]
            g.notes.append(f"directive:rotate_header(header_row={hr})")
            return True, ""
        if action == "set_header":
            headers = d.get("headers") or []
            ev = int(d.get("evidence_source_line", 0))
            if len(headers) != len(g.header):
                return False, f"headers 数({len(headers)}) != 现列数({len(g.header)})"
            line = self.lines[ev - 1] if 1 <= ev <= len(self.lines) else ""
            missing = [h for h in headers if h not in line]
            if missing:
                return False, f"守卫:{missing} 未出现在证据行 L{ev}"
            g.rows[0] = {"line": g.rows[0]["line"], "cells": [str(h) for h in headers]}
            g.notes.append(f"directive:set_header(evidence=L{ev})")
            return True, ""
        if action == "merge_tables":
            ids = d.get("table_ids") or []
            if tid not in ids:
                ids = [tid] + ids
            if len(ids) < 2:
                return False, "merge_tables 需要 ≥2 个 table_id"
            tgt = self.grids[int(ids[0][1:])]
            sig = pipeline.header_sig(tgt.header)
            for other in ids[1:]:
                if other not in self.reports:
                    return False, f"{other} 不存在"
                og = self.grids[int(other[1:])]
                if pipeline.header_sig(og.header) != sig:
                    return False, f"守卫:{other} 表头签名不一致"
                tgt.rows.extend(og.rows[1:])
                self.final[other] = {"table_id": other, "excluded": True,
                                     "reason": f"merged_into {ids[0]}"}
                self.reports.pop(other, None)
            tgt.notes.append(f"directive:merge_tables({ids[1:]})")
            return True, ""
        if action == "reparse_region":
            a, b = int(d.get("start_line", 0)), int(d.get("end_line", 0))
            if not (1 <= a < b <= len(self.lines)):
                return False, f"行区间 L{a}–L{b} 无效(全文 1..{len(self.lines)})"
            rows = []
            for i in range(a - 1, b):
                ln = self.lines[i]
                if not ln.strip().startswith("|"):
                    continue
                cells = [c.strip() for c in re.split(r"(?<!\\)\|", ln)[1:-1]]
                if cells and not all(re.match(r"^[-:\s]*$", c) for c in cells):
                    rows.append({"line": i + 1, "cells": cells})
            if len(rows) < 2:
                return False, f"区域 L{a}–L{b} 未解析出 ≥2 行管道行"
            g.rows = [rows[0]] + rows[1:]
            g.start_line = a
            g.notes.append(f"directive:reparse_region(L{a}-L{b})")
            return True, ""
        if action == "exclude":
            self.final[tid] = {"table_id": tid, "excluded": True, "reason": d.get("reason", "")}
            return True, ""
        return False, f"未知 action:{action}"

    def t_quarantine(self, args: dict) -> str:
        tid = args.get("table_id", "")
        if tid not in self.reports:
            return f"{tid}: 不存在"
        self.final[tid] = {"table_id": tid, "excluded": True, "reason": args.get("reason", "")}
        return f"{tid}: 已隔离,原因:{args.get('reason', '')}。该表不出现在查询侧 catalog。"

    def unfinished(self):
        return [tid for tid in self.reports if tid not in self.final]


def render_obs(tool: str, result: str) -> str:
    return result  # obs 模板即第三人称回传文本本身(见各 t_* 返回)


def supervise(md_path: str, out_path: str, report_path: str = None, max_turns: int = 40,
              dry_run: bool = False) -> dict:
    s = Session(md_path)
    system = open(os.path.join(PROMPTS, "supervision.system.v1.md"), encoding="utf-8").read()
    messages = [{"role": "system", "content": system},
                {"role": "user", "content": s.briefing()}]
    if dry_run:
        print(messages[1]["content"])
        return {}

    cfg = env_cfg()
    turns = 0
    done_summary = None
    while turns < max_turns and done_summary is None:
        turns += 1
        msg = llm_chat(cfg, messages, TOOL_SCHEMAS)
        messages.append(msg)
        calls = msg.get("tool_calls") or []
        if not calls:
            # 模型未调工具:以 observation 提示当前未完成表数(第三人称)
            un = s.unfinished()
            if un:
                messages.append({"role": "user", "content":
                    f"本轮未发生工具调用。仍处于未终态的表:{un}(共 {len(un)} 张)。"})
            continue
        for call in calls:
            name = call["function"]["name"]
            try:
                args = json.loads(call["function"].get("arguments") or "{}")
            except json.JSONDecodeError:
                args = {}
            if name == "done":
                done_summary = args.get("summary", "")
                result = "监督结束。"
            else:
                fn = {"annotate": s.t_annotate, "fetch_slice": s.t_fetch_slice,
                      "run_check": s.t_run_check, "apply_directive": s.t_apply_directive,
                      "quarantine": s.t_quarantine}.get(name)
                result = fn(args) if fn else f"未知工具:{name}"
                s.log.append((name, args, result[:200]))
            messages.append({"role": "tool", "tool_call_id": call["id"], "content": result})
        un = s.unfinished()
        if done_summary is None and un and len(s.log) > 0 and turns % 8 == 0:
            messages.append({"role": "user", "content":
                f"进度观察:已进行 {turns} 轮;仍未终态的表:{un}。"})

    # 兜底终态:未处理表保持确定性初态并附说明(pipeline 不被 LLM 卡死)
    for tid in s.unfinished():
        r = s.reports[tid]
        s.final[tid] = {"table_id": tid, "excluded": False, "confidence": "low",
                        "table_kind": None, "reason": "supervision_incomplete",
                        "notes_add": ["supervision_incomplete"]}

    metas = []
    for i, g in enumerate(s.grids):
        tid = f"t{i}"
        r = s.reports.get(tid)
        f = s.final.get(tid, {})
        if f.get("notes_add"):
            g.notes.extend(f["notes_add"])
        metas.append({"caption": f.get("caption"), "unit": f.get("unit"),
                      "table_kind": f.get("table_kind"), "confidence": f.get("confidence", "low"),
                      "status": r["status"] if r else "quarantine",
                      "checks": r["checks_full"] if r else [],
                      "excluded": f.get("excluded", False)})
    pipeline.write_duckdb(s.grids, metas, out_path)

    report = {"doc": md_path, "duckdb": out_path, "turns": turns, "done_summary": done_summary,
              "budget_exhausted": done_summary is None,
              "tables": {tid: {"final": {k: v for k, v in s.final.get(tid, {}).items()},
                               "status": s.reports[tid]["status"] if tid in s.reports else "merged/quarantined"}
                         for tid in set(list(s.reports) + list(s.final))},
              "log": [{"tool": n, "args": a, "out": o} for n, a, o in s.log]}
    if report_path:
        json.dump(report, open(report_path, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    return report


def main() -> int:
    md = sys.argv[1]
    out = sys.argv[sys.argv.index("--out") + 1] if "--out" in sys.argv else "/tmp/sup_doc.duckdb"
    rep_path = sys.argv[sys.argv.index("--report") + 1] if "--report" in sys.argv else None
    max_turns = int(sys.argv[sys.argv.index("--max-turns") + 1]) if "--max-turns" in sys.argv else 40
    rep = supervise(md, out, rep_path, max_turns, dry_run="--dry-run" in sys.argv)
    if rep:
        print(json.dumps({k: v for k, v in rep.items() if k != "log"}, ensure_ascii=False, indent=1)[:3000])
    return 0


if __name__ == "__main__":
    sys.exit(main())
