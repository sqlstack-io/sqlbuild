"""Build connections follow each node's job warehouse, microbatch sub-workers included."""

from __future__ import annotations

from typing import Any

import pytest

from sqlbuild._native import CompiledObjectKey, CompiledResourceType
from sqlbuild.executor.build.classes.build_scheduler import BuildScheduler
from sqlbuild.executor.build.models import FunctionExecutionResult, SeedExecutionResult
from sqlbuild.executor.load.models import LoadExecutionResult
from sqlbuild.executor.run.models import BatchWindow
from sqlbuild.executor.testing.models import SqlTestExecutionResult
from tests.unit.src.sqlbuild.executor.build.classes._test_types import (
    NodeWarehouseFailureTestCase,
    SubworkerWarehouseTestCase,
)
from tests.unit.src.sqlbuild.executor.build.classes.helpers import (
    failed_node_result,
    node_key,
    source_plan,
    warehouse_scheduler,
)
from tests.unit.src.sqlbuild.executor.scheduling.classes.helpers import (
    MissingWarehouseAdapter,
    RecordingWarehouseAdapter,
)


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
    scheduler: Any = warehouse_scheduler(adapter)
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
            expected_message=(
                "cannot run on warehouse MISSING_WH: Object does not exist: warehouse MISSING_WH"
            ),
        ),
        NodeWarehouseFailureTestCase(
            description="a seed fails with its own kind's result",
            node_warehouse="MISSING_WH",
            expected_message=(
                "cannot run on warehouse MISSING_WH: Object does not exist: warehouse MISSING_WH"
            ),
            resource_type=CompiledResourceType.SEED,
            expected_result_type=SeedExecutionResult,
        ),
        NodeWarehouseFailureTestCase(
            description="a source fails with its own kind's result",
            node_warehouse="MISSING_WH",
            expected_message=(
                "cannot run on warehouse MISSING_WH: Object does not exist: warehouse MISSING_WH"
            ),
            resource_type=CompiledResourceType.SOURCE,
            expected_result_type=LoadExecutionResult,
        ),
        NodeWarehouseFailureTestCase(
            description="a udf fails with its own kind's result",
            node_warehouse="MISSING_WH",
            expected_message=(
                "cannot run on warehouse MISSING_WH: Object does not exist: warehouse MISSING_WH"
            ),
            resource_type=CompiledResourceType.UDF,
            expected_result_type=FunctionExecutionResult,
        ),
        NodeWarehouseFailureTestCase(
            description="a table_fn fails with its own kind's result",
            node_warehouse="MISSING_WH",
            expected_message=(
                "cannot run on warehouse MISSING_WH: Object does not exist: warehouse MISSING_WH"
            ),
            resource_type=CompiledResourceType.TABLE_FN,
            expected_result_type=FunctionExecutionResult,
        ),
        NodeWarehouseFailureTestCase(
            description="a sql_test fails with its own kind's result",
            node_warehouse="MISSING_WH",
            expected_message=(
                "cannot run on warehouse MISSING_WH: Object does not exist: warehouse MISSING_WH"
            ),
            resource_type=CompiledResourceType.SQL_TEST,
            expected_result_type=SqlTestExecutionResult,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_unusable_job_warehouse_when_a_node_starts_then_it_fails_naming_the_warehouse(
    test_case: NodeWarehouseFailureTestCase,
) -> None:
    adapter: MissingWarehouseAdapter = MissingWarehouseAdapter(session_warehouse=None)
    scheduler: Any = warehouse_scheduler(adapter)
    key: CompiledObjectKey = node_key(resource_type=test_case.resource_type, name="orders")
    scheduler._node_warehouses = {key: test_case.node_warehouse}
    scheduler._plan = source_plan("orders")

    result: Any = BuildScheduler._enter_node_warehouse(scheduler, key=key, connection=object())

    assert (type(result), failed_node_result(result)) == (
        test_case.expected_result_type,
        (True, test_case.expected_message),
    )
