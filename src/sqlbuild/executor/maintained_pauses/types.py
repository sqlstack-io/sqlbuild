"""Maintained pause event types."""

from __future__ import annotations

from enum import StrEnum

type StateLocation = tuple[str | None, str]


class MaintainedPauseEventType(StrEnum):
    """What one row of the pause table records."""

    PAUSED = "paused"
    RESUMED = "resumed"
    HEARTBEAT = "heartbeat"
