"""Scheduling models for lifecycle-aware mixed execution."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Any

from sqlbuild.compiler.python_nodes.types import SkipMode
from sqlbuild.executor.scheduling.types import LifecycleNodeStatus

if TYPE_CHECKING:
    from sqlbuild.executor.scheduling.classes.lifecycle_progress import LifecycleProgress


@dataclass(frozen=True)
class LifecycleExecutionNode:
    """One schedulable lifecycle execution node."""

    name: str
    kind: str
    upstream_names: tuple[str, ...] = field(default_factory=tuple)
    payload: Any | None = None


@dataclass(frozen=True)
class LifecycleNodeResult:
    """Generic lifecycle scheduler result for one node."""

    name: str
    kind: str
    status: LifecycleNodeStatus
    error_message: str | None = None
    skip_reason: str | None = None
    skip_mode: SkipMode | None = None


@dataclass(frozen=True)
class LifecycleSchedulerResult:
    """Result bundle for one serial lifecycle scheduler run."""

    results: tuple[LifecycleNodeResult, ...]


@dataclass(frozen=True)
class LifecycleConcurrency:
    """Which ready nodes may run beside each other, and what runs them until none is ready."""

    eligible: Callable[[LifecycleExecutionNode], bool]
    run: Callable[
        [LifecycleProgress],
        None,
    ]
