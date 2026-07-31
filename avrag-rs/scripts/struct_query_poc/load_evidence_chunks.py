#!/usr/bin/env python3
"""证据 chunk 装载（2b）：pipeline sidecar（<duckdb>.evidence.json）→ PG chunks 表。

chunk_type='table_evidence'，仅 citation 水合（get_chunks_by_ids），不进检索面
（不写 rag_text_chunks / Milvus，避免与语料正文重复影响排序）。
幂等：先按 (document_id, 'table_evidence') 删除再插入。

CLI: load_evidence_chunks.py <sidecar.json> [...] [--dsn postgres://...] [--owner <uuid>]
     默认 DSN：env PG_DSN，否则 E2E persistent smoke 库。
     --owner：RLS 租户（app.current_user），默认 E2E 固定身份 00000000-0000-0000-0000-000000000001。
"""
import json
import os
import subprocess
import sys

DEFAULT_DSN = "postgres://avrag:avrag@127.0.0.1:5432/avrag_rs_e2e_smoke"
DEFAULT_OWNER = "00000000-0000-0000-0000-000000000001"


def dollar_quote(text: str) -> str:
    tag = "$struct_ev$"
    if tag in text:
        raise ValueError("evidence md contains the dollar-quote tag")
    return f"{tag}{text}{tag}"


def sql_literal(s: str) -> str:
    return "'" + s.replace("'", "''") + "'"


def main() -> int:
    dsn = os.environ.get("PG_DSN", DEFAULT_DSN)
    owner = os.environ.get("PG_OWNER", DEFAULT_OWNER)
    paths = []
    it = iter(sys.argv[1:])
    for a in it:
        if a == "--dsn":
            dsn = next(it)
        elif a == "--owner":
            owner = next(it)
        else:
            paths.append(a)
    if not paths:
        print(__doc__, file=sys.stderr)
        return 2

    # RLS：chunks/documents 启用 forced row security（tenant_isolation 按
    # app.current_user 判属主），事务内先设定身份再删插。
    stmts = [f"SELECT set_config('app.current_user', {sql_literal(owner)}, true);"]
    n_chunks = 0
    for p in paths:
        side = json.load(open(p, encoding="utf-8"))
        doc_id = side["doc_id"]
        stmts.append(
            f"DELETE FROM chunks WHERE document_id = {sql_literal(doc_id)} AND chunk_type = 'table_evidence';"
        )
        for c in side["chunks"]:
            meta = {
                "source": "struct_query_pipeline",
                "table": c["table"],
                "start_line": c["start_line"],
                "n_rows": c["n_rows"],
            }
            stmts.append(
                "INSERT INTO chunks (id, owner_user_id, document_id, chunk_type, content, metadata)\n"
                f"SELECT {sql_literal(c['chunk_id'])}, d.owner_user_id, d.id, 'table_evidence', "
                f"{dollar_quote(c['md'])}, {sql_literal(json.dumps(meta, ensure_ascii=False))}::jsonb\n"
                f"FROM documents d WHERE d.id = {sql_literal(doc_id)};"
            )
            n_chunks += 1
    sql = "BEGIN;\n" + "\n".join(stmts) + "\nCOMMIT;\n"
    r = subprocess.run(["psql", dsn, "-v", "ON_ERROR_STOP=1", "-q"],
                       input=sql, capture_output=True, text=True)
    if r.returncode != 0:
        print(r.stderr, file=sys.stderr)
        return 1
    print(f"loaded {n_chunks} evidence chunk(s) from {len(paths)} sidecar(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
