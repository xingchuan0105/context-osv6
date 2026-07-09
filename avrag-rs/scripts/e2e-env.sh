#!/usr/bin/env bash
# Source this before running cargo for E2E/product_e2e work:
#   source scripts/e2e-env.sh
#
# Sets RUSTFLAGS to suppress `dead_code` so rustc's check_mod_deathness pass does not
# ICE (rust-lang/rust#157460: StyledBuffer::replace slice OOB when emitting a dead-code
# lint for an item with a >=11-char name). The bug is open with no upstream fix as of
# 1.93.1/1.94.0. cargo's [build]/[target] rustflags do not reach the product_e2e test
# crate, so the env var is the reliable channel.
#
# SWAGGER_UI_DOWNLOAD_URL (offline swagger-ui zip) is handled in .cargo/local-machine.toml.

# Only inject if the caller hasn't set a conflicting RUSTFLAGS already.
case "${RUSTFLAGS:-}" in
    *dead_code*) ;;  # already suppressing; leave as-is
    *)
        if [ -n "${RUSTFLAGS:-}" ]; then
            RUSTFLAGS="$RUSTFLAGS -A dead_code"
        else
            RUSTFLAGS="-A dead_code"
        fi
        ;;
esac
export RUSTFLAGS
echo "[e2e-env] RUSTFLAGS=$RUSTFLAGS"
