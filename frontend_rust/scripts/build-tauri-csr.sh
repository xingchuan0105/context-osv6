#!/usr/bin/env bash
# Build the Phase 0 Tauri CSR artifact. Does not touch desktop/src-tauri/tauri.conf.json.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
export PATH="${HOME}/.local/opt/wasm-bindgen-cli-0.2.127:${PATH}"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"

cargo build -p web-ui --target wasm32-unknown-unknown --no-default-features --features csr

out="${root}/dist/tauri"
mkdir -p "${out}"
wasm-bindgen --target web --out-dir "${out}" \
  "${root}/target/wasm32-unknown-unknown/debug/web_ui.wasm"

cp -f "${root}/crates/web-ui/csr/index.html" "${out}/index.html"
cp -f "${root}/style/design-tokens.css" "${out}/design-tokens.css"
cp -f "${root}/assets/style/chat-poc.css" "${out}/chat-poc.css"

echo "CSR artifact written to ${out}"
