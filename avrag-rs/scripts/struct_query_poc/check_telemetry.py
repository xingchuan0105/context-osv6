#!/usr/bin/env python3
"""pipeline telemetry 聚合: 从 per-doc DuckDB _meta 表读出可查指标.

指标（§10 Phase 2 口径）:
  提取成功率   = high_candidate 表数 / 总表数
  状态分布     = high_candidate / needs_diagnosis / quarantine(excluded)
  指令分布     = checks 各失败类型的命中次数
  confidence   = high / low 分布（_meta.confidence 列）

运行: /tmp/struct_poc/bin/python3 check_telemetry.py [duckdb_dir]
  默认读 /tmp/poc_*.duckdb（check_pipeline.py 的三份 fixture）。
"""
import glob
import json
import os
import sys

import duckdb

DEFAULT_DIR = "/tmp"


def telemetry_for(path: str) -> dict:
    con = duckdb.connect(path, read_only=True)
    try:
        rows = con.execute(
            "SELECT status, confidence, checks FROM _meta"
        ).fetchall()
    except duckdb.Error:
        return {"file": os.path.basename(path), "tables": 0}
    total = len(rows)
    status_dist: dict[str, int] = {}
    confidence_dist: dict[str, int] = {}
    check_failures: dict[str, int] = {}
    for status, confidence, checks_json in rows:
        status_dist[status] = status_dist.get(status, 0) + 1
        confidence_dist[confidence or "unknown"] = confidence_dist.get(confidence or "unknown", 0) + 1
        checks = json.loads(checks_json) if checks_json else []
        if isinstance(checks, list):
            for c in checks:
                if isinstance(c, dict) and not c.get("passed", True):
                    name = c.get("name", "unknown")
                    check_failures[name] = check_failures.get(name, 0) + 1
    high = status_dist.get("high_candidate", 0)
    return {
        "file": os.path.basename(path),
        "tables": total,
        "extract_success_rate": round(high / total, 4) if total else None,
        "status_distribution": status_dist,
        "confidence_distribution": confidence_dist,
        "check_failure_distribution": check_failures,
    }


def main() -> int:
    scan_dir = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_DIR
    paths = sorted(glob.glob(os.path.join(scan_dir, "poc_*.duckdb")))
    if not paths:
        paths = sorted(glob.glob(os.path.join(scan_dir, "*.duckdb")))
    if not paths:
        print(json.dumps({"error": f"no duckdb files in {scan_dir}"}, ensure_ascii=False))
        return 1
    report = [telemetry_for(p) for p in paths]
    print(json.dumps(report, ensure_ascii=False, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
