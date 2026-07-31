#!/usr/bin/env python3
"""pipeline 全链自检(assert 风格, 无框架): 对 pipeline.py 产出的 per-doc DuckDB 断言真值.

覆盖:
  IPD xlsx  → 1 表 370 行 high;六阶段计数;表序 first=LPDT-03;序号自校验
  白药 PDF  → 9 表全 needs_diagnosis;638 banner 被 header_numeric_banner 捕获;
              分隔行残迹已剔除(无全 '-' 行)
  IPD txt   → 0 表(「无表格」路径)
  万科年报  → t114(双栏混入)触发 dual_column_suspect(detail 含三块面板表头行源行);
              ipd/白药 dual_column_suspect 零误报

运行: /tmp/struct_poc/bin/python3 check_pipeline.py   (会先重建四份 duckdb)
"""
import json
import os
import subprocess
import sys

import duckdb

HERE = os.path.dirname(os.path.abspath(__file__))
PY = sys.executable
OUT = {"ipd": "/tmp/poc_ipd.duckdb", "baiyao": "/tmp/poc_baiyao.duckdb",
       "txt": "/tmp/poc_txt.duckdb", "vanke": "/tmp/poc_vanke.duckdb"}
SRC = {
    "ipd": "/tmp/markitdown_out/huawei_ipd_370_activities.xlsx.md",
    "baiyao": "/tmp/markitdown_out/baiyao_it_planning.pdf.md",
    "txt": "/tmp/markitdown_out/huawei_ipd_370_activities.txt.md",
    "vanke": "/tmp/markitdown_out/万科2024年报.pdf.md",
}

failures = []


def check(name, cond, detail=""):
    print(f"{'PASS' if cond else 'FAIL'} {name}" + (f"  {detail}" if not cond else ""))
    if not cond:
        failures.append(name)


def build():
    for k, src in SRC.items():
        r = subprocess.run([PY, os.path.join(HERE, "pipeline.py"), src, "--out", OUT[k]],
                           capture_output=True, text=True)
        assert r.returncode == 0, f"pipeline failed for {k}: {r.stderr}"


def main() -> int:
    build()

    ipd = duckdb.connect(OUT["ipd"], read_only=True)
    check("ipd: 1 table in _meta", ipd.execute("SELECT COUNT(*) FROM _meta").fetchone()[0] == 1)
    check("ipd: n_rows=370", ipd.execute("SELECT COUNT(*) FROM t0").fetchone()[0] == 370)
    phases = dict(ipd.execute("SELECT 阶段, COUNT(*) FROM t0 GROUP BY 阶段").fetchall())
    check("ipd: 验证59/发布30/概念81", phases.get("验证阶段") == 59 and phases.get("发布阶段") == 30
          and phases.get("概念阶段") == 81, str(phases))
    first = ipd.execute("SELECT 活动号 FROM t0 WHERE 角色 LIKE '%LPDT%' AND 阶段='概念阶段' "
                        "ORDER BY row_ord LIMIT 1").fetchone()
    check("ipd: 表序 first=LPDT-03", first == ("LPDT-03",), str(first))
    seq = ipd.execute("SELECT MAX(CAST(编号 AS INTEGER)), COUNT(*) FROM t0").fetchone()
    check("ipd: 序号自校验 max==count==370", seq == (370, 370), str(seq))
    meta = ipd.execute("SELECT status, notes FROM _meta WHERE table_name='t0'").fetchone()
    check("ipd: status=high_candidate", meta[0] == "high_candidate", str(meta))

    by = duckdb.connect(OUT["baiyao"], read_only=True)
    n = by.execute("SELECT COUNT(*) FROM _meta").fetchone()[0]
    check("baiyao: 9 tables", n == 9, str(n))
    banner = by.execute("SELECT COUNT(*) FROM _meta WHERE checks LIKE '%header_numeric_banner%' "
                        "AND checks LIKE '%638%'").fetchone()[0]
    check("baiyao: 638 banner 被 header_numeric_banner 捕获", banner == 1, f"got {banner}")
    junk = 0
    for (tbl,) in by.execute("SELECT table_name FROM _meta").fetchall():
        cols = [c[0] for c in by.execute(f'DESCRIBE "{tbl}"').fetchall() if c[0] not in ("row_ord", "__src_line")]
        q = " AND ".join(f'"{c}" ~ \'^[-:\\s]*$\'' for c in cols)
        junk += by.execute(f'SELECT COUNT(*) FROM "{tbl}" WHERE {q}').fetchone()[0]
    check("baiyao: 无分隔行残迹(全 '-' 行=0)", junk == 0, f"junk rows {junk}")
    low = by.execute("SELECT COUNT(*) FROM _meta WHERE status='needs_diagnosis'").fetchone()[0]
    check("baiyao: 全部 needs_diagnosis(布局网格待监督裁决)", low == n, f"{low}/{n}")

    txt = duckdb.connect(OUT["txt"], read_only=True)
    check("txt: 0 tables(无表格路径)", txt.execute("SELECT COUNT(*) FROM _meta").fetchone()[0] == 0)

    # dual_column_suspect 零误报: ipd/白药所有表的 checks_full 中 dual_column_suspect 均 passed
    def failed_checks(con, tbl):
        checks = json.loads(con.execute("SELECT checks FROM _meta WHERE table_name=?", [tbl]).fetchone()[0])
        return {c["name"]: c for c in checks if not c["passed"]}

    check("ipd: dual_column_suspect 零误报", "dual_column_suspect" not in failed_checks(ipd, "t0"))
    by_fp = [t for (t,) in by.execute("SELECT table_name FROM _meta").fetchall()
             if "dual_column_suspect" in failed_checks(by, t)]
    check("baiyao: dual_column_suspect 零误报", by_fp == [], str(by_fp))

    # 万科 t114(资产负债表双栏混入): dual_column_suspect 触发, detail 给出三块面板表头行源行
    vk = duckdb.connect(OUT["vanke"], read_only=True)
    meta114 = vk.execute("SELECT n_rows, status, checks FROM _meta WHERE table_name='t114'").fetchone()
    checks114 = {c["name"]: c for c in json.loads(meta114[2])}
    dc = checks114.get("dual_column_suspect", {})
    check("vanke: t114 79 行 needs_diagnosis", meta114[0] == 79 and meta114[1] == "needs_diagnosis",
          str(meta114[:2]))
    check("vanke: t114 触发 dual_column_suspect", dc.get("passed") is False, str(dc))
    check("vanke: t114 detail 行集正确(面板表头行 6084/6135/6160)",
          all(str(l) in dc.get("detail", "") for l in (6084, 6135, 6160)), dc.get("detail", ""))
    sh = checks114.get("section_header_rows", {})
    check("vanke: t114 section_header_rows 提示信号(passed=True 有 detail)",
          sh.get("passed") is True and bool(sh.get("detail")), str(sh))

    print("\n== %s ==" % ("ALL PASS" if not failures else f"{len(failures)} FAILURES: {failures}"))
    return 0 if not failures else 1


if __name__ == "__main__":
    sys.exit(main())
