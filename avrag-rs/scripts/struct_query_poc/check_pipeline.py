#!/usr/bin/env python3
"""pipeline 全链自检(assert 风格, 无框架): 对 pipeline.py 产出的 per-doc DuckDB 断言真值.

覆盖:
  IPD xlsx  → 1 表 370 行 high;六阶段计数;表序 first=LPDT-03;序号自校验
  白药 PDF  → 9 表全 needs_diagnosis;638 banner 被 header_numeric_banner 捕获;
              分隔行残迹已剔除(无全 '-' 行)
  IPD txt   → 0 表(「无表格」路径)

运行: /tmp/struct_poc/bin/python3 check_pipeline.py   (会先重建三份 duckdb)
"""
import json
import os
import subprocess
import sys

import duckdb

HERE = os.path.dirname(os.path.abspath(__file__))
PY = sys.executable
OUT = {"ipd": "/tmp/poc_ipd.duckdb", "baiyao": "/tmp/poc_baiyao.duckdb", "txt": "/tmp/poc_txt.duckdb"}
SRC = {
    "ipd": "/tmp/markitdown_out/huawei_ipd_370_activities.xlsx.md",
    "baiyao": "/tmp/markitdown_out/baiyao_it_planning.pdf.md",
    "txt": "/tmp/markitdown_out/huawei_ipd_370_activities.txt.md",
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

    print("\n== %s ==" % ("ALL PASS" if not failures else f"{len(failures)} FAILURES: {failures}"))
    return 0 if not failures else 1


if __name__ == "__main__":
    sys.exit(main())
