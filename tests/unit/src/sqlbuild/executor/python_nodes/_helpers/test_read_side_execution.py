"""Tests for Python read-side SQL-read Python execution tracking."""

from __future__ import annotations

import pytest

from sqlbuild._native import PythonNodeGraph
from sqlbuild.compiler.python_nodes.types import PythonNodeStatus
from sqlbuild.executor.python_nodes.classes.read_side_python_execution_tracker import (
    ReadSidePythonExecutionTracker,
)
from sqlbuild.executor.python_nodes.models import PythonNodeRuntime
from sqlbuild.executor.run.models import ModelExecutionResult
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.observability import (
    EventDispatcher,
    LifecycleEvent,
    dispatcher_scope,
    invocation_scope,
)
from tests.unit.src.sqlbuild.executor.python_nodes._helpers._test_types import (
    ReadSidePythonTrackerTestCase,
)
from tests.unit.src.sqlbuild.executor.python_nodes._helpers.helpers import (
    PythonNodeContextTestAdapter,
    build_read_side_sql_task_asset_graph,
    read_side_calls,
    reset_read_side_calls,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ReadSidePythonTrackerTestCase(
            description="runs sql dependent Python before unrelated SQL completes",
            selected_names=frozenset({"profile_stg_orders", "export_stg_profile"}),
            completed_sql_names=("stg_orders",),
            expected_result_names=("profile_stg_orders", "export_stg_profile"),
            expected_call_order=("profile_stg_orders", "export_stg_profile"),
            expected_statuses=(PythonNodeStatus.SUCCESS, PythonNodeStatus.SUCCESS),
            expected_skip_reasons=(None, None),
        ),
        ReadSidePythonTrackerTestCase(
            description="runs Python on an unscheduled SQL dependency without waiting",
            selected_names=frozenset({"profile_stg_orders", "export_stg_profile"}),
            completed_sql_names=(),
            scheduled_sql_names=frozenset(),
            expected_result_names=("profile_stg_orders", "export_stg_profile"),
            expected_call_order=("profile_stg_orders", "export_stg_profile"),
            expected_statuses=(PythonNodeStatus.SUCCESS, PythonNodeStatus.SUCCESS),
            expected_skip_reasons=(None, None),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sql_dep_completes_when_tracking_read_side_then_runs_ready_python_nodes(
    test_case: ReadSidePythonTrackerTestCase,
) -> None:
    graph: PythonNodeGraph = build_read_side_sql_task_asset_graph()
    reset_read_side_calls()
    tracker: ReadSidePythonExecutionTracker = ReadSidePythonExecutionTracker(
        python_graph=graph,
        selected_python_names=test_case.selected_names,
        scheduled_sql_names=test_case.scheduled_sql_names,
        runtime=PythonNodeRuntime(
            adapter=PythonNodeContextTestAdapter(),
            connection_config={},
            connection=object(),
            run_id="test_run",
            target="dev",
            vars={},
            is_reload=False,
            default_schema="default_schema",
        ),
    )

    tracker.dispatch_ready_python_nodes()
    sql_name: str
    for sql_name in test_case.completed_sql_names:
        tracker.record_sql_result(
            ModelExecutionResult(model_name=sql_name, status=ExecutionStatus.SUCCESS)
        )

    assert tuple(result.node_name for result in tracker.results) == test_case.expected_result_names
    assert tuple(result.status for result in tracker.results) == test_case.expected_statuses
    assert (
        tuple(result.skip_reason for result in tracker.results) == test_case.expected_skip_reasons
    )
    assert read_side_calls() == test_case.expected_call_order


@pytest.mark.parametrize(
    "test_case",
    [
        ReadSidePythonTrackerTestCase(
            description="finalizes sql blocked Python nodes as skipped",
            selected_names=frozenset({"profile_stg_orders", "export_stg_profile"}),
            completed_sql_names=(),
            failed_sql_names=("stg_orders",),
            expected_result_names=("export_stg_profile", "profile_stg_orders"),
            expected_call_order=(),
            expected_statuses=(PythonNodeStatus.SKIPPED, PythonNodeStatus.SKIPPED),
            expected_skip_reasons=(
                "Upstream Python node did not complete: profile_stg_orders",
                "Upstream SQL resource did not succeed: stg_orders",
            ),
        ),
        ReadSidePythonTrackerTestCase(
            description="finalizes Python nodes whose scheduled SQL never reported as skipped",
            selected_names=frozenset({"profile_stg_orders", "export_stg_profile"}),
            completed_sql_names=(),
            expected_result_names=("export_stg_profile", "profile_stg_orders"),
            expected_call_order=(),
            expected_statuses=(PythonNodeStatus.SKIPPED, PythonNodeStatus.SKIPPED),
            expected_skip_reasons=(
                "Upstream Python node did not complete: profile_stg_orders",
                "Upstream SQL resource did not complete: stg_orders",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sql_dep_fails_when_finalizing_read_side_then_skips_unrun_python_nodes(
    test_case: ReadSidePythonTrackerTestCase,
) -> None:
    graph: PythonNodeGraph = build_read_side_sql_task_asset_graph()
    reset_read_side_calls()
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)
    tracker: ReadSidePythonExecutionTracker = ReadSidePythonExecutionTracker(
        python_graph=graph,
        selected_python_names=test_case.selected_names,
        scheduled_sql_names=test_case.scheduled_sql_names,
        runtime=PythonNodeRuntime(
            adapter=PythonNodeContextTestAdapter(),
            connection_config={},
            connection=object(),
            run_id="test_run",
            target="dev",
            vars={},
            is_reload=False,
            default_schema="default_schema",
        ),
    )

    sql_name: str
    for sql_name in test_case.failed_sql_names:
        tracker.record_sql_result(
            ModelExecutionResult(model_name=sql_name, status=ExecutionStatus.FAILED)
        )
    with invocation_scope("inv-read-side-skips"), dispatcher_scope(dispatcher):
        tracker.finalize_unrun_python_nodes()

    assert tuple(result.node_name for result in tracker.results) == test_case.expected_result_names
    assert tuple(result.status for result in tracker.results) == test_case.expected_statuses
    assert (
        tuple(result.skip_reason for result in tracker.results) == test_case.expected_skip_reasons
    )
    assert read_side_calls() == test_case.expected_call_order
    assert tuple(event.event_type for event in events) == (
        "resource_attempt_started",
        "resource_attempt_skipped",
        "resource_attempt_started",
        "resource_attempt_skipped",
    )
    assert tuple(event.resource_id for event in events) == (
        "asset:export_stg_profile",
        "asset:export_stg_profile",
        "asset:profile_stg_orders",
        "asset:profile_stg_orders",
    )
    assert len({event.resource_attempt_id for event in events}) == 2
    assert all(event.run_id == "test_run" for event in events)
