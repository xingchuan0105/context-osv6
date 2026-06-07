"""Exception types for avrag_sdk."""

from __future__ import annotations

from typing import Optional


class AvragError(Exception):
    """Base exception for avrag_sdk."""


class AvragAPIError(AvragError):
    """HTTP error from the avrag Rust backend."""

    def __init__(self, status_code: int, message: str, body: Optional[str] = None):
        self.status_code = status_code
        self.message = message
        self.body = body
        super().__init__(f"avrag API error {status_code}: {message}")


class AvragTimeoutError(AvragError):
    """Request to avrag backend timed out."""


class AvragAuthError(AvragError):
    """Auth context missing or invalid."""
