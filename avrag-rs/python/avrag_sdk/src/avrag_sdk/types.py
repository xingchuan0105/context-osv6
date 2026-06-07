"""Type definitions for avrag_sdk.

These mirror the Rust types in `crates/retrieval-data-plane/src/lib.rs`
(ScoredChunk) and the tool return types in `crates/rag-core/src/runtime/tools/`.
"""

from __future__ import annotations

from typing import Any, Optional
from pydantic import BaseModel, Field


class Chunk(BaseModel):
    """A scored chunk from retrieval.

    Mirrors `avrag_retrieval_data_plane::ScoredChunk` (subset of fields
    that are useful inside the sandbox).
    """

    chunk_id: str
    doc_id: str
    content: str
    score: float
    source: str
    page: Optional[int] = None
    chunk_type: str = "text"
    metadata: dict[str, Any] = Field(default_factory=dict)


class Relation(BaseModel):
    """A graph relation (subject-predicate-object triple)."""

    subject: str
    predicate: str
    object: str
    score: float
    supporting_chunk_ids: list[str] = Field(default_factory=list)


class Document(BaseModel):
    """A document record from index_lookup."""

    doc_id: str
    title: Optional[str] = None
    source: Optional[str] = None
    metadata: dict[str, Any] = Field(default_factory=dict)


class Summary(BaseModel):
    """A document summary from doc_summary."""

    doc_id: str
    summary: str
    metadata: dict[str, Any] = Field(default_factory=dict)


class Metadata(BaseModel):
    """Document metadata fields from doc_metadata."""

    doc_id: str
    fields: dict[str, Any] = Field(default_factory=dict)


class WebResult(BaseModel):
    """A web search result from the search crate (Brave)."""

    title: str
    url: str
    snippet: str
    citation_index: Optional[int] = None
