"""Public enum vocabulary only; event processing belongs to Rust."""

from enum import StrEnum


class PlatformEventKind(StrEnum):
    """Closed vocabulary of durable platform observations."""

    HEARTBEAT = "heartbeat"
    SCHEDULER_NO_LEADER = "scheduler_no_leader"
    TICK_OVERDUE = "tick_overdue"
    CALENDAR_HORIZON_LOW = "calendar_horizon_low"
