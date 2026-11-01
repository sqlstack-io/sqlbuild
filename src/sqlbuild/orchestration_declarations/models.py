"""Authoring records only; selection, validation and scheduling belong to Rust."""

from __future__ import annotations

from collections.abc import Callable, Iterable, Sequence
from dataclasses import dataclass, field, replace
from datetime import datetime
from typing import Literal

from sqlbuild.orchestration_declarations.types import PlatformEventKind


@dataclass(frozen=True, kw_only=True)
class Schedule:
    """Cron with an RFC 3339 start; catch_up='all' requires a positive max_catch_up."""

    cron: str
    timezone: str
    starts_at: str | None = None
    default_status: Literal["running", "paused"] = "running"
    catch_up: Literal["latest", "all"] = "latest"
    max_catch_up: int | None = None


@dataclass(frozen=True, eq=False)
class Job:
    """A named selection to execute, optionally on a wall-clock schedule."""

    name: str
    description: str = field(kw_only=True)
    select: str = field(kw_only=True)
    exclude: str = field(default="", kw_only=True)
    overlap: Literal["coalesce", "queue", "allow"] = field(default="coalesce", kw_only=True)
    on_failure: Literal["skip_dependents", "skip_downstream_jobs", "stop_group"] = field(
        default="skip_dependents", kw_only=True
    )
    settings: dict[str, object] = field(default_factory=dict, kw_only=True)
    concurrency: int | None = field(default=None, kw_only=True)
    concurrency_groups: tuple[str, ...] = field(default=(), kw_only=True)
    schedule: Schedule | None = field(default=None, kw_only=True)
    targets: tuple[str, ...] = field(default=(), kw_only=True)


@dataclass(frozen=True, eq=False)
class JobGroup:
    """Jobs ordered by compiled lineage and optional explicit dependencies."""

    name: str
    description: str = field(kw_only=True)
    jobs: Sequence[Job] = field(kw_only=True)
    after: dict[Job, Sequence[Job]] = field(default_factory=dict, kw_only=True)
    overlap: Literal["coalesce", "queue", "allow"] = field(default="coalesce", kw_only=True)
    concurrency_groups: tuple[str, ...] = field(default=(), kw_only=True)
    schedule: Schedule | None = field(default=None, kw_only=True)
    targets: tuple[str, ...] = field(default=(), kw_only=True)


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
    catch_up: Literal["latest", "all"] = "latest"
    max_catch_up: int | None = None
    targets: tuple[str, ...] = ()
    function: Callable[..., Iterable[datetime]] | None = field(default=None, repr=False)

    def __call__(self, function: Callable[..., Iterable[datetime]]) -> CalendarSchedule:
        return replace(self, function=function)


@dataclass(frozen=True, kw_only=True)
class PlatformEventCondition:
    """Consume matching server observations in durable storage order."""

    kinds: Sequence[PlatformEventKind]
    kind: Literal["platform_event"] = field(default="platform_event", init=False)


@dataclass(frozen=True, eq=False)
class Sensor:
    """Start a job for each matching event; event identity deduplicates replayed requests."""

    name: str
    description: str = field(kw_only=True)
    when: PlatformEventCondition = field(kw_only=True)
    run: Job | JobGroup = field(kw_only=True)
    minimum_interval_seconds: int = field(default=30, kw_only=True)
    default_status: Literal["running", "stopped"] = field(default="running", kw_only=True)
    targets: tuple[str, ...] = field(default=(), kw_only=True)
