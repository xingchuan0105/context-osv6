#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${ROOT_DIR}/frontend_next/lib/contracts/generated"
CONTRACTS_DIR="${ROOT_DIR}/contracts"

mkdir -p "${OUT_DIR}/fixtures"

echo "==> typeshare: ${CONTRACTS_DIR}/src -> ${OUT_DIR}"
typeshare \
  -l typescript \
  -c "${CONTRACTS_DIR}/typeshare.toml" \
  -d "${OUT_DIR}" \
  "${CONTRACTS_DIR}/src"

CONTRACTS_TS="${OUT_DIR}/contracts.ts"
if [[ -f "${CONTRACTS_TS}" ]]; then
  sed -i 's/BTreeMap<string, unknown>/Record<string, unknown>/g' "${CONTRACTS_TS}"
  if grep -q 'AnswerBlock' "${CONTRACTS_TS}" && ! grep -q 'import type { AnswerBlock }' "${CONTRACTS_TS}"; then
    python3 - <<PY
from pathlib import Path
path = Path("${CONTRACTS_TS}")
text = path.read_text()
needle = 'import type { AnswerBlock } from "./answer_block";\n'
if needle.strip() not in text:
    if text.startswith("/*"):
        end = text.find("*/")
        if end != -1:
            insert_at = end + 2
            text = text[:insert_at] + "\n\n" + needle + text[insert_at:]
        else:
            text = needle + "\n" + text
    else:
        text = needle + "\n" + text
    path.write_text(text)
PY
  fi
fi

echo "==> ts-rs: AnswerBlock, ChatEvent"
(
  cd "${CONTRACTS_DIR}"
  cargo run --quiet --bin export-types
)

echo "==> golden JSON fixtures"
(
  cd "${CONTRACTS_DIR}"
  cargo test --quiet export_golden_fixtures -- --ignored --exact
)

cat > "${OUT_DIR}/index.ts" <<'EOF'
/**
 * Generated contract types from the Rust `contracts` crate.
 * Regenerate: `pnpm generate:contracts`
 */
export * from "./contracts";
export type { AnswerBlock } from "./answer_block";
export type { ChatEvent } from "./chat_event";
EOF

echo "==> done: ${OUT_DIR}"
