from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class CacheInvalidationTestCase:
    """Statements executed between two identical column lookups of ``orders``."""

    description: str
    transactional_ddl: bool
    statements_between: tuple[str, ...]
    expected_reads: int


@dataclass(frozen=True)
class OverlappingReadTestCase:
    """A statement that finishes while a lookup of ``orders`` is in flight."""

    description: str
    statement_during_read: str
    expected_reads: int


@dataclass(frozen=True)
class LiveInvalidationTestCase:
    """DDL executed through the adapter between two lookups on a real DuckDB connection."""

    description: str
    ddl: str
    expected_first_columns: tuple[str, ...]
    expected_second_columns: tuple[str, ...]
    expected_second_exists: bool


@dataclass(frozen=True)
class ListingExpiryTestCase:
    """A schema listing refreshed at time zero and looked up again some seconds later."""

    description: str
    seconds_later: float
    expected_listings: int
