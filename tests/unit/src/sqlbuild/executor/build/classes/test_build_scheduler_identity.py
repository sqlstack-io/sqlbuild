"""Identity propagation tests for primary concurrent scheduler submissions."""

from __future__ import annotations

import threading
from collections import deque
from queue import Queue
from typing import Any

import pytest

from sqlbuild._native import CompiledObjectKey, CompiledResourceType
from sqlbuild.executor.build.classes.build_scheduler import BuildScheduler
from sqlbuild.executor.build.models import NodeCompletion
from sqlbuild.executor.run.models import ModelExecutionResult
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.observability import (
    ExecutionIdentity,
    current_execution_identity,
    invocation_scope,
    run_scope,
)
from tests.unit.src.sqlbuild.executor.build.classes._test_types import SchedulerIdentityTestCase


@pytest.mark.parametrize(
    "test_case",
    [
        SchedulerIdentityTestCase(
            description="regular primary worker",
            expected_invocation_id="inv-regular",
            expected_run_id="run-regular",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_regular_primary_work_when_scheduler_submits_then_identity_is_propagated(
    test_case: SchedulerIdentityTestCase,
) -> None:
    key: CompiledObjectKey = CompiledObjectKey(CompiledResourceType.SOURCE, "orders")
    observed: list[ExecutionIdentity | None] = []
    scheduler: Any = object.__new__(BuildScheduler)
    scheduler._ready = deque((key,))
    scheduler._in_flight = set()
    scheduler._blocked_keys = set()
    scheduler._node_jobs = {}
    scheduler._job_limits = {}
    scheduler._job_running = {}
    scheduler._max_concurrency = 1
    scheduler._stop = False
    scheduler._microbatch_coordinator_lock = threading.Lock()
    scheduler._microbatch_coordinators = set()
    scheduler._microbatch_subworkers = 0
    scheduler._completion_queue = Queue()
    scheduler._is_concurrent_microbatch = lambda _key: False
    scheduler._pre_dispatch = lambda _key: True
    scheduler._mark_complete = lambda _key: None
    scheduler._report_scheduler_state = lambda: None
    scheduler._handle_completion = lambda **_kwargs: None

    def worker(worker_key: CompiledObjectKey) -> None:
        observed.append(current_execution_identity())
        scheduler._completion_queue.put(
            NodeCompletion(
                key=worker_key,
                result=ModelExecutionResult(
                    model_name=worker_key.name,
                    status=ExecutionStatus.SUCCESS,
                ),
            )
        )

    scheduler._worker = worker

    with invocation_scope(test_case.expected_invocation_id):
        with run_scope(test_case.expected_run_id):
            scheduler._run_concurrent()

    assert observed[0] is not None
    assert observed[0].invocation_id == test_case.expected_invocation_id
    assert observed[0].run_id == test_case.expected_run_id


@pytest.mark.parametrize(
    "test_case",
    [
        SchedulerIdentityTestCase(
            description="microbatch primary worker",
            expected_invocation_id="inv-microbatch",
            expected_run_id="run-microbatch",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_microbatch_primary_work_when_scheduler_submits_then_identity_is_propagated(
    test_case: SchedulerIdentityTestCase,
) -> None:
    key: CompiledObjectKey = CompiledObjectKey(CompiledResourceType.MODEL, "orders")
    observed: list[ExecutionIdentity | None] = []
    scheduler: Any = object.__new__(BuildScheduler)
    scheduler._ready = deque((key,))
    scheduler._in_flight = set()
    scheduler._blocked_keys = set()
    scheduler._node_jobs = {}
    scheduler._job_limits = {}
    scheduler._job_running = {}
    scheduler._max_concurrency = 2
    scheduler._stop = False
    scheduler._microbatch_coordinator_lock = threading.Lock()
    scheduler._microbatch_coordinators = set()
    scheduler._microbatch_subworkers = 0
    scheduler._completion_queue = Queue()
    scheduler._is_concurrent_microbatch = lambda _key: True
    scheduler._pre_dispatch = lambda _key: True
    scheduler._mark_complete = lambda _key: None
    scheduler._report_scheduler_state = lambda: None
    scheduler._handle_completion = lambda **_kwargs: None

    def worker(*, key: CompiledObjectKey, pool: object) -> None:
        del pool
        observed.append(current_execution_identity())
        scheduler._completion_queue.put(
            NodeCompletion(
                key=key,
                result=ModelExecutionResult(
                    model_name=key.name,
                    status=ExecutionStatus.SUCCESS,
                ),
            )
        )

    scheduler._concurrent_microbatch_worker = worker

    with invocation_scope(test_case.expected_invocation_id):
        with run_scope(test_case.expected_run_id):
            scheduler._run_concurrent()

    assert observed[0] is not None
    assert observed[0].invocation_id == test_case.expected_invocation_id
    assert observed[0].run_id == test_case.expected_run_id
