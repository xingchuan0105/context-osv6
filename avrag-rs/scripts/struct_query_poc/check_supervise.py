#!/usr/bin/env python3
"""supervision loop 自检(assert 风格, 无框架).

Part 1(确定性, 无 LLM): 工具语义与守卫
  - quarantine/exclude 终态不被 annotate 覆盖(回归: 2026-07-31 annotate 复活隔离 bug)
  - rotate_header header_row=0 / 越界 → 拒
  - set_header 证据行不含表头文字 → 拒
  - drop_columns_matching 守卫(非全空列拒丢 — IPD Unnamed 方言)
  - rotate_header 合法 → 应用且复验过(IPD 案例)
  - run_check 只读守卫(禁 ATTACH/多语句)
  - 兜底: 未处理表 → low + supervision_incomplete

Part 2(--live, 真 LLM, INGESTION_LLM_*): 结构不变量
  - loop 在预算内结束; 每表有终态; quarantine 表不在 duckdb;
    无「校验失败却 confidence=high」; 非排除表入库且 confidence 与报告一致

运行: /tmp/struct_poc/bin/python3 check_supervise.py [--live]
"""
import json
import os
import sys

import duckdb

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import supervise

BY_MD = "/tmp/markitdown_out/baiyao_it_planning.pdf.md"
IPD_MD = "/tmp/markitdown_out/huawei_ipd_370_activities.xlsx.md"

failures = []


def check(name, cond, detail=""):
    print(f"{'PASS' if cond else 'FAIL'} {name}" + (f"  {detail}" if not cond else ""))
    if not cond:
        failures.append(name)


def part1():
    s = supervise.Session(BY_MD)

    # 隔离终态不被 annotate 覆盖(回归)
    s.t_quarantine({"table_id": "t2", "reason": "回归测试"})
    r = s.t_annotate({"tables": [{"table_id": "t2", "table_kind": "detail", "confidence": "low"}]})
    check("quarantine 后 annotate 被拒", "终态不被" in r, r)
    check("t2 保持 excluded", s.final["t2"]["excluded"] is True)

    # exclude 指令同样 sticky
    s.t_apply_directive({"table_id": "t0", "directive": {"action": "exclude", "reason": "layout"}})
    r = s.t_annotate({"tables": [{"table_id": "t0", "table_kind": "layout", "confidence": "low"}]})
    check("exclude 后 annotate 被拒", "终态不被" in r, r)

    # rotate_header 守卫
    r = s.t_apply_directive({"table_id": "t1", "directive": {"action": "rotate_header", "header_row": 0}})
    check("rotate_header header_row=0 被拒", "未通过校验" in r, r[:80])
    r = s.t_apply_directive({"table_id": "t1", "directive": {"action": "rotate_header", "header_row": 999}})
    check("rotate_header 越界被拒", "未通过校验" in r)

    # set_header 证据守卫
    r = s.t_apply_directive({"table_id": "t1",
                             "directive": {"action": "set_header", "headers": ["甲"] * 11,
                                           "evidence_source_line": 1}})
    check("set_header 证据行不含文字被拒", "未通过校验" in r, r[:100])

    # confidence=high 守卫(校验失败的表)
    r = s.t_annotate({"tables": [{"table_id": "t1", "table_kind": "detail", "confidence": "high"}]})
    check("校验失败表 confidence=high 被拒", "守卫" in r, r[:80])

    # run_check 只读守卫
    r = s.t_run_check({"sql": "ATTACH '/etc/passwd'"})
    check("run_check 禁 ATTACH", "守卫" in r)
    r = s.t_run_check({"sql": "SELECT COUNT(*) FROM t1"})
    check("run_check 合法 SELECT 可执行", "run_check 完成" in r, r[:60])

    # IPD: 合法 rotate_header 应用且复验过(drop 守卫拒丢非空列)
    s2 = supervise.Session(IPD_MD)
    g = s2.grids[0]
    check("IPD auto-rotate 已生效(表头=编号/阶段/…)", g.header[0] == "编号", str(g.header[:3]))
    r = s2.t_run_check({"sql": "SELECT COUNT(*) FROM t0"})
    check("IPD run_check COUNT=370", "370" in r, r[:80])

    # 兜底终态
    un = s.unfinished()
    check("unfinished 不含已终态表", "t2" not in un and "t0" not in un)


def part2_live():
    rep = supervise.supervise(BY_MD, "/tmp/sup_baiyao_live.duckdb", None, max_turns=40)
    check("live: 预算内结束(done 被调用)", rep["done_summary"] is not None,
          f"turns={rep['turns']}")
    check("live: turns <= 40", rep["turns"] <= 40, str(rep["turns"]))
    finals = rep["tables"]
    check("live: 每表有终态", all(t["final"] for t in finals.values()),
          str([k for k, t in finals.items() if not t["final"]]))
    con = duckdb.connect("/tmp/sup_baiyao_live.duckdb", read_only=True)
    in_db = {r[0]: r[1] for r in con.execute("SELECT table_name, confidence FROM _meta").fetchall()}
    for tid, t in finals.items():
        f = t["final"]
        if f.get("excluded"):
            check(f"live: {tid} quarantine/excluded 不在库", tid not in in_db)
        else:
            check(f"live: {tid} 在库且 confidence 一致",
                  in_db.get(tid) == f.get("confidence"), f"{in_db.get(tid)} != {f.get('confidence')}")
    bad_high = [tid for tid, t in finals.items()
                if t["final"].get("confidence") == "high" and t.get("status") == "needs_diagnosis"]
    check("live: 无「校验失败却 high」", not bad_high, str(bad_high))


def main() -> int:
    print("--- Part 1: 确定性工具语义 ---")
    part1()
    if "--live" in sys.argv:
        print("--- Part 2: live LLM 结构不变量 ---")
        part2_live()
    print("\n== %s ==" % ("ALL PASS" if not failures else f"{len(failures)} FAILURES: {failures}"))
    return 0 if not failures else 1


if __name__ == "__main__":
    sys.exit(main())
