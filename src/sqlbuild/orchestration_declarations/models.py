"""Authoring records only; selection, validation and scheduling belong to Rust."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Literal


@dataclass(frozen=True, kw_only=True)
class Schedule:
    """A wall-clock cron schedule attached to a job."""

    cron: str
    timezone: str


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
    concurrency_groups: tuple[str, ...] = field(default=(), kw_only=True)
    schedule: Schedule | None = field(default=None, kw_only=True)
    targets: tuple[str, ...] = field(default=(), kw_only=True)
