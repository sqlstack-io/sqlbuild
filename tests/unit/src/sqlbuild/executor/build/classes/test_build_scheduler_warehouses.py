"""Build connections follow each node's job warehouse, microbatch sub-workers included."""

from __future__ import annotations

import threading
from queue import Queue
from typing import Any, cast

import pytest

from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.executor.build.classes.build_scheduler import BuildScheduler
from sqlbuild.executor.run.models import BatchWindow, ModelExecutionResult
from sqlbuild.executor.scheduling.classes.session_warehouses import SessionWarehouses
from sqlbuild.executor.scheduling.types import ExecutionStatus
from tests.unit.src.sqlbuild.executor.build.classes._test_types import (
    NodeWarehouseFailureTestCase,
    SubworkerWarehouseTestCase,
)
from tests.unit.src.sqlbuild.executor.build.classes.helpers import model_key
from tests.unit.src.sqlbuild.executor.scheduling.classes.helpers import (
    RecordingWarehouseAdapter,
)


def _scheduler(adapter: RecordingWarehouseAdapter) -> Any:
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


@pytest.mark.parametrize(
    "test_case",
    [
        SubworkerWarehouseTestCase(
            description="a sub-worker moves its connection to the coordinator's job warehouse",
            coordinator_warehouse="ORDERS_WH",
            earlier_warehouse=None,
            expected_statements=("ORDERS_WH",),
        ),
        SubworkerWarehouseTestCase(
            description="a plain model's sub-worker returns a connection a job left elsewhere",
            coordinator_warehouse=None,
            earlier_warehouse="REFUNDS_WH",
            expected_statements=("REFUNDS_WH", "BUILD_WH"),
        ),
        SubworkerWarehouseTestCase(
            description="a connection already on the warehouse issues nothing more",
            coordinator_warehouse="ORDERS_WH",
            earlier_warehouse="ORDERS_WH",
            expected_statements=("ORDERS_WH",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_coordinator_warehouse_when_a_subworker_runs_a_batch_then_its_connection_follows(
    test_case: SubworkerWarehouseTestCase,
) -> None:
    adapter: RecordingWarehouseAdapter = RecordingWarehouseAdapter(session_warehouse=None)
    scheduler: Any = _scheduler(adapter)
    pooled: object = object()
    scheduler._session_warehouses.use(connection=pooled, warehouse=test_case.earlier_warehouse)
    scheduler._connection_pool.put(pooled)
    executed_on: list[object] = []

    def execute(_batch: BatchWindow, connection: object) -> Any:
        executed_on.append(connection)
        return "done"

    outcome: Any = BuildScheduler._run_microbatch_batch_worker(
        scheduler,
        jobs=(),
        warehouse=test_case.coordinator_warehouse,
        batch=BatchWindow(start="0", end="1", index=0),
        execute=execute,
    )

    assert (
        outcome,
        executed_on == [pooled],
        tuple(warehouse for _, warehouse in adapter.statements),
        scheduler._connection_pool.get_nowait() is pooled,
    ) == ("done", True, test_case.expected_statements, True)


@pytest.mark.parametrize(
    "test_case",
    [
        NodeWarehouseFailureTestCase(
            description="a job warehouse the session cannot use fails the node with the reason",
            node_warehouse="MISSING_WH",
            failing="MISSING_WH",
            expected_message=(
                "cannot run on warehouse MISSING_WH: Object does not exist: warehouse MISSING_WH"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_unusable_job_warehouse_when_a_node_starts_then_it_fails_naming_the_warehouse(
    test_case: NodeWarehouseFailureTestCase,
) -> None:
    adapter: RecordingWarehouseAdapter = RecordingWarehouseAdapter(
        session_warehouse=None, failing=test_case.failing
    )
    scheduler: Any = _scheduler(adapter)
    scheduler._node_warehouses = {model_key("orders"): test_case.node_warehouse}

    result: ModelExecutionResult | None = BuildScheduler._enter_node_warehouse(
        scheduler, key=model_key("orders"), connection=object()
    )

    assert result is not None
    assert (result.status, result.error_message) == (
        ExecutionStatus.FAILED,
        test_case.expected_message,
    )
