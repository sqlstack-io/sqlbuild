"""How a lifecycle scheduler runs some of its nodes beside each other."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass

from sqlbuild.executor.scheduling.classes.lifecycle_progress import LifecycleProgress
from sqlbuild.executor.scheduling.models import LifecycleExecutionNode


@dataclass(frozen=True)
class LifecycleConcurrency:
    """Which ready nodes may run beside each other, and what runs them until none is ready."""

    eligible: Callable[[LifecycleExecutionNode], bool]
    run: Callable[[LifecycleProgress], None]
