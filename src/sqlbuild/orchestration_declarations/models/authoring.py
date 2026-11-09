"""Authoring records only; selection, validation and scheduling belong to Rust."""

from __future__ import annotations

from collections.abc import Callable, Iterable, Sequence
from dataclasses import dataclass, field, replace
from datetime import datetime
from typing import Literal

from sqlbuild._native import declaration_site
from sqlbuild.orchestration_declarations.types import NodeEventKind, PlatformEventKind, RunEventKind


@dataclass(frozen=True, kw_only=True)
class Schedule:
    """Cron with an RFC 3339 start; missed times collapse into one run of the latest."""

    cron: str
    timezone: str
    starts_at: str | None = None
    default_status: Literal["running", "paused"] = "running"


@dataclass(frozen=True, eq=False)
class Job:
    """A named selection to execute, optionally on a wall-clock schedule."""

    name: str
    description: str = field(kw_only=True)
    select: str = field(kw_only=True)
    exclude: str = field(default="", kw_only=True)
    settings: dict[str, object] = field(default_factory=dict, kw_only=True)
    concurrency: int | None = field(default=None, kw_only=True)
    concurrency_groups: tuple[str, ...] = field(default=(), kw_only=True)
    schedule: Schedule | None = field(default=None, kw_only=True)
    targets: tuple[str, ...] = field(default=(), kw_only=True)
    _declared_at: tuple[str, int] | None = field(default=None, init=False, repr=False)

    def __post_init__(self) -> None:
        object.__setattr__(self, "_declared_at", declaration_site())


@dataclass(frozen=True, eq=False)
class JobGroup:
    """Jobs ordered by compiled lineage and optional explicit dependencies."""

    name: str
    description: str = field(kw_only=True)
    jobs: Sequence[Job] = field(kw_only=True)
    after: dict[Job, Sequence[Job]] = field(default_factory=dict, kw_only=True)
    concurrency_groups: tuple[str, ...] = field(default=(), kw_only=True)
    schedule: Schedule | None = field(default=None, kw_only=True)
    targets: tuple[str, ...] = field(default=(), kw_only=True)
    _declared_at: tuple[str, int] | None = field(default=None, init=False, repr=False)

    def __post_init__(self) -> None:
        object.__setattr__(self, "_declared_at", declaration_site())


@dataclass(frozen=True, kw_only=True)
class CalendarWindow:
    """Timezone-aware, half-open interval for an upcoming calendar evaluation."""

    start: datetime
    end: datetime


@dataclass(frozen=True, kw_only=True, eq=False)
class CalendarSchedule:
    """Decorate a location callback returning upcoming timezone-aware datetimes."""

    job: Job | JobGroup
    description: str
    horizon: str
    timezone: str
    starts_at: str | None = None
    default_status: Literal["running", "paused"] = "running"
    targets: tuple[str, ...] = ()
    function: Callable[..., Iterable[datetime]] | None = field(default=None, repr=False)

    def __call__(self, function: Callable[..., Iterable[datetime]]) -> CalendarSchedule:
        return replace(self, function=function)


@dataclass(frozen=True, kw_only=True)
class PlatformEventCondition:
    """Consume matching server observations in durable storage order."""

    kinds: Sequence[PlatformEventKind]
    kind: Literal["platform_event"] = field(default="platform_event", init=False)


@dataclass(frozen=True, kw_only=True)
class RunEventCondition:
    """Match scoped terminal outcomes, optionally requiring genuinely executed successful nodes."""

    job: Job | JobGroup
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


@dataclass(frozen=True, eq=False)
class Sensor:
    """Start a job for each matching event; event identity deduplicates replayed requests."""

    name: str
    description: str = field(kw_only=True)
    when: (
        PlatformEventCondition
        | RunEventCondition
        | NodeEventCondition
        | SourceNewDataCondition
        | SourceStaleCondition
        | None
    ) = field(default=None, kw_only=True)
    run: Job | JobGroup = field(kw_only=True)
    minimum_interval_seconds: int = field(default=30, kw_only=True)
    default_status: Literal["running", "paused"] = field(default="running", kw_only=True)
    targets: tuple[str, ...] = field(default=(), kw_only=True)
    function: Callable[..., Iterable[RunRequest] | SkipReason] | None = field(
        default=None, repr=False, kw_only=True
    )

    def __call__(self, function: Callable[..., Iterable[RunRequest] | SkipReason]) -> Sensor:
        return replace(self, function=function)


@dataclass(frozen=True, kw_only=True)
class RunRequest:
    """A sensor-local key deduplicates requests across evaluations and retries."""

    run_key: str


@dataclass(frozen=True)
class SkipReason:
    """Complete an evaluation without starting a run."""

    reason: str
