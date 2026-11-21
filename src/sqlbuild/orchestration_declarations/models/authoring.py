"""Authoring records only; selection, validation and scheduling belong to Rust."""

from __future__ import annotations

from collections.abc import Callable, Iterable, Sequence
from dataclasses import dataclass, field, replace
from datetime import datetime
from typing import Literal

from sqlbuild._native import declaration_site
from sqlbuild.orchestration_declarations.types import (
    CheckEventKind,
    NodeEventKind,
    PlatformEventKind,
    RunEventKind,
    SensorResult,
)


@dataclass(frozen=True, kw_only=True)
class CalendarWindow:
    """Timezone-aware, half-open interval for an upcoming calendar evaluation."""

    start: datetime
    end: datetime


@dataclass(frozen=True, kw_only=True, eq=False)
class Calendar:
    """Decorate a location callback returning upcoming timezone-aware datetimes for schedules."""

    name: str
    description: str
    horizon: str
    function: Callable[..., Iterable[datetime]] | None = field(default=None, repr=False)
    _declared_at: tuple[str, int] | None = field(default=None, init=False, repr=False)

    def __post_init__(self) -> None:
        object.__setattr__(self, "_declared_at", declaration_site())

    def __call__(self, function: Callable[..., Iterable[datetime]]) -> Calendar:
        return replace(self, function=function)


@dataclass(frozen=True, kw_only=True)
class PlatformEventCondition:
    """Consume matching server observations in durable storage order."""

    kinds: Sequence[PlatformEventKind]
    kind: Literal["platform_event"] = field(default="platform_event", init=False)


@dataclass(frozen=True, kw_only=True)
class RunEventCondition:
    """Match a job's or group's terminal outcomes; listed nodes must have run and succeeded."""

    job: str
    kinds: Sequence[RunEventKind]
    executed_successfully: Sequence[str] | None = None
    kind: Literal["run_event"] = field(default="run_event", init=False)


@dataclass(frozen=True, kw_only=True)
class NodeEventCondition:
    """Match each successful rebuild in any run within the sensor's location and target."""

    select: str
    kinds: Sequence[NodeEventKind]
    kind: Literal["node_event"] = field(default="node_event", init=False)


@dataclass(frozen=True, kw_only=True)
class CheckEventCondition:
    """Match each check, audit or SQL test on the selected nodes failing or warning in a run."""

    select: str
    kinds: Sequence[CheckEventKind]
    kind: Literal["check_event"] = field(default="check_event", init=False)


@dataclass(frozen=True, kw_only=True)
class SourceNewDataCondition:
    """Match the first non-null source version and later changed versions, in receipt order."""

    select: str
    kind: Literal["source_new_data"] = field(default="source_new_data", init=False)


@dataclass(frozen=True, kw_only=True)
class SourceStaleCondition:
    """Match timestamp source versions older than after_seconds, once per version."""

    select: str
    after_seconds: int
    kind: Literal["source_stale"] = field(default="source_stale", init=False)


@dataclass(frozen=True, kw_only=True, eq=False)
class Sensor:
    """Decorate a function the location calls per event `when` matches, else every interval."""

    description: str
    when: (
        PlatformEventCondition
        | RunEventCondition
        | NodeEventCondition
        | CheckEventCondition
        | SourceNewDataCondition
        | SourceStaleCondition
        | None
    ) = None
    name: str | None = None
    minimum_interval_seconds: int = 30
    timeout_seconds: int = 60
    default_status: Literal["running", "paused"] = "running"
    targets: tuple[str, ...] = ()
    function: Callable[..., SensorResult] | None = field(default=None, repr=False)
    _declared_at: tuple[str, int] | None = field(default=None, init=False, repr=False)

    def __post_init__(self) -> None:
        object.__setattr__(self, "_declared_at", declaration_site())

    def __call__(self, function: Callable[..., SensorResult]) -> Sensor:
        return replace(
            self, function=function, name=self.name or getattr(function, "__name__", None)
        )


@dataclass(frozen=True, kw_only=True)
class RunRequest:
    """Start `job` once per `run_key`, which defaults to the event's ID and the job."""

    job: str
    run_key: str | None = None


@dataclass(frozen=True)
class SkipReason:
    """Handle the event, or this poll, without starting a run, and say why."""

    reason: str
