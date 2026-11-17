"""Deterministic diagnostics tests for concurrent scheduler state."""

from __future__ import annotations

from collections import deque
from typing import Any

import pytest

from sqlbuild._native import CompiledObjectKey, CompiledResourceType
from sqlbuild.executor.build.classes.build_scheduler import BuildScheduler
from sqlbuild.executor.build.models import BuildCallbacks, SchedulerState
from tests.unit.src.sqlbuild.executor.build.classes._test_types import (
    SchedulerDiagnosticsTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        SchedulerDiagnosticsTestCase(
            description="deduplicates unchanged frontier-constrained state",
            expected_state_count=1,
            expected_running=1,
            expected_ready=0,
            expected_waiting=1,
            expected_limit=2,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unchanged_concurrent_state_when_reporting_then_emits_once(
    test_case: SchedulerDiagnosticsTestCase,
) -> None:
    completed: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.MODEL, name="completed"
    )
    running: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.MODEL, name="running"
    )
    waiting: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.MODEL, name="waiting"
    )
    states: list[SchedulerState] = []
    scheduler: Any = object.__new__(BuildScheduler)
    scheduler._callbacks = BuildCallbacks(on_scheduler_state=states.append)
    scheduler._selected_execution_keys = frozenset({completed, running, waiting})
    scheduler._completed_keys = {completed}
    scheduler._in_flight = {running}
    scheduler._ready = deque()
    scheduler._max_concurrency = test_case.expected_limit
    scheduler._last_scheduler_state = None
    scheduler._stop = False
    scheduler._node_gate = None
    scheduler._gated = []

    scheduler._report_scheduler_state()
    scheduler._report_scheduler_state()

    assert len(states) == test_case.expected_state_count
    assert states[0] == SchedulerState(
        running=test_case.expected_running,
        ready=test_case.expected_ready,
        waiting=test_case.expected_waiting,
        limit=test_case.expected_limit,
        aborted=test_case.expected_aborted,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SchedulerDiagnosticsTestCase(
            description="initial fail-fast failure reports every unresolved node as aborted",
            expected_state_count=1,
            expected_running=0,
            expected_ready=0,
            expected_waiting=0,
            expected_limit=4,
            expected_aborted=2,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_initial_fail_fast_failure_when_reporting_then_state_is_terminal(
    test_case: SchedulerDiagnosticsTestCase,
) -> None:
    first: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.MODEL, name="first"
    )
    second: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.MODEL, name="second"
    )
    states: list[SchedulerState] = []
    scheduler: Any = object.__new__(BuildScheduler)
    scheduler._callbacks = BuildCallbacks(on_scheduler_state=states.append)
    scheduler._selected_execution_keys = frozenset({first, second})
    scheduler._completed_keys = set()
    scheduler._in_flight = set()
    scheduler._ready = deque((first,))
    scheduler._max_concurrency = test_case.expected_limit
    scheduler._last_scheduler_state = None
    scheduler._stop = True
    scheduler._node_gate = None
    scheduler._gated = []

    scheduler._report_scheduler_state()

    assert states == [
        SchedulerState(
            running=test_case.expected_running,
            ready=test_case.expected_ready,
            waiting=test_case.expected_waiting,
            limit=test_case.expected_limit,
            aborted=test_case.expected_aborted,
        )
    ]


@pytest.mark.parametrize(
    "test_case",
    [
        SchedulerDiagnosticsTestCase(
            description="concurrent fail-fast drains running work before reporting aborted nodes",
            expected_state_count=2,
            expected_running=0,
            expected_ready=0,
            expected_waiting=0,
            expected_limit=2,
            expected_aborted=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_concurrent_fail_fast_when_workers_drain_then_final_state_is_terminal(
    test_case: SchedulerDiagnosticsTestCase,
) -> None:
    running: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.MODEL, name="running"
    )
    aborted: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.MODEL, name="aborted"
    )
    states: list[SchedulerState] = []
    scheduler: Any = object.__new__(BuildScheduler)
    scheduler._callbacks = BuildCallbacks(on_scheduler_state=states.append)
    scheduler._selected_execution_keys = frozenset({running, aborted})
    scheduler._completed_keys = set()
    scheduler._in_flight = {running}
    scheduler._ready = deque()
    scheduler._max_concurrency = test_case.expected_limit
    scheduler._last_scheduler_state = None
    scheduler._stop = True
    scheduler._node_gate = None
    scheduler._gated = []

    scheduler._report_scheduler_state()
    scheduler._in_flight.clear()
    scheduler._completed_keys.add(running)
    scheduler._report_scheduler_state()

    assert len(states) == test_case.expected_state_count
    assert states[-1] == SchedulerState(
        running=test_case.expected_running,
        ready=test_case.expected_ready,
        waiting=test_case.expected_waiting,
        limit=test_case.expected_limit,
        aborted=test_case.expected_aborted,
    )
