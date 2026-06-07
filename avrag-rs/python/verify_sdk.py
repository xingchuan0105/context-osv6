#!/usr/bin/env python3
"""verify_sdk.py — verify that avrag_sdk is correctly installed.

Run as: `python3 python/verify_sdk.py`
Exit code 0 if installed and importable; 1 otherwise.

This is a smoke test for the worker's Python environment. It checks:
1. `avrag_sdk` is importable
2. The client can be constructed
3. The expected methods are present (no signature drift)
"""

from __future__ import annotations

import sys


def main() -> int:
    try:
        import avrag_sdk
    except ImportError as e:
        print(f"❌ avrag_sdk not importable: {e}", file=sys.stderr)
        return 1

    print(f"✅ avrag_sdk imported from {avrag_sdk.__file__}")

    # Check expected public API
    from avrag_sdk import (
        AvragClient,
        Chunk,
        Relation,
        Document,
        Summary,
        Metadata,
        WebResult,
        AvragError,
        AvragAPIError,
    )

    expected_methods = [
        "dense", "dense_batch",
        "lexical",
        "graph",
        "index_lookup",
        "doc_summary", "doc_metadata",
        "rerank", "rerank_batch",
        "web_search",
    ]

    client_cls = AvragClient
    missing = [m for m in expected_methods if not hasattr(client_cls, m)]

    if missing:
        print(f"❌ AvragClient missing methods: {missing}", file=sys.stderr)
        return 1

    print(f"✅ AvragClient has all {len(expected_methods)} expected methods")

    # Construct a client (don't make HTTP calls, just check it works)
    client = AvragClient(base_url="http://localhost:9999")
    print(f"✅ AvragClient can be constructed (base_url={client.base_url})")

    # Module-level singleton
    from avrag_sdk import client as default_client
    assert isinstance(default_client, AvragClient)
    print("✅ Module-level singleton client exists")

    print()
    print("All verifications passed. Sandbox is ready.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
