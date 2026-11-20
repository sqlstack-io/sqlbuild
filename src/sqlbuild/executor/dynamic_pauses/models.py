"""Records of the dynamic pause lease."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime

from sqlbuild._native import CompiledRelationLocation
from sqlbuild.executor.dynamic_pauses.types import DynamicPauseEventType


@dataclass(frozen=True)
class DynamicPauseEvent:
    """One row to append: a pause, a resume, or a heartbeat of the run holding models."""

    event_type: DynamicPauseEventType
    run_id: str
    recorded_by: str
    relation: CompiledRelationLocation | None
    sequence: int = 0


@dataclass(frozen=True)
class OpenClaim:
    """A sync's claim on a model it may be resuming, and its age by the warehouse clock."""

    run_id: str
    relation: CompiledRelationLocation
    age_seconds: float


@dataclass(frozen=True)
class OpenHold:
    """A model a run paused and nobody resumed, when it paused, and how long the run is silent."""

    run_id: str
    relation: CompiledRelationLocation
    idle_seconds: float
    paused_at: datetime


@dataclass(frozen=True)
class HoldsResumed:
    """The holds whose models resumed and were closed, and each hold that failed with its error."""

    resumed: tuple[OpenHold, ...]
    failures: tuple[tuple[OpenHold, Exception], ...]


@dataclass(frozen=True)
class ResumeOutcome:
    """A pass's resumed holds and failures, and the pause table's token when it left none open."""

    resumed: tuple[OpenHold, ...]
    failures: tuple[str, ...]
    quiet_token: str | None
