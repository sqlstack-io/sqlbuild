"""A Python node whose job warehouse the session cannot use fails alone, with its own result."""

from __future__ import annotations

from typing import Any

import pytest

from sqlbuild._native import DiscoveredCheckFunction, PythonNodeGraph
from sqlbuild.compiler.python_nodes.types import PythonNodeKind, PythonNodeStatus
from sqlbuild.executor.python_nodes._helpers.ingress_execution import (
    execute_ingress_python_loader_nodes,
)
from sqlbuild.executor.python_nodes._helpers.python_checks import execute_python_check_nodes
from sqlbuild.executor.python_nodes.models import (
    IngressCallbacks,
    PythonCheckCallbacks,
    PythonCheckExecutionResult,
    PythonIngressLoaderExecutorResult,
    PythonNodeExecutionResult,
    PythonNodeRunState,
)
from tests.unit.src.sqlbuild.executor.python_nodes._helpers._test_types import (
    CheckWarehouseFailureTestCase,
    IngressWarehouseFailureTestCase,
)
from tests.unit.src.sqlbuild.executor.python_nodes._helpers.helpers import (
    MISSING_WAREHOUSE_FAILURE,
    build_ingress_task_loader_graph,
    build_python_check_graph,
    ingress_calls,
    ingress_loader_function,
    ingress_source_map,
    missing_warehouse_runtime,
    python_check_function_for_case,
    reset_ingress_calls,
)
from tests.unit.src.sqlbuild.executor.scheduling.classes.helpers import MissingWarehouseAdapter


@pytest.mark.parametrize(
    "test_case",
    [
        IngressWarehouseFailureTestCase(
            description="a loader that cannot switch fails as a load, after its task ran",
            missing_warehouse_node="load_ingress_orders",
            expected_python=(("prepare_ingress_orders", PythonNodeStatus.SUCCESS, None),),
            expected_loads=(("raw_orders", "failed", MISSING_WAREHOUSE_FAILURE),),
            expected_call_order=("prepare_ingress_orders",),
        ),
        IngressWarehouseFailureTestCase(
            description="a task that cannot switch fails, and its loader never runs",
            missing_warehouse_node="prepare_ingress_orders",
            expected_python=(
                ("prepare_ingress_orders", PythonNodeStatus.FAILED, MISSING_WAREHOUSE_FAILURE),
            ),
            expected_loads=(("raw_orders", "skipped", None),),
            expected_call_order=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_unusable_job_warehouse_when_running_ingress_then_only_that_node_fails(
    test_case: IngressWarehouseFailureTestCase,
) -> None:
    reset_ingress_calls()
    graph: PythonNodeGraph = build_ingress_task_loader_graph()
    adapter: MissingWarehouseAdapter = MissingWarehouseAdapter(session_warehouse=None)

    result: PythonIngressLoaderExecutorResult = execute_ingress_python_loader_nodes(
        python_graph=graph,
        selected_python_names=frozenset({"prepare_ingress_orders", "load_ingress_orders"}),
        loader_functions=(ingress_loader_function(),),
        source_map=ingress_source_map(),
        runtime=missing_warehouse_runtime(adapter=adapter, node=test_case.missing_warehouse_node),
        callbacks=IngressCallbacks(),
    )

    assert (
        tuple((node.node_name, node.status, node.error_message) for node in result.python_results),
        tuple(
            (load.source_name, load.status.value, load.error_message)
            for load in result.load_results
        ),
        ingress_calls(),
    ) == (test_case.expected_python, test_case.expected_loads, test_case.expected_call_order)


@pytest.mark.parametrize(
    "test_case",
    [
        CheckWarehouseFailureTestCase(
            description="a ready check that cannot switch fails with the reason",
            upstream_status=PythonNodeStatus.SUCCESS,
            expected_error_message=MISSING_WAREHOUSE_FAILURE,
            expected_switch_attempts=1,
        ),
        CheckWarehouseFailureTestCase(
            description="a blocked check keeps its block and never switches",
            upstream_status=PythonNodeStatus.FAILED,
            expected_error_message="Upstream Python node failed: upstream_task",
            expected_switch_attempts=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_unusable_job_warehouse_when_running_a_check_then_it_fails_alone(
    test_case: CheckWarehouseFailureTestCase,
) -> None:
    check_function: DiscoveredCheckFunction = python_check_function_for_case("passes")
    adapter: MissingWarehouseAdapter = MissingWarehouseAdapter(session_warehouse=None)

    results: tuple[PythonCheckExecutionResult, ...] = execute_python_check_nodes(
        check_functions=(check_function,),
        python_graph=build_python_check_graph(check_function=check_function),
        upstream_python_results=(
            PythonNodeExecutionResult(
                node_name="upstream_task",
                kind=PythonNodeKind.ASSET,
                status=test_case.upstream_status,
            ),
        ),
        upstream_load_results=(),
        run_state=PythonNodeRunState(),
        runtime=missing_warehouse_runtime(adapter=adapter, node=check_function.name),
        callbacks=PythonCheckCallbacks(),
    )

    checked: Any = results[0]
    assert (
        checked.failed,
        checked.error_message,
        len(adapter.statements),
    ) == (True, test_case.expected_error_message, test_case.expected_switch_attempts)
