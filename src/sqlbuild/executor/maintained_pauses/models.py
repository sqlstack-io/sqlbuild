"""Records of the maintained pause lease."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime

from sqlbuild._native import CompiledRelationLocation
from sqlbuild.executor.maintained_pauses.types import MaintainedPauseEventType


@dataclass(frozen=True)
class MaintainedPauseEvent:
    """One row to append: a pause, a resume, or a heartbeat of the run holding models."""

    event_type: MaintainedPauseEventType
    run_id: str
    recorded_by: str
    relation: CompiledRelationLocation | None
    sequence: int = 0


@dataclass(frozen=True)
class OpenHold:
    """A model a run paused and nobody resumed, when it paused, and how long the run is silent."""

    run_id: str
    relation: CompiledRelationLocation
    idle_seconds: float
    paused_at: datetime


@dataclass(frozen=True)
class ResumeOutcome:
    """The holds one pass resumed, and the pause table's change token when it left none open."""

    resumed: tuple[OpenHold, ...]
    quiet_token: str | None
