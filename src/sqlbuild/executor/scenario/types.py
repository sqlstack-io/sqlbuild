from enum import StrEnum


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


type LocalSqlColumns = tuple[tuple[str, str], ...]
