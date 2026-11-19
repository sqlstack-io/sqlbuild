"""Drive the build scheduler's job-limited dispatch with stub workers that record concurrency."""

from __future__ import annotations

import threading
import time
from collections import deque
from queue import Queue
from typing import Any, cast

from sqlbuild._native import CompiledObjectKey, CompiledResourceType
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.executor.build.classes.build_scheduler import BuildScheduler
from sqlbuild.executor.build.models import BuildRuntimeParams, NodeCompletion
from sqlbuild.executor.run.models import ModelExecutionResult
from sqlbuild.executor.scheduling.classes.session_warehouses import SessionWarehouses
from sqlbuild.executor.scheduling.types import ExecutionStatus
from tests.unit.src.sqlbuild.executor.build.classes._test_types import (
    JobLimitedBuildTestCase,
    SubworkerReservationTestCase,
)
from tests.unit.src.sqlbuild.executor.scheduling.classes.helpers import RecordingWarehouseAdapter

# How long each stub node runs, long enough for the loop to start every node it may.
_NODE_SECONDS: float = 0.05
_ALL_NODES: str = "*"
_STATUS_WHEN_FAILING: dict[bool, ExecutionStatus] = {
    True: ExecutionStatus.FAILED,
    False: ExecutionStatus.SUCCESS,
}


def model_key(name: str) -> CompiledObjectKey:
    """The key of model `name`."""

    return CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=name)


def _bare_scheduler(
    *, job_limits: dict[str, int], node_jobs: dict[str, tuple[str, ...]], max_concurrency: int
) -> Any:
    scheduler: Any = object.__new__(BuildScheduler)
    scheduler._ready = deque()
    scheduler._in_flight = set()
    scheduler._blocked_keys = set()
    scheduler._max_concurrency = max_concurrency
    scheduler._stop = False
    scheduler._microbatch_coordinator_lock = threading.Lock()
    scheduler._microbatch_coordinators = set()
    scheduler._microbatch_coordinator_demand = 0
    scheduler._microbatch_subworkers = 0
    scheduler._completion_queue = Queue()
    scheduler._session_warehouses = None
    scheduler._node_warehouses = {}
    scheduler._init_job_limits(
        BuildRuntimeParams(
            run_id="run",
            job_limits=job_limits,
            node_jobs={model_key(name): jobs for name, jobs in node_jobs.items()},
        )
    )
    return scheduler


def run_job_limited_build(
    test_case: JobLimitedBuildTestCase,
) -> tuple[dict[str, int], tuple[str, ...]]:
    """Each job's, and the whole build's, peak running nodes, and the nodes skipped."""

    scheduler: Any = _bare_scheduler(
        job_limits=test_case.job_limits,
        node_jobs=test_case.node_jobs,
        max_concurrency=test_case.max_concurrency,
    )
    lock: threading.Lock = threading.Lock()
    running: dict[str, int] = {}
    peaks: dict[str, int] = {}
    finished: set[str] = set()
    unbuilt: set[str] = set()
    skipped: list[str] = []
    scheduler._ready.extend(
        model_key(name)
        for name in filter(lambda node: not test_case.upstream[node], test_case.upstream)
    )

    dependents: dict[str, tuple[str, ...]] = {
        name: tuple(filter(lambda node: name in test_case.upstream[node], test_case.upstream))
        for name in test_case.upstream
    }

    def release(name: str) -> None:
        finished.add(name)
        ready: tuple[str, ...] = tuple(
            filter(lambda node: set(test_case.upstream[node]) <= finished, dependents[name])
        )
        scheduler._blocked_keys.update(
            model_key(node)
            for node in filter(lambda node: bool(unbuilt & set(test_case.upstream[node])), ready)
        )
        scheduler._ready.extend(model_key(node) for node in ready)

    def count(name: str, change: int) -> None:
        with lock:
            for group in (*test_case.node_jobs.get(name, ()), _ALL_NODES):
                running[group] = running.get(group, 0) + change
                peaks[group] = max(peaks.get(group, 0), running[group])

    def worker(key: CompiledObjectKey) -> None:
        count(key.name, 1)
        time.sleep(_NODE_SECONDS)
        count(key.name, -1)
        status: ExecutionStatus = _STATUS_WHEN_FAILING[key.name in test_case.failing]
        scheduler._completion_queue.put(
            NodeCompletion(key=key, result=ModelExecutionResult(model_name=key.name, status=status))
        )

    def handle_completion(*, key: CompiledObjectKey, result: ModelExecutionResult) -> None:
        unbuilt.update({key.name} & test_case.failing)
        release(key.name)

    def mark_complete(key: CompiledObjectKey) -> None:
        unbuilt.add(key.name)
        release(key.name)

    scheduler._worker = worker
    scheduler._handle_completion = handle_completion
    scheduler._mark_complete = mark_complete
    scheduler._record_skipped = lambda key: skipped.append(key.name)
    scheduler._pre_dispatch = lambda _key: True
    scheduler._is_concurrent_microbatch = lambda _key: False
    scheduler._report_scheduler_state = lambda: None
    scheduler._run_concurrent()
    return (
        {group: peaks.get(group, 0) for group in test_case.expected_peaks},
        tuple(sorted(skipped)),
    )


def reserve_subworkers(test_case: SubworkerReservationTestCase) -> tuple[int, tuple[bool, ...]]:
    """The coordinator demand with the queued coordinators, then each reservation's outcome."""

    queued: dict[str, tuple[str, ...]] = {
        f"queued_{index}": jobs for index, jobs in enumerate(test_case.queued_coordinator_jobs)
    }
    scheduler: Any = _bare_scheduler(
        job_limits=test_case.job_limits,
        node_jobs={"coordinator": test_case.coordinator_jobs, **queued},
        max_concurrency=test_case.max_concurrency,
    )
    scheduler._job_running = dict(test_case.job_running)
    scheduler._microbatch_coordinators = {model_key("coordinator")}
    scheduler._ready.extend(model_key(name) for name in queued)
    scheduler._is_concurrent_microbatch = lambda _key: True
    scheduler._update_microbatch_coordinator_demand()
    reservations: tuple[bool, ...] = tuple(
        scheduler._reserve_microbatch_subworker(test_case.coordinator_jobs)
        for _ in test_case.expected_reservations
    )
    return scheduler._microbatch_coordinator_demand, reservations


def warehouse_scheduler(adapter: RecordingWarehouseAdapter) -> Any:
    """A scheduler with only the state warehouse switching and sub-worker slots read."""

    scheduler: Any = object.__new__(BuildScheduler)
    scheduler._session_warehouses = SessionWarehouses(
        adapter=cast(BaseAdapter, adapter), home="BUILD_WH"
    )
    scheduler._node_warehouses = {}
    scheduler._microbatch_coordinator_lock = threading.Lock()
    scheduler._job_lock = threading.Lock()
    scheduler._job_limits = {}
    scheduler._job_running = {}
    scheduler._microbatch_subworkers = 1
    scheduler._connection_pool = Queue()
    return scheduler
