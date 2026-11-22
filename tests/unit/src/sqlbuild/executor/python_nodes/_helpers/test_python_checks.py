"""Tests for Python check execution helpers."""

from __future__ import annotations

import json
from datetime import datetime
from pathlib import Path

import pytest

from sqlbuild._native import (
    CostContext,
    DiscoveredCheckFunction,
    ExecutionEventWriter,
    PythonNodeGraph,
    TerminalEventIndex,
    format_check_json,
    terminal_event_index_scope,
)
from sqlbuild.compiler.python_nodes.types import PythonNodeKind, PythonNodeStatus, SkipMode
from sqlbuild.cost.models import CostResourceContext
from sqlbuild.executor.node_results.models import NodeResultEnvelope
from sqlbuild.executor.python_nodes._helpers.python_checks import execute_python_check_nodes
from sqlbuild.executor.python_nodes.models import (
    CheckContext,
    PythonCheckCallbacks,
    PythonCheckExecutionResult,
    PythonCheckResult,
    PythonNodeExecutionResult,
    PythonNodeRunState,
    PythonNodeRuntime,
)
from sqlbuild.observability import (
    EventDispatcher,
    LifecycleEvent,
    dispatcher_scope,
    invocation_scope,
)
from sqlbuild.provider.classes.container import ProviderContainer
from sqlbuild.provider.classes.session import ProviderSession
from sqlbuild.python_nodes.types import PythonCheckSeverity
from tests.unit.src.sqlbuild.executor.python_nodes._helpers._test_types import (
    BlockedPythonCheckLifecycleTestCase,
    MalformedPythonOperationTestCase,
    PythonCheckExecutorTestCase,
    PythonCheckLifecycleTestCase,
    PythonPostCallLifecycleTestCase,
)
from tests.unit.src.sqlbuild.executor.python_nodes._helpers.helpers import (
    ExecutionSlackProvider,
    PythonNodeContextTestAdapter,
    PythonNodeContextTestResultStore,
    build_python_check_graph,
    context_provider_check,
    provider_check,
    python_check_function_for_case,
    python_operation_events,
)


