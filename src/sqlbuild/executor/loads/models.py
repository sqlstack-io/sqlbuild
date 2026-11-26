"""The SQLLoad project a load runs as, and what one run of it did."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class LoadProject:
    """The SQLLoad project text a load runs as, and the session its target writes through."""

    text: str
    session: str


@dataclass(frozen=True)
class LoadedTable:
    """One table's load: its rows, or why it failed, and the tables SQLLoad created or altered."""

    table: str
    rows: int | None
    error: str | None
    changes: tuple[str, ...]
    retryable: bool
    warnings: tuple[str, ...] = ()
