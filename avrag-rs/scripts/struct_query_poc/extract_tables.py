#!/usr/bin/env python3
"""struct_query PoC: markitdown md → markdown-it-py 提取 → rotate_header → DuckDB → 3 fixtures.

真值锚点(7-28/7-29 文档 + 本脚本断言一致):
  - COUNT(*) = 370
  - 验证阶段 59 / 发布阶段 30(概念 81 / 计划 86 / 开发 92 / 生命周期 22)
  - 概念阶段第一个 LPDT 活动(表序) = LPDT-03(row_ord=2,即数据第 3 行)

P0 已确认事实:
  - duckdb_markdown 社区扩展丢 CJK 单元格(tables_json/table_rows 同病),不可用;
    提取改用 markdown-it-py(gfm-like)。DuckDB 仍负责存储/查询。
  - markitdown xlsx 方言: sheet 标题行被 pandas 吃成假表头(「华为IPD…」+ Unnamed: N),
    真表头降为数据第 1 行 -> rotate_header 指令场景实证。
  - 单元格内换行为字面 \\n(markitdown xlsx 方言),利于行寻址,不影响提取。

运行: /tmp/struct_poc/bin/python3 extract_tables.py
"""
import json
import re
import sys
from collections import Counter

import duckdb
from markdown_it import MarkdownIt

MD_PATH = "/tmp/markitdown_out/huawei_ipd_370_activities.xlsx.md"

# 监督指令(P0 手工给出;P1 起由 supervision loop 产出)
# P0 发现:本语料的 Unnamed 列**并非空列**(pandas 仅因假表头行缺值而命名 Unnamed,
# 列内全是数据)——drop_columns_matching 必须有「该列在数据区全空」的确定性守卫,
# 否则会把 阶段/活动/活动号 全列误删。此处仅 rotate_header,不丢列。
DIRECTIVE = {"action": "rotate_header", "header_row": 1}


def extract_grids(md_text: str):
    """markdown-it-py gfm-like → 每张表一个 grid(含表头行,按文档序)。"""
    tokens = MarkdownIt("gfm-like").parse(md_text)
    grids, cur_rows, cur_cells, in_table = [], [], None, False
    for t in tokens:
        if t.type == "table_open":
            in_table, cur_rows = True, []
        elif t.type == "table_close":
            in_table = False
            grids.append(cur_rows)
        elif in_table and t.type == "tr_open":
            cur_cells = []
        elif in_table and t.type == "tr_close":
            cur_rows.append(cur_cells)
        elif in_table and t.type == "inline" and cur_cells is not None:
            cur_cells.append(t.content)
    return grids


def rotate_header(grid, directive):
    """rotate_header: 把第 header_row 数据行提升为表头(假表头行丢弃)。

    drop_columns_matching 可选,且带守卫:仅当该列在全部数据行为空时才真正丢弃
    (LLM 提议、确定性代码裁决——守卫不通过则拒丢并保留列)。
    """
    body = grid[1:]
    header = body[directive["header_row"] - 1]
    data = body[directive["header_row"]:]
    pat = directive.get("drop_columns_matching")
    if pat:
        n = len(header)
        keep = [
            i
            for i in range(n)
            if not (re.match(pat, header[i]) and all(i >= len(r) or r[i] == "" for r in data))
        ]
        header = [header[i] for i in keep]
        data = [[r[i] if i < len(r) else "" for i in keep] for r in data]
    return header, data


def quote_ident(name: str) -> str:
    return '"' + name.replace('"', '""') + '"'


def main() -> int:
    text = open(MD_PATH, encoding="utf-8").read()
    grids = extract_grids(text)
    assert len(grids) == 1, f"expect 1 table, got {len(grids)}"
    grid = grids[0]
    print(f"extract: 1 table, {len(grid)} rows raw (含假表头+真表头)")

    headers, rows = rotate_header(grid, DIRECTIVE)
    print(f"rotate_header -> headers={headers}")
    print(f"data rows: {len(rows)}")

    con = duckdb.connect()
    cols = ", ".join(f"{quote_ident(h)} VARCHAR" for h in headers)
    con.execute(f"CREATE TABLE ipd_activities (row_ord INTEGER, {cols})")
    con.executemany(
        f"INSERT INTO ipd_activities VALUES ({', '.join(['?'] * (len(headers) + 1))})",
        [[i] + r for i, r in enumerate(rows)],
    )

    failures = []

    def check(name, sql, expect, unordered=False):
        got = con.execute(sql).fetchall()
        ok = (sorted(got) == sorted(expect)) if unordered else (got == expect)
        print(f"{'PASS' if ok else 'FAIL'} {name}: {got}" + ("" if ok else f" expect {expect}"))
        if not ok:
            failures.append(name)

    check("A0 COUNT(*)=370", "SELECT COUNT(*) FROM ipd_activities", [(370,)])
    check(
        "A1 阶段计数 验证59/发布30",
        "SELECT 阶段, COUNT(*) FROM ipd_activities GROUP BY 阶段 ORDER BY 阶段",
        [("发布阶段", 30), ("开发阶段", 92), ("生命周期", 22), ("概念阶段", 81), ("计划阶段", 86), ("验证阶段", 59)],
        unordered=True,
    )
    check(
        "A2 概念阶段第一个 LPDT(表序)=LPDT-03",
        "SELECT 活动号 FROM ipd_activities WHERE 角色 LIKE '%LPDT%' AND 阶段='概念阶段' ORDER BY row_ord LIMIT 1",
        [("LPDT-03",)],
    )
    check(
        "A3 序号自校验 max==count==370",
        "SELECT MAX(CAST(编号 AS INTEGER)), COUNT(*) FROM ipd_activities",
        [(370, 370)],
    )

    print("\n== %s ==" % ("ALL PASS" if not failures else f"{len(failures)} FAILURES: {failures}"))
    return 0 if not failures else 1


if __name__ == "__main__":
    sys.exit(main())