@pytest.mark.parametrize(
    "test_case",
    (
        MalformedPythonOperationTestCase(
            description="check returns unsupported object",
            operation_name="python_check",
            expected_error_fragment="must return PythonCheckResult, True, or False",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_malformed_check_return_when_executing_then_operation_fails_once(
    test_case: MalformedPythonOperationTestCase,
) -> None:
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    def malformed_check(ctx: CheckContext) -> object:
        del ctx
        return object()

    check_function: DiscoveredCheckFunction = DiscoveredCheckFunction(
        file_path=Path(__file__),
        relative_path=Path(Path(__file__).name),
        name="malformed_check",
        function=malformed_check,
        depends_on=(),
    )
    with invocation_scope("inv-malformed-check"), dispatcher_scope(dispatcher):
        results: tuple[PythonCheckExecutionResult, ...] = execute_python_check_nodes(
            check_functions=(check_function,),
            python_graph=build_python_check_graph(check_function=check_function),
            upstream_python_results=(),
            upstream_load_results=(),
            run_state=PythonNodeRunState(),
            runtime=PythonNodeRuntime(
                adapter=PythonNodeContextTestAdapter(),
                connection_config={},
                connection=object(),
                run_id="run-malformed",
                target="dev",
                vars={},
                is_reload=False,
            ),
        )

    operation_events: tuple[LifecycleEvent, ...] = python_operation_events(events)
    resource_events: tuple[LifecycleEvent, ...] = tuple(
        filter(lambda event: event.event_type.startswith("resource_attempt_"), events)
    )
    assert results[0].failed
    assert test_case.expected_error_fragment in (results[0].error_message or "")
    assert tuple(event.event_type for event in operation_events) == (
        "operation_started",
        "operation_failed",
    )
    assert operation_events[0].payload["operation_name"] == test_case.operation_name
    assert tuple(event.run_id for event in resource_events) == (
        "run-malformed",
        "run-malformed",
    )


@pytest.mark.parametrize(
    "test_case",
    (
        PythonCheckLifecycleTestCase(
            description="error-severity returned failure",
            passed=False,
            severity=PythonCheckSeverity.ERROR,
            expected_event_types=(
                "resource_attempt_started",
                "operation_started",
                "operation_failed",
                "resource_attempt_failed",
            ),
        ),
        PythonCheckLifecycleTestCase(
            description="warning returned failure",
            passed=False,
            severity=PythonCheckSeverity.WARN,
            expected_event_types=(
                "resource_attempt_started",
                "operation_started",
                "operation_completed",
                "resource_attempt_completed",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_returned_check_failure_when_executing_then_child_and_resource_match_severity(
    test_case: PythonCheckLifecycleTestCase,
) -> None:
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    def returned_check(ctx: CheckContext) -> object:
        del ctx
        return PythonCheckResult(passed=test_case.passed, severity=test_case.severity)

    check_function: DiscoveredCheckFunction = DiscoveredCheckFunction(
        file_path=Path(__file__),
        relative_path=Path(Path(__file__).name),
        name="returned_check",
        function=returned_check,
        depends_on=(),
    )
    with invocation_scope("inv-returned-check"), dispatcher_scope(dispatcher):
        _ = execute_python_check_nodes(
            check_functions=(check_function,),
            python_graph=build_python_check_graph(check_function=check_function),
            upstream_python_results=(),
            upstream_load_results=(),
            run_state=PythonNodeRunState(),
            runtime=PythonNodeRuntime(
                adapter=PythonNodeContextTestAdapter(),
                connection_config={},
                connection=object(),
                run_id="run-returned-check",
                target="dev",
                vars={},
                is_reload=False,
            ),
        )

    assert tuple(event.event_type for event in events) == test_case.expected_event_types


@pytest.mark.parametrize(
    "test_case",
    (
        PythonPostCallLifecycleTestCase(
            description="passing check followed by persistence failure",
            expected_event_types=(
                "resource_attempt_started",
                "operation_started",
                "operation_completed",
                "resource_attempt_failed",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_passing_check_when_persistence_fails_then_resource_only_fails(
    test_case: PythonPostCallLifecycleTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)
    result_store: PythonNodeContextTestResultStore = PythonNodeContextTestResultStore({})
    monkeypatch.setattr(
        result_store,
        "write",
        lambda record: (_ for _ in ()).throw(RuntimeError("check persistence failed")),
    )

    def passing_check(ctx: CheckContext) -> bool:
        del ctx
        return True

    check_function: DiscoveredCheckFunction = DiscoveredCheckFunction(
        file_path=Path(__file__),
        relative_path=Path(Path(__file__).name),
        name="persisted_check",
        function=passing_check,
        depends_on=(),
    )
    with (
        invocation_scope("inv-check-persist-failure"),
        dispatcher_scope(dispatcher),
        pytest.raises(RuntimeError, match="check persistence failed"),
    ):
        _ = execute_python_check_nodes(
            check_functions=(check_function,),
            python_graph=build_python_check_graph(check_function=check_function),
            upstream_python_results=(),
            upstream_load_results=(),
            run_state=PythonNodeRunState(),
            runtime=PythonNodeRuntime(
                adapter=PythonNodeContextTestAdapter(),
                connection_config={},
                connection=object(),
                run_id="run-check-persist-failure",
                target="dev",
                vars={},
                is_reload=False,
                result_store=result_store,
            ),
        )

    assert tuple(event.event_type for event in events) == test_case.expected_event_types


@pytest.mark.parametrize(
    "test_case",
    (
        PythonCheckExecutorTestCase(
            description="executes passing check with upstream metadata",
            expected_passed=True,
            expected_severity=PythonCheckSeverity.ERROR,
            expected_message="passed",
            upstream_skip_mode=None,
        ),
        PythonCheckExecutorTestCase(
            description="preserves explicit warning result",
            expected_passed=False,
            expected_severity=PythonCheckSeverity.WARN,
            expected_message="warned",
            upstream_skip_mode=None,
        ),
        PythonCheckExecutorTestCase(
            description="normalizes false result as error failure",
            expected_passed=False,
            expected_severity=PythonCheckSeverity.ERROR,
            expected_message=None,
            upstream_skip_mode=None,
        ),
        PythonCheckExecutorTestCase(
            description="normalizes check exception as error failure",
            expected_passed=False,
            expected_severity=PythonCheckSeverity.ERROR,
            expected_message=None,
            upstream_skip_mode=None,
            expected_error_fragment="check exploded",
        ),
        PythonCheckExecutorTestCase(
            description="fails when upstream failed",
            expected_passed=False,
            expected_severity=PythonCheckSeverity.ERROR,
            expected_message=None,
            upstream_skip_mode=None,
            expected_error_fragment="Upstream Python node failed: upstream_task",
            upstream_status=PythonNodeStatus.FAILED,
        ),
        PythonCheckExecutorTestCase(
            description="warns when upstream skipped",
            expected_passed=False,
            expected_severity=PythonCheckSeverity.WARN,
            expected_message="Upstream Python node skipped: upstream_task",
            upstream_status=PythonNodeStatus.SKIPPED,
            upstream_skip_mode=SkipMode.HARD,
            upstream_skip_reason="not needed",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_python_check_when_executing_then_returns_expected_result(
    test_case: PythonCheckExecutorTestCase,
) -> None:
    check_function: DiscoveredCheckFunction = python_check_function_for_case(test_case.description)
    graph: PythonNodeGraph = build_python_check_graph(check_function=check_function)
    run_state: PythonNodeRunState = PythonNodeRunState()
    upstream_result: PythonNodeExecutionResult = PythonNodeExecutionResult(
        node_name="upstream_task",
        kind=PythonNodeKind.ASSET,
        status=test_case.upstream_status,
        payload={"rows": 3},
        metadata={"rows": 3},
        skip_mode=test_case.upstream_skip_mode,
        skip_reason=test_case.upstream_skip_reason,
    )
    run_state.record_result(result=upstream_result)
    result_store: PythonNodeContextTestResultStore = PythonNodeContextTestResultStore(
        {
            (PythonNodeKind.ASSET.value, "upstream_task"): (
                NodeResultEnvelope(
                    node_type=PythonNodeKind.ASSET.value,
                    node_name="upstream_task",
                    run_id="run_1",
                    status=test_case.upstream_status.value,
                    payload={"rows": 3},
                    metadata={"rows": 3},
                    error_message=None,
                    materialized=None,
                    ts=datetime(2026, 1, 1),
                ),
            )
        }
    )

    results: tuple[PythonCheckExecutionResult, ...] = execute_python_check_nodes(
        check_functions=(check_function,),
        python_graph=graph,
        upstream_python_results=(upstream_result,),
        upstream_load_results=(),
        run_state=run_state,
        runtime=PythonNodeRuntime(
            adapter=PythonNodeContextTestAdapter(),
            connection_config={},
            connection=object(),
            run_id="run_1",
            target="dev",
            vars={},
            is_reload=False,
            result_store=result_store,
        ),
    )

    assert len(results) == 1
    result: PythonCheckExecutionResult = results[0]
    assert result.passed == test_case.expected_passed
    assert result.severity == test_case.expected_severity
    assert result.message == test_case.expected_message
    expected_error_fragment: str = test_case.expected_error_fragment or ""
    assert expected_error_fragment in (result.error_message or "")


@pytest.mark.parametrize(
    "test_case",
    (
        BlockedPythonCheckLifecycleTestCase(
            description="failed upstream produces failed blocked check terminal",
            upstream_status=PythonNodeStatus.FAILED,
            upstream_skip_mode=None,
            expected_terminal="resource_attempt_failed",
            expected_check_status="fail",
            expected_summary={
                "pass_count": 0,
                "warn_count": 0,
                "fail_count": 1,
                "total_count": 1,
            },
        ),
        BlockedPythonCheckLifecycleTestCase(
            description="skipped upstream produces completed warning check terminal",
            upstream_status=PythonNodeStatus.SKIPPED,
            upstream_skip_mode=SkipMode.HARD,
            expected_terminal="resource_attempt_completed",
            expected_check_status="warn",
            expected_summary={
                "pass_count": 0,
                "warn_count": 1,
                "fail_count": 0,
                "total_count": 1,
            },
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_blocked_python_check_when_persisted_then_terminal_precedes_integration_outputs(
    test_case: BlockedPythonCheckLifecycleTestCase,
    tmp_path: Path,
) -> None:
    check_function: DiscoveredCheckFunction = python_check_function_for_case(test_case.description)
    graph: PythonNodeGraph = build_python_check_graph(check_function=check_function)
    upstream_result: PythonNodeExecutionResult = PythonNodeExecutionResult(
        node_name="upstream_task",
        kind=PythonNodeKind.ASSET,
        status=test_case.upstream_status,
        skip_mode=test_case.upstream_skip_mode,
        skip_reason="blocked",
    )
    events: list[LifecycleEvent] = []
    event_path: Path = tmp_path / "integration-results.jsonl"
    projector: TerminalEventIndex = TerminalEventIndex()
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)
    dispatcher.subscribe_lifecycle(subscriber=projector.consume, accepts_opaque=False)

    with (
        invocation_scope("blocked-check-invocation"),
        dispatcher_scope(dispatcher),
        terminal_event_index_scope(projector),
    ):
        writer: ExecutionEventWriter = ExecutionEventWriter(path=event_path)
        results: tuple[PythonCheckExecutionResult, ...] = execute_python_check_nodes(
            check_functions=(check_function,),
            python_graph=graph,
            upstream_python_results=(upstream_result,),
            upstream_load_results=(),
            run_state=PythonNodeRunState(),
            runtime=PythonNodeRuntime(
                adapter=PythonNodeContextTestAdapter(),
                connection_config={},
                connection=object(),
                run_id="blocked-check-run",
                target="dev",
                vars={},
                is_reload=False,
            ),
            callbacks=PythonCheckCallbacks(
                on_check_complete=lambda result: writer.write_build_result(
                    result=result, plan=None, command="check"
                )
            ),
        )
        writer.close()
        final_payload: dict[str, object] = json.loads(format_check_json(results=results))

    integration_payload: dict[str, object] = json.loads(event_path.read_text(encoding="utf-8"))
    projected_check: dict[str, object] = integration_payload["checks"][0]  # type: ignore[index,assignment]
    assert len(events) == 2
    assert tuple(event.run_id for event in events) == (
        "blocked-check-run",
        "blocked-check-run",
    )
    assert events[0].event_type == "resource_attempt_started"
    assert events[1].event_type == test_case.expected_terminal
    assert projected_check["status"] == test_case.expected_check_status
    assert final_payload["summary"] == test_case.expected_summary
    final_checks: list[dict[str, object]] = final_payload["checks"]  # type: ignore[assignment]
    assert len(final_checks) == 1
    assert final_checks[0]["name"] == projected_check["name"]
    assert final_checks[0]["status"] == test_case.expected_check_status


@pytest.mark.parametrize(
    "test_case",
    [
        PythonCheckExecutorTestCase(
            description="injects provider into check execution",
            expected_passed=True,
            expected_severity=PythonCheckSeverity.ERROR,
            expected_message=None,
            upstream_skip_mode=None,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_parameter_when_executing_python_check_then_provider_is_injected(
    test_case: PythonCheckExecutorTestCase,
) -> None:
    check_function: DiscoveredCheckFunction = DiscoveredCheckFunction(
        file_path=Path(__file__),
        relative_path=Path(Path(__file__).name),
        name="provider_check",
        function=provider_check,
        depends_on=(),
    )
    graph: PythonNodeGraph = build_python_check_graph(check_function=check_function)
    providers: ProviderContainer = ProviderSession(
        {"slack_provider": ExecutionSlackProvider(label="slack")}
    ).providers

    results: tuple[PythonCheckExecutionResult, ...] = execute_python_check_nodes(
        check_functions=(check_function,),
        python_graph=graph,
        upstream_python_results=(),
        upstream_load_results=(),
        run_state=PythonNodeRunState(),
        runtime=PythonNodeRuntime(
            adapter=PythonNodeContextTestAdapter(),
            connection_config={},
            connection=object(),
            run_id="run_1",
            target="dev",
            vars={},
            is_reload=False,
            providers=providers,
        ),
    )

    assert len(results) == 1
    result: PythonCheckExecutionResult = results[0]
    assert result.passed == test_case.expected_passed
    assert result.severity == test_case.expected_severity
    assert result.message == test_case.expected_message


@pytest.mark.parametrize(
    "test_case",
    [
        PythonCheckExecutorTestCase(
            description="exposes providers on check context",
            expected_passed=True,
            expected_severity=PythonCheckSeverity.ERROR,
            expected_message=None,
            upstream_skip_mode=None,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_container_when_executing_python_check_then_context_exposes_providers(
    test_case: PythonCheckExecutorTestCase,
) -> None:
    check_function: DiscoveredCheckFunction = DiscoveredCheckFunction(
        file_path=Path(__file__),
        relative_path=Path(Path(__file__).name),
        name="context_provider_check",
        function=context_provider_check,
        depends_on=(),
    )
    graph: PythonNodeGraph = build_python_check_graph(check_function=check_function)
    providers: ProviderContainer = ProviderSession(
        {"slack_provider": ExecutionSlackProvider(label="slack")}
    ).providers

    results: tuple[PythonCheckExecutionResult, ...] = execute_python_check_nodes(
        check_functions=(check_function,),
        python_graph=graph,
        upstream_python_results=(),
        upstream_load_results=(),
        run_state=PythonNodeRunState(),
        runtime=PythonNodeRuntime(
            adapter=PythonNodeContextTestAdapter(),
            connection_config={},
            connection=object(),
            run_id="run_1",
            target="dev",
            vars={},
            is_reload=False,
            providers=providers,
        ),
    )

    assert len(results) == 1
    result: PythonCheckExecutionResult = results[0]
    assert result.passed == test_case.expected_passed
    assert result.severity == test_case.expected_severity
    assert result.message == test_case.expected_message


@pytest.mark.parametrize(
    "test_case",
    [
        PythonCheckExecutorTestCase(
            description="normalizes missing provider container as check failure",
            expected_passed=False,
            expected_severity=PythonCheckSeverity.ERROR,
            expected_message=None,
            upstream_skip_mode=None,
            expected_error_fragment=(
                "Provider parameter 'slack_provider' requires provider 'slack_provider', "
                "but no provider container is available"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_provider_container_when_executing_python_check_then_failure_is_recorded(
    test_case: PythonCheckExecutorTestCase,
) -> None:
    check_function: DiscoveredCheckFunction = DiscoveredCheckFunction(
        file_path=Path(__file__),
        relative_path=Path(Path(__file__).name),
        name="provider_check",
        function=provider_check,
        depends_on=(),
    )
    graph: PythonNodeGraph = build_python_check_graph(check_function=check_function)

    results: tuple[PythonCheckExecutionResult, ...] = execute_python_check_nodes(
        check_functions=(check_function,),
        python_graph=graph,
        upstream_python_results=(),
        upstream_load_results=(),
        run_state=PythonNodeRunState(),
        runtime=PythonNodeRuntime(
            adapter=PythonNodeContextTestAdapter(),
            connection_config={},
            connection=object(),
            run_id="run_1",
            target="dev",
            vars={},
            is_reload=False,
        ),
    )

    assert len(results) == 1
    result: PythonCheckExecutionResult = results[0]
    assert result.passed == test_case.expected_passed
    assert result.severity == test_case.expected_severity
    assert result.message == test_case.expected_message
    assert test_case.expected_error_fragment is not None
    assert test_case.expected_error_fragment in (result.error_message or "")


@pytest.mark.parametrize(
    "test_case",
    [
        PythonCheckExecutorTestCase(
            description="attributes post build check to exact check identity and phase",
            expected_passed=True,
            expected_severity=PythonCheckSeverity.ERROR,
            expected_message=None,
            upstream_skip_mode=None,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_post_build_check_when_executing_then_cost_context_uses_check_identity(
    test_case: PythonCheckExecutorTestCase,
) -> None:
    observed_cost_contexts: list[CostResourceContext | None] = []

    def attributed_check(_ctx: CheckContext) -> bool:
        observed_cost_contexts.append(CostContext.current())
        return True

    check_function: DiscoveredCheckFunction = DiscoveredCheckFunction(
        file_path=Path(__file__),
        relative_path=Path(Path(__file__).name),
        name="orders_are_valid",
        function=attributed_check,
        depends_on=(),
    )
    with CostContext.scope(
        run_id="run_1",
        resource_type="run",
        resource_name="dev",
        phase="post_build_checks",
    ):
        results: tuple[PythonCheckExecutionResult, ...] = execute_python_check_nodes(
            check_functions=(check_function,),
            python_graph=build_python_check_graph(check_function=check_function),
            upstream_python_results=(),
            upstream_load_results=(),
            run_state=PythonNodeRunState(),
            runtime=PythonNodeRuntime(
                adapter=PythonNodeContextTestAdapter(),
                connection_config={},
                connection=object(),
                run_id="run_1",
                target="dev",
                vars={},
                is_reload=False,
            ),
        )

    cost_context: CostResourceContext | None = observed_cost_contexts[0]
    assert results[0].passed is test_case.expected_passed
    assert cost_context is not None
    assert cost_context.resource_type == "check"
    assert cost_context.resource_name == "orders_are_valid"
    assert cost_context.phase == "post_build_checks"
    assert cost_context.attempt == 1
