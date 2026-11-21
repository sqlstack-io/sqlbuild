"""Public enum vocabulary and the sensor result alias only; event processing belongs to Rust."""

from collections.abc import Iterable
from enum import StrEnum
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from sqlbuild.orchestration_declarations.models.authoring import RunRequest, SkipReason


class PlatformEventKind(StrEnum):
    """Launch failures include response errors and do not prove worker absence."""

    HEARTBEAT = "heartbeat"
    SCHEDULER_NO_LEADER = "scheduler_no_leader"
    TICK_OVERDUE = "tick_overdue"
    CALENDAR_HORIZON_LOW = "calendar_horizon_low"
    LOCATION_BROKEN = "location_broken"
    LAUNCH_FAILED = "launch_failed"
    RUN_FAILED = "run_failed"
    TRIGGER_SKIPPED = "trigger_skipped"
    SENSOR_RECHECKED = "sensor_rechecked"
    CALENDAR_TIMES_DROPPED = "calendar_times_dropped"


class RunEventKind(StrEnum):
    """COMPLETED selects every terminal outcome; it is not a run status."""

    COMPLETED = "completed"
    SUCCESS = "success"
    WARN = "warn"
    PARTIAL = "partial"
    FAILED = "failed"
    CANCELLED = "cancelled"
    SKIPPED = "skipped"


class NodeEventKind(StrEnum):
    """Actual execution drives success and transitions into or out of failure."""

    SUCCESS = "success"
    FAILED = "failed"
    RECOVERED = "recovered"


class CheckEventKind(StrEnum):
    """A check, audit or SQL test inside a run that failed or warned, whatever the run's outcome."""

    FAILED = "failed"
    WARN = "warn"


type SensorResult = RunRequest | Iterable[RunRequest] | SkipReason | None
