#!/usr/bin/env python3
"""Regenerate docs/e2e-test-registry.yaml from `cargo test --list` output."""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs" / "e2e-test-registry.yaml"

IGNORED_SUFFIXES = {
    "backend_launcher",
    "black_swan_paddle_pdf_smoke",
    "open_query_with_real_brave_returns_web_citation",
    "real_llm_concurrent_rag_queries_have_independent_citation_chunks",
    "real_llm_general_chat_returns_substantive_answer",
    "real_llm_format_html_renderer_returns_html",
    "real_llm_multi_turn_rag_follow_up_remembers_context",
    "real_llm_rag_bundled_pdf_corpus_query",
    "real_llm_rag_multidoc_pdf_and_txt",
    "real_llm_rag_staging_local_book_pdf",
    "real_llm_rag_after_liteparse_pdf_ingest_returns_citation",
    "real_llm_rag_complex_query_uses_multiple_tools",
    "real_llm_rag_document_qa_returns_citation",
    "real_llm_search_open_query_returns_web_citation",
    "office_xlsx_staging_ingest_e2e",
    "minimal_docx_liteparse_pdf_ingest_e2e",
    "cost_report_from_artifacts",
}

MODULE_META: dict[tuple[str, str], dict] = {
    ("smoke", "chat_smoke"): {
        "capabilities": ["CAP-CHAT"],
        "parallel_group": "G-parallel-smoke",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("smoke", "search_smoke"): {
        "capabilities": ["CAP-SEARCH"],
        "parallel_group": "G-parallel-smoke",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("smoke", "search_real_smoke"): {
        "capabilities": ["CAP-SEARCH"],
        "parallel_group": "G-parallel-smoke",
        "deps": ["R", "I"],
        "evidence": ["E-P", "E-Prod", "E-Q"],
        "note": "SEARCH_USE_REAL=1 staging; #[ignore]",
    },
    ("smoke", "ingestion_smoke"): {
        "capabilities": ["CAP-INGEST"],
        "parallel_group": "G-serial-rag",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("smoke", "rag_smoke"): {
        "capabilities": ["CAP-RAG"],
        "parallel_group": "G-serial-rag",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("smoke", "rag_fallback_smoke"): {
        "capabilities": ["CAP-RAG", "CAP-DEGRADE"],
        "parallel_group": "G-serial-rag",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("smoke", "rag_codegen_multitool_smoke"): {
        "capabilities": ["CAP-RAG"],
        "parallel_group": "G-serial-rag",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("smoke", "memory_multiturn_smoke"): {
        "capabilities": ["CAP-MEMORY"],
        "parallel_group": "G-serial-rag",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("smoke", "paddle_image_smoke"): {
        "capabilities": ["CAP-INGEST"],
        "parallel_group": "G-serial-rag",
        "deps": ["M", "I", "P"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("smoke", "paddle_pdf_smoke"): {
        "capabilities": ["CAP-INGEST"],
        "parallel_group": "G-serial-rag",
        "deps": ["M", "I", "P"],
        "evidence": ["E-P", "E-Prod"],
        "note": "manual-only; Black Swan PDF + real Paddle Jobs; #[ignore]",
    },
    ("smoke", "auth_boundary"): {
        "capabilities": ["CAP-AUTH", "CAP-CHAT"],
        "parallel_group": "G-parallel-smoke",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
        "note": "run module with --test-threads=1",
    },
    ("smoke", "share_boundary"): {
        "capabilities": ["CAP-SHARE"],
        "parallel_group": "G-parallel-smoke",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("smoke", "billing_boundary"): {
        "capabilities": ["CAP-BILLING"],
        "parallel_group": "G-parallel-smoke",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("integration", "streaming_chat"): {
        "capabilities": ["CAP-STREAM", "CAP-RAG"],
        "parallel_group": "G-serial-integration",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod", "E-Obs"],
    },
    ("integration", "liteparse_pdf_e2e"): {
        "capabilities": ["CAP-INGEST"],
        "parallel_group": "G-serial-integration",
        "deps": ["M", "I", "P"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("integration", "paddle_image_e2e"): {
        "capabilities": ["CAP-INGEST"],
        "parallel_group": "G-serial-integration",
        "deps": ["M", "I", "P"],
        "evidence": ["E-P", "E-Prod"],
        "note": "routing metadata contract; full path in smoke::paddle_image_smoke",
    },
    ("integration", "office_xlsx_e2e"): {
        "capabilities": ["CAP-INGEST"],
        "parallel_group": "G-serial-integration",
        "deps": ["M", "I", "P"],
        "evidence": ["E-P", "E-Prod"],
        "note": "mock office-parser; real JVM in office_xlsx_staging_e2e",
    },
    ("integration", "office_doc_liteparse_e2e"): {
        "capabilities": ["CAP-INGEST"],
        "parallel_group": "G-serial-integration",
        "deps": ["I"],
        "evidence": ["E-P", "E-Prod"],
        "note": "docx→pdf LiteParse; #[ignore] requires libreoffice",
    },
    ("integration", "office_xlsx_staging_e2e"): {
        "capabilities": ["CAP-INGEST"],
        "parallel_group": "G-serial-integration",
        "deps": ["I", "P"],
        "evidence": ["E-P", "E-Prod"],
        "note": "real office-parser-jvm; #[ignore] staging only",
    },
    ("integration", "document_lifecycle"): {
        "capabilities": ["CAP-INGEST"],
        "parallel_group": "G-serial-integration",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("integration", "format_output"): {
        "capabilities": ["CAP-FORMAT"],
        "parallel_group": "G-serial-integration",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("integration", "multi_doc"): {
        "capabilities": ["CAP-RAG"],
        "parallel_group": "G-serial-integration",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("integration", "concurrent_query"): {
        "capabilities": ["CAP-RAG"],
        "parallel_group": "G-serial-integration",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("integration", "embedding_cache"): {
        "capabilities": ["CAP-RAG"],
        "parallel_group": "G-serial-integration",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("integration", "ingestion_full"): {
        "capabilities": ["CAP-RAG", "CAP-DEGRADE"],
        "parallel_group": "G-serial-integration",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("integration", "bad_file"): {
        "capabilities": ["CAP-INGEST"],
        "parallel_group": "G-serial-integration",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("integration", "duplicate_upload"): {
        "capabilities": ["CAP-INGEST"],
        "parallel_group": "G-serial-integration",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("failure", "search_degrade"): {
        "capabilities": ["CAP-DEGRADE", "CAP-SEARCH"],
        "parallel_group": "G-serial-integration",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("failure", "provider_down"): {
        "capabilities": ["CAP-DEGRADE", "CAP-SEARCH"],
        "parallel_group": "G-serial-integration",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("failure", "embedding_down"): {
        "capabilities": ["CAP-DEGRADE", "CAP-RAG"],
        "parallel_group": "G-serial-integration",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("failure", "timeout"): {
        "capabilities": ["CAP-DEGRADE", "CAP-INGEST"],
        "parallel_group": "G-serial-integration",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("tenants", "isolation"): {
        "capabilities": ["CAP-TENANT", "CAP-RAG"],
        "parallel_group": "G-serial-integration",
        "deps": ["M", "I"],
        "evidence": ["E-P", "E-Prod"],
    },
    ("llm_real", "chat_real"): {
        "capabilities": ["CAP-CHAT"],
        "parallel_group": "G-serial-llm",
        "deps": ["R", "I"],
        "evidence": ["E-P", "E-Prod", "E-Q", "E-Obs"],
    },
    ("llm_real", "rag_real"): {
        "capabilities": ["CAP-RAG"],
        "parallel_group": "G-serial-llm",
        "deps": ["R", "I"],
        "evidence": ["E-P", "E-Prod", "E-Q", "E-Obs"],
    },
    ("llm_real", "search_real"): {
        "capabilities": ["CAP-SEARCH"],
        "parallel_group": "G-serial-llm",
        "deps": ["R", "I"],
        "evidence": ["E-P", "E-Prod", "E-Q", "E-Obs"],
    },
    ("llm_real", "multi_turn"): {
        "capabilities": ["CAP-MEMORY", "CAP-RAG"],
        "parallel_group": "G-serial-llm",
        "deps": ["R", "I"],
        "evidence": ["E-P", "E-Prod", "E-Q", "E-Obs"],
    },
    ("llm_real", "format_real"): {
        "capabilities": ["CAP-FORMAT"],
        "parallel_group": "G-serial-llm",
        "deps": ["R", "I"],
        "evidence": ["E-P", "E-Prod", "E-Q", "E-Obs"],
    },
    ("llm_real", "pdf_corpus"): {
        "capabilities": ["CAP-RAG", "CAP-INGEST"],
        "parallel_group": "G-serial-llm",
        "deps": ["R", "I"],
        "evidence": ["E-P", "E-Prod", "E-Q", "E-Obs"],
        "note": "bundled phase0-mini.pdf P4 routing + optional E2E_LLM_REAL_STAGING_PDF manual probe",
    },
    ("llm_real", "pdf_rag_e2e"): {
        "capabilities": ["CAP-RAG", "CAP-INGEST"],
        "parallel_group": "G-serial-llm",
        "deps": ["R", "I", "P"],
        "evidence": ["E-P", "E-Prod", "E-Q", "E-Obs"],
    },
    ("llm_real", "stream_reasoning_tests"): {
        "capabilities": ["CAP-STREAM"],
        "parallel_group": "G-serial-llm",
        "deps": ["M"],
        "evidence": ["E-P", "E-Obs"],
        "layer_override": "L6",
    },
}

LAYER_BY_SUITE = {
    "smoke": "L1",
    "integration": "L2",
    "failure": "L2",
    "tenants": "L2",
    "llm_real": "L3",
}

INFRA_DEFAULT = {
    "capabilities": [],
    "parallel_group": None,
    "deps": [],
    "evidence": ["E-P"],
    "layer_override": "L6",
}


def list_tests() -> list[str]:
    out = subprocess.check_output(
        [
            "cargo",
            "test",
            "--test",
            "product_e2e",
            "-p",
            "app",
            "--features",
            "product-e2e",
            "--",
            "--list",
        ],
        cwd=ROOT,
        text=True,
        stderr=subprocess.DEVNULL,
    )
    ids = []
    for line in out.splitlines():
        if line.strip().endswith(": test"):
            ids.append(line.rsplit(": test", 1)[0].strip())
    return ids


def yaml_quote(s: str) -> str:
    if re.fullmatch(r"[A-Za-z0-9_.:-]+", s):
        return s
    return '"' + s.replace('"', '\\"') + '"'


def emit_list(key: str, items: list[str], indent: int) -> list[str]:
    pad = " " * indent
    if not items:
        return [f"{pad}{key}: []"]
    lines = [f"{pad}{key}:"]
    for item in items:
        lines.append(f"{pad}  - {yaml_quote(item)}")
    return lines


def main() -> int:
    test_ids = list_tests()
    entries = []

    for tid in test_ids:
        m = re.match(
            r"product_e2e::(?P<suite>\w+)::(?P<module>[^:]+)::(?P<fn>[^:]+)$", tid
        )
        if m:
            suite, module, fn = m.group("suite"), m.group("module"), m.group("fn")
            meta = MODULE_META.get((suite, module), {})
            layer = meta.get("layer_override") or LAYER_BY_SUITE.get(suite, "L6")
            caps = meta.get("capabilities", [])
            evidence = meta.get("evidence", ["E-P"])
            deps = meta.get("deps", ["M", "I"] if suite != "llm_real" else ["R", "I"])
            pg = meta.get("parallel_group")
            ignored = fn in IGNORED_SUFFIXES or module == "paddle_pdf_smoke" and fn.startswith(
                "black_swan"
            )
            note = meta.get("note")
        else:
            # infrastructure: setup::tests, e2e_gate, mock_routing, backend_launcher
            layer = "L6"
            caps = []
            evidence = ["E-P"]
            deps = []
            pg = None
            ignored = tid.endswith("backend_launcher")
            note = None
            if "e2e_gate" in tid:
                caps = []
                note = "E2E_MODE suite gating"
            elif "mock_routing" in tid:
                caps = ["CAP-RAG", "CAP-CHAT"]
                note = "mock LLM routing contracts"
            elif "test_context" in tid:
                note = "Milvus/PG bootstrap hygiene"

        entry_lines = [
            f"  - id: {yaml_quote(tid)}",
            f"    layer: {layer}",
        ]
        if m:
            entry_lines.append(f"    module: {module}")
            entry_lines.append(f"    suite: {suite}")
        entry_lines.extend(emit_list("capabilities", caps, 4))
        entry_lines.extend(emit_list("evidence", evidence, 4))
        if deps:
            entry_lines.extend(emit_list("deps", deps, 4))
        if pg:
            entry_lines.append(f"    parallel_group: {yaml_quote(pg)}")
        entry_lines.append(f"    ci_default: {'false' if ignored else 'true'}")
        if ignored:
            entry_lines.append("    ignore: true")
        if note:
            entry_lines.append(f"    note: {yaml_quote(note)}")
        entries.append("\n".join(entry_lines))

    header = """# E2E Test Registry — machine-readable index for TEAF
# See: docs/e2e-analysis-framework.md
# Regenerate: ./scripts/generate-e2e-test-registry.py

schema_version: 1
generated_test_count: {count}

capability_domains:
  CAP-CHAT:
    name: 通用对话
    required_layers: [L1, L3, L5]
  CAP-RAG:
    name: 文档问答
    required_layers: [L1, L2, L3, L4, L5]
  CAP-SEARCH:
    name: 联网搜索
    required_layers: [L1, L2, L3, L4, L5]
  CAP-INGEST:
    name: 入库解析
    required_layers: [L1, L2, L3]
  CAP-STREAM:
    name: 流式可观测
    required_layers: [L2, L6]
  CAP-MEMORY:
    name: 多轮记忆
    required_layers: [L1, L3]
  CAP-FORMAT:
    name: 格式输出
    required_layers: [L2, L3, L4]
  CAP-AUTH:
    name: 认证边界
    required_layers: [L1, L6]
  CAP-SHARE:
    name: 协作分享
    required_layers: [L1, L5]
  CAP-BILLING:
    name: 计费同意
    required_layers: [L1, L5]
  CAP-TENANT:
    name: 租户隔离
    required_layers: [L2]
  CAP-DEGRADE:
    name: 降级韧性
    required_layers: [L2]

smoke_module_lists:
  non_rag: [chat_smoke, search_smoke, auth_boundary, share_boundary, billing_boundary]
  rag_serial: [ingestion_smoke, rag_smoke, rag_fallback_smoke, rag_codegen_multitool_smoke, memory_multiturn_smoke, paddle_image_smoke]
  manual_only: [search_real_smoke, paddle_pdf_smoke]

playwright_specs:
  - path: frontend_next/e2e/specs/skills/rag-available.spec.ts
    layer: L4
    capabilities: [CAP-RAG]
    evidence: [E-P, E-Prod, E-Q]
    citation_gate: hard
  - path: frontend_next/e2e/specs/skills/search-available.spec.ts
    layer: L4
    capabilities: [CAP-SEARCH]
    evidence: [E-P, E-Prod, E-Q]
    citation_gate: hard
  - path: frontend_next/e2e/specs/skills/format-output.spec.ts
    layer: L4
    capabilities: [CAP-FORMAT]
    evidence: [E-P, E-Prod, E-Q]
    citation_gate: hard
  - path: frontend_next/e2e/specs/journey/workspace-chat.spec.ts
    layer: L5
    capabilities: [CAP-CHAT, CAP-SEARCH]
    evidence: [E-P, E-Prod]
    citation_gate: soft_search_hard_on_nightly
  - path: frontend_next/e2e/specs/journey/workspace-upload-rag.spec.ts
    layer: L5
    capabilities: [CAP-RAG, CAP-INGEST]
    evidence: [E-P, E-Prod, E-Q]
    citation_gate: hard
    note: txt fixture upload
  - path: frontend_next/e2e/specs/journey/workspace-upload-pdf-rag.spec.ts
    layer: L5
    capabilities: [CAP-RAG, CAP-INGEST]
    evidence: [E-P, E-Prod, E-Q]
    citation_gate: hard
    note: bundled phase0-mini.pdf LiteParse path
  - path: frontend_next/e2e/specs/journey/invite-collaboration.spec.ts
    layer: L5
    capabilities: [CAP-SHARE]
    evidence: [E-P, E-Prod]
  - path: frontend_next/e2e/specs/journey/workspace-share.spec.ts
    layer: L5
    capabilities: [CAP-SHARE]
    evidence: [E-P, E-Prod]
  - path: frontend_next/e2e/specs/smoke/auth-flow.spec.ts
    layer: L5
    capabilities: [CAP-AUTH]
    evidence: [E-P, E-Prod]
  - path: frontend_next/e2e/specs/billing/paywall-flow.spec.ts
    layer: L5
    capabilities: [CAP-BILLING]
    evidence: [E-P, E-Prod]

contract_tests:
  - id: transport-http::chat_stream_contract
    layer: L6
    capabilities: [CAP-STREAM, CAP-CHAT]
    evidence: [E-P]
    package: transport-http
    path: crates/transport-http/tests/chat_stream_contract.rs
  - id: transport-http::router_surface::admin_routes_reject_org_only_proxy_auth
    layer: L6
    capabilities: [CAP-AUTH]
    evidence: [E-P, E-Prod]
    package: transport-http
    path: crates/transport-http/tests/router_surface.rs
  - id: transport-http::runtime_execute_contract::post_runtime_execute_requires_auth_context
    layer: L6
    capabilities: [CAP-AUTH]
    evidence: [E-P, E-Prod]
    package: transport-http
    path: crates/transport-http/tests/runtime_execute_contract.rs
  - id: transport-http::rag_execute_plan_contract::post_rag_execute_plan_requires_auth_context
    layer: L6
    capabilities: [CAP-AUTH, CAP-RAG]
    evidence: [E-P, E-Prod]
    package: transport-http
    path: crates/transport-http/tests/rag_execute_plan_contract.rs
  - id: app::unified_agent_contract
    layer: L6
    capabilities: [CAP-RAG]
    evidence: [E-P, E-Prod]
    package: app
  - id: transport-http::api_key_security_contract::workspace_api_key_cannot_create_org_api_key
    layer: L6
    capabilities: [CAP-AUTH]
    evidence: [E-P, E-Prod]
    package: transport-http
    path: crates/transport-http/tests/api_key_security_contract.rs
    note: API key scope/permission boundary (workspace vs org, admin strip)
  - id: transport-http::mcp_unified_contract::mcp_ingestion_flow_create_upload_complete_status
    layer: L6
    capabilities: [CAP-AUTH, CAP-INGEST]
    evidence: [E-P, E-Prod]
    package: transport-http
    path: crates/transport-http/tests/mcp_unified_contract.rs
    note: MCP tools/call envelope + create_upload -> complete -> status flow
  - id: transport-http::openai_completions_contract::openai_completions_with_workspace_key_returns_200_body
    layer: L6
    capabilities: [CAP-CHAT, CAP-AUTH]
    evidence: [E-P, E-Prod]
    package: transport-http
    path: crates/transport-http/tests/openai_completions_contract.rs
    note: OpenAI-compatible chat completions route — 401 / 200+body / SSE / 403 boundary

tests:
""".format(
        count=len(test_ids)
    )

    OUT.write_text(header + "\n".join(entries) + "\n", encoding="utf-8")
    print(f"Wrote {len(test_ids)} tests to {OUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
