"""A group member's build holds gated nodes outside its slots until their inputs are built."""

from __future__ import annotations

import queue
import threading
from collections import deque
from types import SimpleNamespace
from typing import Any

import pytest

from sqlbuild._native import CompiledObjectKey, CompiledResourceType
from sqlbuild.executor.build.classes import build_scheduler
from sqlbuild.executor.build.classes.build_scheduler import BuildScheduler
from tests.unit.src.sqlbuild.executor.build.classes._test_types import SchedulerNodeGateTestCase


@pytest.mark.parametrize(
    "test_case",
    [
        SchedulerNodeGateTestCase(
            description="one slot runs the independent node while the gated one waits",
            max_concurrency=1,
            expected_executed=("independent", "waits_then_opens", "after_opened"),
            expected_skipped=("input_failed", "after_skipped"),
        ),
        SchedulerNodeGateTestCase(
            description="two slots run the independent node while the gated one waits",
            max_concurrency=2,
            expected_executed=("independent", "waits_then_opens", "after_opened"),
            expected_skipped=("input_failed", "after_skipped"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_gated_nodes_when_scheduling_then_waiting_nodes_hold_no_slot_and_skips_cascade(
    test_case: SchedulerNodeGateTestCase, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(build_scheduler, "_GATE_POLL_SECONDS", 0.01)
    keys: dict[str, CompiledObjectKey] = {
        name: CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=name)
        for name in (
            "waits_then_opens",
            "independent",
            "input_failed",
            "after_opened",
            "after_skipped",
        )
    }
    polls: dict[str, int] = {}
    executed: list[str] = []
    skipped: list[str] = []
    lock: threading.Lock = threading.Lock()

    planned: dict[str, deque[str | None]] = {
        "model:waits_then_opens": deque([None, None]),
        "model:input_failed": deque(["model:staged_orders"]),
    }

    def node_gate(names: list[str]) -> list[tuple[str, str | None]]:
        polls.update({name: polls.get(name, 0) + 1 for name in names})
        closed_now: filter[str] = filter(lambda name: bool(planned.get(name)), names)
        return list(map(lambda name: (name, planned[name].popleft()), closed_now))

    def execute_node(*, key: CompiledObjectKey, connection: Any) -> object:
        del connection
        with lock:
            executed.append(key.name)
        return SimpleNamespace(status="success")

    scheduler: Any = object.__new__(BuildScheduler)
    scheduler._ready = deque([keys["waits_then_opens"], keys["independent"], keys["input_failed"]])
    scheduler._in_flight = set()
    scheduler._stop = False
    scheduler._blocked_keys = set()
    scheduler._completed_keys = set()
    scheduler._selected_execution_keys = frozenset(keys.values())
    scheduler._in_degree = {keys["after_opened"]: 1, keys["after_skipped"]: 1}
    scheduler._plan = SimpleNamespace(
        downstream_deps={
            keys["waits_then_opens"]: (keys["after_opened"],),
            keys["input_failed"]: (keys["after_skipped"],),
        },
        execution_order=tuple(keys.values()),
    )
    scheduler._max_concurrency = test_case.max_concurrency
    scheduler._completion_queue = queue.Queue()
    scheduler._connection_pool = queue.Queue()
    for index in range(test_case.max_concurrency):
        scheduler._connection_pool.put(f"connection-{index}")
    scheduler._connection_recovery = SimpleNamespace(usable=lambda connection: connection)
    scheduler._microbatch_coordinator_lock = threading.Lock()
    scheduler._microbatch_coordinators = set()
    scheduler._microbatch_coordinator_demand = 0
    scheduler._microbatch_subworkers = 0
    scheduler._runtime = SimpleNamespace(microbatch_concurrency=False)
    scheduler._indexes = SimpleNamespace(model_entries_by_key={})
    scheduler._callbacks = SimpleNamespace(on_scheduler_state=None)
    scheduler._last_scheduler_state = None
    scheduler._node_gate = node_gate
    scheduler._gated = []
    scheduler._gates_checked_at = 0.0
    scheduler._pre_dispatch = lambda key: True
    scheduler._execute_node = execute_node
    scheduler._record_skipped = lambda key: skipped.append(key.name)
    scheduler._handle_completion = lambda *, key, result: BuildScheduler._mark_complete(
        scheduler, key
    )

    run_mode: dict[int, Any] = {1: BuildScheduler._run_serial, 2: BuildScheduler._run_concurrent}
    run_mode[test_case.max_concurrency](scheduler)

    assert (tuple(sorted(executed)), tuple(skipped)) == (
        tuple(sorted(test_case.expected_executed)),
        test_case.expected_skipped,
    )
    assert executed.index("independent") < executed.index("waits_then_opens")
    assert executed.index("waits_then_opens") < executed.index("after_opened")
    assert polls["model:waits_then_opens"] >= 3
