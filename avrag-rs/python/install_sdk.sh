#!/usr/bin/env bash
# install_sdk.sh — install the avrag-sdk Python package into the current
# Python environment so that the code-interpreter sandbox can import it.
#
# Usage:
#   ./python/install_sdk.sh              # editable install (dev)
#   ./python/install_sdk.sh --release    # non-editable (CI / production)
#   ./python/install_sdk.sh --user       # install to user site-packages
#   ./python/install_sdk.sh --verify     # just verify install, don't install
#
# This script is idempotent — re-running it is safe.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SDK_DIR="$SCRIPT_DIR/avrag_sdk"
MODE="editable"
TARGET=""
VERIFY_ONLY=false

for arg in "$@"; do
    case "$arg" in
        --release)  MODE="release" ;;
        --user)     TARGET="--user" ;;
        --verify)   VERIFY_ONLY=true ;;
        -h|--help)
            echo "Usage: $0 [--release] [--user] [--verify]"
            echo "  --release  Non-editable install (CI / production)"
            echo "  --user     Install to user site-packages"
            echo "  --verify   Just check if SDK is importable"
            exit 0
            ;;
        *)
            echo "Unknown argument: $arg" >&2
            exit 1
            ;;
    esac
done

if [[ ! -d "$SDK_DIR" ]]; then
    echo "error: SDK directory not found: $SDK_DIR" >&2
    exit 1
fi

# Verify install: try to import the SDK and check version
verify_install() {
    if python3 -c "import avrag_sdk; print('avrag_sdk installed at', avrag_sdk.__file__)" 2>/dev/null; then
        return 0
    else
        return 1
    fi
}

if [[ "$VERIFY_ONLY" == "true" ]]; then
    if verify_install; then
        exit 0
    else
        echo "avrag_sdk is NOT importable in current Python environment" >&2
        echo "Run this script without --verify to install it." >&2
        exit 1
    fi
fi

# Pre-install check: is it already importable?
if verify_install; then
    echo "avrag_sdk is already installed. Skipping."
    echo "Re-run with --force (TODO) to reinstall."
    exit 0
fi

# Install
# We always pass --break-system-packages because the worker's Python is
# system-managed (the code-interpreter crate spawns `python3` directly).
# In a CI / production environment, prefer activating a venv first and
# omitting this flag. The flag is a no-op on pip versions that don't
# support it.
PEP668_FLAG="--break-system-packages"

case "$MODE" in
    editable)
        echo "Installing avrag_sdk in editable mode from $SDK_DIR"
        python3 -m pip install --upgrade pip $PEP668_FLAG 2>/dev/null || true
        python3 -m pip install $TARGET $PEP668_FLAG -e "$SDK_DIR"
        ;;
    release)
        echo "Building wheel and installing avrag_sdk"
        python3 -m pip install --upgrade pip $PEP668_FLAG 2>/dev/null || true
        python3 -m pip install $TARGET $PEP668_FLAG "$SDK_DIR"
        ;;
esac

# Post-install verification
echo
echo "Verifying install..."
if verify_install; then
    echo
    echo "✅ avrag_sdk installed successfully."
else
    echo
    echo "❌ Install completed but import failed. Check Python environment." >&2
    exit 1
fi
