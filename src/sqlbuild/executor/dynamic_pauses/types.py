"""Dynamic pause event types."""

from __future__ import annotations

from enum import StrEnum

type StateLocation = tuple[str | None, str]


class DynamicPauseEventType(StrEnum):
    """What one row of the pause table records."""

    PAUSED = "paused"
    RESUMED = "resumed"
    HEARTBEAT = "heartbeat"
    RESUMING = "resuming"
    """A sync's claim on a model it is about to resume, closed by a `RESUMED` row of its run."""
