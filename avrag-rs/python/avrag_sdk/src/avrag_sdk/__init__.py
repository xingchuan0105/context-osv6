"""avrag_sdk — Python SDK for avrag-rs retrieval primitives.

Used inside the code-interpreter sandbox. The model writes Python that
imports `avrag_sdk`, calls retrieval primitives, and returns chunks.

The SDK is a thin HTTP client — all heavy lifting happens in the Rust
backend (crates/retrieval-data-plane + bins/api). Python is for
orchestration only.

Example:
    from avrag_sdk import client

    # Simple: one-shot retrieval
    chunks = await client.dense("XX 客户 合同", k=10)

    # Complex: parallel + iterative
    import asyncio
    tasks = [client.dense(q, k=20) for q in queries]
    results = await asyncio.gather(*tasks)

Design notes:
    - All methods are async (httpx-based)
    - Methods accept Python primitives; return pydantic models
    - `*_batch` variants exist for `dense` and `rerank` to amortize
      external API cost when the model wants to call with multiple
      queries/candidates
"""

from avrag_sdk.client import AvragClient
from avrag_sdk.types import (
    Chunk,
    Relation,
    Document,
    Summary,
    Metadata,
    WebResult,
)
from avrag_sdk.exceptions import (
    AvragError,
    AvragAPIError,
    AvragTimeoutError,
    AvragAuthError,
)

# Module-level singleton — the sandbox image has one client per process.
# Override `base_url` via env var AVRAG_API_URL or constructor argument.
client = AvragClient()

__all__ = [
    "AvragClient",
    "client",
    "Chunk",
    "Relation",
    "Document",
    "Summary",
    "Metadata",
    "WebResult",
    "AvragError",
    "AvragAPIError",
    "AvragTimeoutError",
    "AvragAuthError",
]
