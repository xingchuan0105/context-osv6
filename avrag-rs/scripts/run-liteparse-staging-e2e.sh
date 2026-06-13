#!/usr/bin/env bash
# LiteParse staging E2E — bundled phase0-mini.pdf, full upload → worker → index.
set -euo pipefail

cd "$(dirname "$0")/.."

trap 'docker ps -aq --filter name=avrag-test- | xargs -r docker rm -f' EXIT

export E2E_MODE=integration

# Respect repo .env for optional Paddle / pdf-renderer when extending this run.
if [[ -f .env ]]; then
  set -a
  # shellcheck disable=SC1091
  source .env
  set +a
fi

export INGESTION_PDF_MAX_PAGES="${INGESTION_PDF_MAX_PAGES:-8}"
export INGESTION_TRIPLET_ENABLED="${INGESTION_TRIPLET_ENABLED:-0}"
export INGESTION_VLM_TRIPLET_ENABLED="${INGESTION_VLM_TRIPLET_ENABLED:-0}"
export INGESTION_VLM_SUMMARY_ENABLED="${INGESTION_VLM_SUMMARY_ENABLED:-0}"

cargo build -p avrag-worker

echo "== LiteParse staging E2E (phase0-mini.pdf) =="
cargo test --test product_e2e -p app --features product-e2e \
  integration::liteparse_pdf_e2e::phase0_mini_liteparse_pdf_ingest_e2e \
  -- --test-threads=1 --nocapture
