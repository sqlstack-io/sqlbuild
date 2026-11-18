"""Drive the build scheduler's job-limited dispatch without running nodes."""

from __future__ import annotations

from collections import deque
from typing import Any

from sqlbuild._native import CompiledObjectKey, CompiledResourceType
from sqlbuild.executor.build.classes.build_scheduler import BuildScheduler
from tests.unit.src.sqlbuild.executor.build.classes._test_types import JobLimitDispatchTestCase


def model_key(name: str) -> CompiledObjectKey:
    """The key of model `name`."""

    return CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=name)


def dispatched_names(test_case: JobLimitDispatchTestCase) -> tuple[str | None, ...]:
    """Each dispatch's node after `finished_before` ("" for none) finishes, starting it."""

    scheduler: Any = object.__new__(BuildScheduler)
    scheduler._ready = deque(model_key(name) for name in test_case.ready)
    scheduler._in_flight = set()
    scheduler._blocked_keys = {model_key(name) for name in test_case.blocked}
    scheduler._job_limits = test_case.job_limits
    scheduler._node_jobs = {model_key(name): jobs for name, jobs in test_case.node_jobs.items()}
    scheduler._job_running = {}
    dispatched: list[str | None] = []
    for finished in test_case.finished_before:
        scheduler._finish_in_flight(model_key(finished))
        taken: CompiledObjectKey | None = scheduler._take_ready_key()
        scheduler._start_in_flight(taken)
        dispatched.append(getattr(taken, "name", None))
    return tuple(dispatched)
