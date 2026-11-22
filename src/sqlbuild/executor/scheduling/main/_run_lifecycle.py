"""Public operation for lifecycle-aware serial scheduling."""

from __future__ import annotations

from collections.abc import Callable

from sqlbuild.executor.scheduling._helpers.lifecycle import (
    run_lifecycle_scheduler as _run_lifecycle_scheduler,
)
from sqlbuild.executor.scheduling.classes.lifecycle_concurrency import LifecycleConcurrency
from sqlbuild.executor.scheduling.models import (
    LifecycleExecutionNode,
    LifecycleNodeResult,
    LifecycleSchedulerResult,
)


def run_lifecycle_scheduler(
    *,
    nodes: tuple[LifecycleExecutionNode, ...],
    handler: Callable[[LifecycleExecutionNode], LifecycleNodeResult],
    concurrency: LifecycleConcurrency | None = None,
) -> LifecycleSchedulerResult:
    """Run lifecycle nodes serially in topological order, except those `concurrency` runs."""

    return _run_lifecycle_scheduler(nodes=nodes, handler=handler, concurrency=concurrency)
