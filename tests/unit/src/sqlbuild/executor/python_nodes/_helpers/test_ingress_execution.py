"""Tests for Python ingress loader lifecycle execution."""

from __future__ import annotations

from dataclasses import replace
from pathlib import Path

import pytest

from sqlbuild._native import DiscoveredPythonNode, PythonNodeGraph, SourceEntry
from sqlbuild.compiler.python_nodes.types import PythonNodeKind, PythonNodeStatus, SkipMode
from sqlbuild.executor.load.main._resource_kind import load_resource_kind
from sqlbuild.executor.python_nodes._helpers.ingress_execution import (
    _record_scheduler_skips,
    execute_ingress_python_loader_nodes,
)
from sqlbuild.executor.python_nodes.classes.ingress_results import IngressResultAccumulator
from sqlbuild.executor.python_nodes.models import (
    IngressCallbacks,
    PythonIngressLoaderExecutorResult,
    PythonNodeExecutionResult,
    PythonNodeRuntime,
)
from sqlbuild.executor.scheduling.models import LifecycleNodeResult, LifecycleSchedulerResult
from sqlbuild.executor.scheduling.types import LifecycleNodeStatus
from sqlbuild.observability import (
    EventDispatcher,
    LifecycleEvent,
    dispatcher_scope,
    invocation_scope,
)
from tests.unit.src.sqlbuild.executor.python_nodes._helpers._test_types import (
    IngressSchedulerSkipLifecycleTestCase,
    PythonIngressLoaderExecutorTestCase,
)
from tests.unit.src.sqlbuild.executor.python_nodes._helpers.helpers import (
    PythonNodeContextTestAdapter,
    PythonNodeContextTestResultStore,
    build_ingress_task_asset_graph,
    build_ingress_task_loader_graph,
    ingress_calls,
    ingress_loader_function,
    ingress_source_map,
    python_operation_events,
    reset_ingress_calls,
    scheduler_bypassed_task,
)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonIngressLoaderExecutorTestCase(
            description="runs task before dependent loader",
            selected_names=frozenset({"prepare_ingress_orders", "load_ingress_orders"}),
            expected_python_names=("prepare_ingress_orders",),
            expected_load_names=("raw_orders",),
            expected_python_statuses=(PythonNodeStatus.SUCCESS,),
            expected_load_statuses=("success",),
            expected_call_order=("prepare_ingress_orders", "load_ingress_orders"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_ingress_task_to_loader_when_executing_then_runs_in_lifecycle_order(
    test_case: PythonIngressLoaderExecutorTestCase,
) -> None:
    graph: PythonNodeGraph = build_ingress_task_loader_graph()
    reset_ingress_calls()
    events: list[LifecycleEvent] = []
    callback_order: list[str] = []
    dispatcher: EventDispatcher = EventDispatcher()

    def record_event(event: LifecycleEvent) -> None:
        events.append(event)
        callback_order.append(f"event:{event.event_type}:{event.payload.get('resource_name', '')}")

    dispatcher.subscribe_lifecycle(subscriber=record_event, accepts_opaque=False)

    with invocation_scope("ingress-invocation"), dispatcher_scope(dispatcher):
        result: PythonIngressLoaderExecutorResult = execute_ingress_python_loader_nodes(
            python_graph=graph,
            selected_python_names=test_case.selected_names,
            loader_functions=(ingress_loader_function(),),
            source_map=ingress_source_map(),
            runtime=PythonNodeRuntime(
                adapter=PythonNodeContextTestAdapter(),
                connection_config={},
                connection=object(),
                run_id="test_run",
                target="dev",
                vars={},
                is_reload=False,
            ),
            callbacks=IngressCallbacks(
                on_node_start=lambda name, resource_kind: callback_order.append(f"start:{name}"),
                on_node_complete=lambda completed: callback_order.append(
                    "complete:"
                    f"{completed.__dict__.get('source_name') or completed.__dict__.get('node_name')}"
                ),
            ),
        )

    assert tuple(node_result.node_name for node_result in result.python_results) == (
        test_case.expected_python_names
    )
    assert tuple(load_result.source_name for load_result in result.load_results) == (
        test_case.expected_load_names
    )
    assert tuple(node_result.status for node_result in result.python_results) == (
        test_case.expected_python_statuses
    )
    assert tuple(load_result.status.value for load_result in result.load_results) == (
        test_case.expected_load_statuses
    )
    assert ingress_calls() == test_case.expected_call_order
    assert result.python_results[0].payload == {"prepared": True}
    loader_events: tuple[LifecycleEvent, ...] = tuple(
        filter(lambda event: event.payload.get("resource_name") == "raw_orders", events)
    )
    assert tuple(event.event_type for event in loader_events) == (
        "resource_attempt_started",
        "resource_attempt_completed",
    )
    assert tuple(event.run_id for event in loader_events) == ("test_run", "test_run")
    assert callback_order.index("event:resource_attempt_started:raw_orders") < (
        callback_order.index("start:raw_orders")
    )
    assert callback_order.index("start:raw_orders") < callback_order.index(
        "event:resource_attempt_completed:raw_orders"
    )
    assert callback_order.index("event:resource_attempt_completed:raw_orders") < (
        callback_order.index("complete:raw_orders")
    )
    assert callback_order.index("event:resource_attempt_completed:prepare_ingress_orders") < (
        callback_order.index("complete:prepare_ingress_orders")
    )
    assert callback_order.count("complete:prepare_ingress_orders") == 1
    assert callback_order.count("complete:raw_orders") == 1


@pytest.mark.parametrize(
    "test_case",
    (
        PythonIngressLoaderExecutorTestCase(
            description="ingress task and asset complete once after canonical terminals",
            selected_names=frozenset({"prepare_ingress_orders", "publish_ingress_orders"}),
            expected_python_names=("prepare_ingress_orders", "publish_ingress_orders"),
            expected_load_names=(),
            expected_python_statuses=(PythonNodeStatus.SUCCESS, PythonNodeStatus.SUCCESS),
            expected_load_statuses=(),
            expected_call_order=("prepare_ingress_orders", "publish_ingress_orders"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_ingress_task_and_asset_when_executing_then_each_completion_follows_own_terminal_once(
    test_case: PythonIngressLoaderExecutorTestCase,
) -> None:
    graph: PythonNodeGraph = build_ingress_task_asset_graph()
    reset_ingress_calls()
    callback_order: list[str] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(
        subscriber=lambda event: callback_order.append(
            f"event:{event.event_type}:{event.payload.get('resource_name', '')}"
        ),
        accepts_opaque=False,
    )

    with invocation_scope("ingress-asset-invocation"), dispatcher_scope(dispatcher):
        result: PythonIngressLoaderExecutorResult = execute_ingress_python_loader_nodes(
            python_graph=graph,
            selected_python_names=test_case.selected_names,
            loader_functions=(),
            source_map={},
            runtime=PythonNodeRuntime(
                adapter=PythonNodeContextTestAdapter(),
                connection_config={},
                connection=object(),
                run_id="test_run",
                target="dev",
                vars={},
                is_reload=False,
            ),
            callbacks=IngressCallbacks(
                on_node_complete=lambda completed: callback_order.append(
                    f"complete:{completed.node_name}"
                )
            ),
        )

    assert (
        tuple(item.node_name for item in result.python_results) == test_case.expected_python_names
    )
    assert (
        tuple(item.status for item in result.python_results) == test_case.expected_python_statuses
    )
    assert ingress_calls() == test_case.expected_call_order
    for node_name in test_case.expected_python_names:
        assert callback_order.count(f"complete:{node_name}") == 1
        assert callback_order.index(f"event:resource_attempt_completed:{node_name}") < (
            callback_order.index(f"complete:{node_name}")
        )


@pytest.mark.parametrize(
    "test_case",
    (
        IngressSchedulerSkipLifecycleTestCase(
            description="scheduler bypasses downstream loader and task exactly once",
            expected_resource_ids=("source:raw_orders", "asset:bypassed_task"),
            expected_event_types=(
                "resource_attempt_started",
                "resource_attempt_skipped",
                "resource_attempt_started",
                "resource_attempt_skipped",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_pre_recorded_hard_skip_when_scheduler_bypasses_downstream_then_only_new_results_get_attempts(
    test_case: IngressSchedulerSkipLifecycleTestCase,
) -> None:
    base_graph: PythonNodeGraph = build_ingress_task_loader_graph()
    bypassed_node: DiscoveredPythonNode = DiscoveredPythonNode(
        kind=PythonNodeKind.ASSET,
        file_path=Path("/project/tasks/bypassed.py"),
        relative_path=Path("python/nodes/tasks/bypassed.py"),
        name="bypassed_task",
        function=scheduler_bypassed_task,
    )
    graph: PythonNodeGraph = replace(
        base_graph,
        nodes=(*base_graph.nodes, bypassed_node),
        nodes_by_name={**base_graph.nodes_by_name, bypassed_node.name: bypassed_node},
    )
    accumulator: IngressResultAccumulator = IngressResultAccumulator()
    accumulator.record_python_result(
        name="prepare_ingress_orders",
        result=PythonNodeExecutionResult(
            node_name="prepare_ingress_orders",
            kind=PythonNodeKind.ASSET,
            status=PythonNodeStatus.SKIPPED,
            skip_mode=SkipMode.HARD,
            skip_reason="upstream hard skip",
        ),
    )
    scheduler_result: LifecycleSchedulerResult = LifecycleSchedulerResult(
        results=(
            LifecycleNodeResult(
                name="prepare_ingress_orders",
                kind="asset",
                status=LifecycleNodeStatus.SKIPPED,
                skip_mode=SkipMode.HARD,
            ),
            LifecycleNodeResult(
                name="load_ingress_orders",
                kind="loader",
                status=LifecycleNodeStatus.SKIPPED,
                skip_mode=SkipMode.HARD,
            ),
            LifecycleNodeResult(
                name="bypassed_task",
                kind="asset",
                status=LifecycleNodeStatus.SKIPPED,
                skip_mode=SkipMode.HARD,
            ),
        )
    )
    source_map: dict[str, SourceEntry] = ingress_source_map()
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)
    result_store: PythonNodeContextTestResultStore = PythonNodeContextTestResultStore({})

    with invocation_scope("inv-scheduler-skips"), dispatcher_scope(dispatcher):
        _record_scheduler_skips(
            scheduler_result=scheduler_result,
            python_graph=graph,
            source_by_loader_name={"load_ingress_orders": source_map["raw_orders"]},
            results=accumulator,
            result_store=result_store,
            run_id="run-scheduler-skips",
        )

    assert tuple(event.event_type for event in events) == test_case.expected_event_types
    assert tuple(events[index].resource_id for index in (0, 2)) == test_case.expected_resource_ids
    assert events[0].resource_attempt_id == events[1].resource_attempt_id
    assert events[2].resource_attempt_id == events[3].resource_attempt_id
    assert events[0].resource_attempt_id != events[2].resource_attempt_id
    assert tuple(event.payload.get("skip_code") for event in (events[1], events[3])) == (
        "scheduler",
        "scheduler",
    )
    assert tuple(event.payload.get("skip_mode") for event in (events[1], events[3])) == (
        "hard",
        "hard",
    )
    assert events[0].payload["resource_kind"] == load_resource_kind(source_map["raw_orders"]).value
    assert events[2].payload["resource_kind"] == "asset"
    assert python_operation_events(events) == ()
    assert tuple(accumulator.python_results_by_name) == (
        "prepare_ingress_orders",
        "bypassed_task",
    )
    assert tuple(accumulator.load_results_by_name) == ("load_ingress_orders",)
    assert len(result_store.written_records) == 1


@pytest.mark.parametrize(
    "test_case",
    (
        IngressSchedulerSkipLifecycleTestCase(
            description="scheduler-bypassed loader persistence failure",
            expected_resource_ids=("source:raw_orders",),
            expected_event_types=(
                "resource_attempt_started",
                "resource_attempt_failed",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_scheduler_bypassed_loader_when_persistence_fails_then_attempt_fails_not_skips(
    test_case: IngressSchedulerSkipLifecycleTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    graph: PythonNodeGraph = build_ingress_task_loader_graph()
    accumulator: IngressResultAccumulator = IngressResultAccumulator()
    accumulator.record_python_result(
        name="prepare_ingress_orders",
        result=PythonNodeExecutionResult(
            node_name="prepare_ingress_orders",
            kind=PythonNodeKind.ASSET,
            status=PythonNodeStatus.SKIPPED,
            skip_mode=SkipMode.HARD,
        ),
    )
    scheduler_result: LifecycleSchedulerResult = LifecycleSchedulerResult(
        results=(
            LifecycleNodeResult(
                name="load_ingress_orders",
                kind="loader",
                status=LifecycleNodeStatus.SKIPPED,
                skip_mode=SkipMode.HARD,
            ),
        )
    )
    source_map: dict[str, SourceEntry] = ingress_source_map()
    result_store: PythonNodeContextTestResultStore = PythonNodeContextTestResultStore({})
    monkeypatch.setattr(
        result_store,
        "write",
        lambda _record: (_ for _ in ()).throw(RuntimeError("skip persistence failed")),
    )
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with (
        invocation_scope("inv-scheduler-persist-failure"),
        dispatcher_scope(dispatcher),
        pytest.raises(RuntimeError, match="skip persistence failed"),
    ):
        _record_scheduler_skips(
            scheduler_result=scheduler_result,
            python_graph=graph,
            source_by_loader_name={"load_ingress_orders": source_map["raw_orders"]},
            results=accumulator,
            result_store=result_store,
            run_id="run-scheduler-persist-failure",
        )

    assert tuple(event.event_type for event in events) == test_case.expected_event_types
    assert tuple(event.resource_id for event in events) == test_case.expected_resource_ids * 2
    assert events[0].resource_attempt_id == events[1].resource_attempt_id
    assert python_operation_events(events) == ()
    assert tuple(accumulator.load_results_by_name) == ("load_ingress_orders",)
