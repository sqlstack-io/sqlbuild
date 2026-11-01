"""Public enum vocabulary only; event processing belongs to Rust."""

from enum import StrEnum


class PlatformEventKind(StrEnum):
    """Launch failures include response errors and do not prove worker absence."""

    HEARTBEAT = "heartbeat"
    SCHEDULER_NO_LEADER = "scheduler_no_leader"
    TICK_OVERDUE = "tick_overdue"
    CALENDAR_HORIZON_LOW = "calendar_horizon_low"
    LOCATION_BROKEN = "location_broken"
    LAUNCH_FAILED = "launch_failed"


class RunEventKind(StrEnum):
    """COMPLETED selects every terminal outcome; it is not a run status."""

    COMPLETED = "completed"
    SUCCESS = "success"
    WARN = "warn"
    PARTIAL = "partial"
    FAILED = "failed"
    CANCELLED = "cancelled"
    PRESUMED_FAILED = "presumed_failed"


class NodeEventKind(StrEnum):
    """Actual execution drives success and transitions into or out of failure."""

    SUCCESS = "success"
    FAILED = "failed"
    RECOVERED = "recovered"
