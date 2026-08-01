#!/usr/bin/env bash
# Staging ingest E2E — real external parsers (Paddle PDF, Office xlsx, optional local book).
# Not part of PR smoke. Requires avrag-rs/.env and local services where noted.
set -euo pipefail

cd "$(dirname "$0")/.."

# Ephemeral avrag-test-* PG containers are intentionally left running for
# reuse across e2e runs (see AGENTS.md). Set E2E_PRUNE_TEST_PG=1 to force teardown.
if [[ "${E2E_PRUNE_TEST_PG:-0}" == "1" ]]; then
  trap 'docker ps -aq --filter name=avrag-test- | xargs -r docker rm -f' EXIT
fi

if [[ -f .env ]]; then
  set -a
  # shellcheck disable=SC1091
  source .env
  set +a
fi

export INGESTION_PDF_MAX_PAGES="${INGESTION_PDF_MAX_PAGES:-20}"
export INGESTION_TRIPLET_ENABLED="${INGESTION_TRIPLET_ENABLED:-0}"
export INGESTION_VLM_TRIPLET_ENABLED="${INGESTION_VLM_TRIPLET_ENABLED:-0}"
export INGESTION_VLM_SUMMARY_ENABLED="${INGESTION_VLM_SUMMARY_ENABLED:-0}"
export INGESTION_PAGE_RASTER_WITH_OCR="${INGESTION_PAGE_RASTER_WITH_OCR:-0}"

cargo build -p avrag-worker -p app --features product-e2e --tests

# LiteParse + office staging E2E 已退役（2026-07-31 markitdown 唯一解析器）。
# 原 liteparse_pdf_e2e / office_*_staging_e2e 测试已删除。

black_swan_default="/mnt/e/OneDrive/桌面/知境笔记/the-black-swan_-the-impact-of-the-highly-improbable-second-edition-pdfdrive.com-.pdf"
black_swan="${E2E_LLM_REAL_BLACK_SWAN_PDF:-$black_swan_default}"
if [[ -f "$black_swan" ]]; then
  echo "== Black Swan Paddle PDF smoke (20 pages) =="
  E2E_MODE=smoke cargo test --test product_e2e -p app --features product-e2e \
    smoke::paddle_pdf_smoke::black_swan_paddle_pdf_smoke \
    -- --ignored --test-threads=1 --nocapture
else
  echo "SKIP: paddle_pdf_smoke (set E2E_LLM_REAL_BLACK_SWAN_PDF or place Black Swan PDF)"
fi

if [[ -n "${E2E_LLM_REAL_STAGING_PDF:-}" && -f "${E2E_LLM_REAL_STAGING_PDF}" ]]; then
  echo "== Optional local book llm_real staging PDF =="
  E2E_MODE=nightly SEARCH_REQUIRE_REAL=1 cargo test --test product_e2e -p app --features product-e2e \
    llm_real::pdf_corpus::real_llm_rag_staging_local_book_pdf \
    -- --ignored --test-threads=1 --nocapture
else
  echo "SKIP: llm_real staging local book (set E2E_LLM_REAL_STAGING_PDF)"
fi

echo "OK: staging ingest E2E finished"
