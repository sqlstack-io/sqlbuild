from enum import StrEnum
from typing import TypeAlias


class ScenarioSnapshotState(StrEnum):
    """Availability state for one durable local scenario snapshot."""

    FRESH = "fresh"
    MISSING = "missing"
    STALE = "stale"
    INVALID = "invalid"


class ScenarioLocalRunStatus(StrEnum):
    """User-facing outcome for one local scenario replay."""

    PASS = "PASS"
    FAIL = "FAIL"
    ERROR = "ERROR"
    SKIP = "SKIP"


LocalSqlColumns: TypeAlias = tuple[tuple[str, str], ...]
"""Result columns of one local statement as (name, Snowflake type) pairs."""
