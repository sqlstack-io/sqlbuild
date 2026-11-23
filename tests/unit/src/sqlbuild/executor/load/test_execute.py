"""Tests for source loader execution contracts."""

from __future__ import annotations

from collections.abc import Iterator
from datetime import UTC, datetime
from pathlib import Path
from typing import cast

import pytest

from sqlbuild._native import (
    CostContext,
    DiscoveredLoaderFunction,
    SourceEntry,
    SourceWriteStrategy,
)
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.cost.models import CostResourceContext
from sqlbuild.executor.load.main._execute import execute_source_load
from sqlbuild.executor.load.models import (
    LoaderContext,
    LoadExecutionResult,
    LoadRuntimeParams,
)
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.observability import (
    EventDispatcher,
    LifecycleEvent,
    dispatcher_scope,
    invocation_scope,
)
from sqlbuild.provider.classes.container import ProviderContainer
from sqlbuild.provider.classes.session import ProviderSession
from sqlbuild.runtime.observability.classes.statement_lifecycle import StatementLifecycle
from tests.unit.src.sqlbuild.executor.load._test_types import (
    LoaderOperationLifecycleTestCase,
    SourceLoadExecutionContextTestCase,
    SourceLoadNoneReturnTestCase,
)
from tests.unit.src.sqlbuild.executor.load.helpers import (
    LoaderContextTestAdapter,
    operation_events,
    operation_statement_events,
    statement_events,
)
from tests.unit.src.sqlbuild.executor.python_nodes._helpers.helpers import ExecutionSlackProvider


@pytest.mark.parametrize(
    "test_case",
    (
        LoaderOperationLifecycleTestCase(
            description="generator completes after framework staging consumption",
            expected_status=ExecutionStatus.SUCCESS,
            expected_event_types=("operation_started", "operation_completed"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_loader_generator_when_staging_consumes_it_then_terminal_follows_exhaustion(
    test_case: LoaderOperationLifecycleTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)
    monkeypatch.setattr(
        "sqlbuild.executor.load.main._execute._apply_source_write_strategy",
        lambda **kwargs: kwargs["adapter"],
    )

    def raw_orders_loader(ctx: LoaderContext) -> Iterator[dict[str, object]]:
        del ctx
        assert tuple(event.event_type for event in operation_events(events)) == (
            "operation_started",
        )
        with StatementLifecycle(adapter="duckdb", sql="SELECT 1", intent="loader_test"):
            pass
        yield {"order_id": 1}
        assert tuple(event.event_type for event in operation_events(events)) == (
            "operation_started",
        )

    with invocation_scope("inv-loader-generator"), dispatcher_scope(dispatcher):
        result: LoadExecutionResult = execute_source_load(
            source_entry=SourceEntry(
                name="raw_orders",
                table="orders",
                loader="raw_orders_loader",
                write_strategy=SourceWriteStrategy.TABLE,
            ),
            loader_function=DiscoveredLoaderFunction(
                file_path=Path("python/nodes/loaders/raw.py"),
                relative_path=Path("python/nodes/loaders/raw.py"),
                name="raw_orders_loader",
                function=raw_orders_loader,
            ),
            adapter=LoaderContextTestAdapter(),
            connection_config={},
            connection=object(),
            statement_recorder=StatementRecorder(),
            runtime=LoadRuntimeParams(run_id="run-loader", target="dev", vars={}, is_reload=False),
        )

    observed_operations: tuple[LifecycleEvent, ...] = operation_events(events)
    observed_statements: tuple[LifecycleEvent, ...] = statement_events(events)
    assert result.status == test_case.expected_status, result.error_message
    assert (
        tuple(event.event_type for event in observed_operations) == test_case.expected_event_types
    )
    assert observed_operations[0].payload["operation_name"] == "managed_source_load"
    correlated_statements: tuple[LifecycleEvent, ...] = operation_statement_events(
        events=observed_statements,
        operation_id=observed_operations[0].operation_id,
    )
    assert correlated_statements
    assert {event.operation_id for event in correlated_statements} == {
        observed_operations[0].operation_id
    }


@pytest.mark.parametrize(
    "test_case",
    (
        LoaderOperationLifecycleTestCase(
            description="generator failure during framework staging consumption",
            expected_status=ExecutionStatus.FAILED,
            expected_event_types=("operation_started", "operation_failed"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_loader_generator_failure_when_staging_consumes_it_then_failed_terminal_follows(
    test_case: LoaderOperationLifecycleTestCase,
) -> None:
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    def failing_loader(ctx: LoaderContext) -> Iterator[dict[str, object]]:
        del ctx
        assert operation_events(events)[-1].event_type == "operation_started"
        yield {"order_id": 1}
        assert operation_events(events)[-1].event_type == "operation_started"
        raise RuntimeError("private generator failure")

    with invocation_scope("inv-loader-generator-failure"), dispatcher_scope(dispatcher):
        result: LoadExecutionResult = execute_source_load(
            source_entry=SourceEntry(
                name="raw_orders",
                table="orders",
                loader="raw_orders_loader",
                write_strategy=SourceWriteStrategy.TABLE,
            ),
            loader_function=DiscoveredLoaderFunction(
                file_path=Path("python/nodes/loaders/raw.py"),
                relative_path=Path("python/nodes/loaders/raw.py"),
                name="raw_orders_loader",
                function=failing_loader,
            ),
            adapter=LoaderContextTestAdapter(),
            connection_config={},
            connection=object(),
            statement_recorder=StatementRecorder(),
            runtime=LoadRuntimeParams(run_id="run-loader", target="dev", vars={}, is_reload=False),
        )

    observed_operations: tuple[LifecycleEvent, ...] = operation_events(events)
    assert result.status == test_case.expected_status
    assert (
        tuple(event.event_type for event in observed_operations) == test_case.expected_event_types
    )
    assert observed_operations[-1].payload["error_type"] == "RuntimeError"
    assert "private generator failure" not in repr(observed_operations)


@pytest.mark.parametrize(
    "test_case",
    [
        SourceLoadExecutionContextTestCase(
            description="passes runtime metadata and cursor fields to loader context",
            source_name="raw_orders",
            loader_name="raw_orders_loader",
            target_table="orders",
            database="analytics",
            schema="raw",
            run_id="run-123",
            target="dev",
            vars={"batch": 7},
            is_reload=True,
            start_cursor_ts=datetime(2026, 5, 1, tzinfo=UTC),
            end_cursor_ts=datetime(2026, 5, 2, tzinfo=UTC),
            start_cursor_int=10,
            end_cursor_int=20,
            expected_target="analytics.raw.orders",
            expected_current_cursor_value="max-value",
            expected_status=ExecutionStatus.SUCCESS,
            expected_rows_loaded=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_loader_when_executing_then_context_includes_runtime_metadata(
    test_case: SourceLoadExecutionContextTestCase,
) -> None:
    observed_contexts: list[LoaderContext] = []
    observed_cost_contexts: list[CostResourceContext | None] = []

    def raw_orders_loader(ctx: LoaderContext) -> None:
        observed_contexts.append(ctx)
        observed_cost_contexts.append(CostContext.current())
        return None

    with CostContext.scope(
        run_id=test_case.run_id,
        resource_type="run",
        resource_name=test_case.target or "default",
        phase="build",
    ):
        result: LoadExecutionResult = execute_source_load(
            source_entry=SourceEntry(
                name=test_case.source_name,
                database=test_case.database,
                schema=test_case.schema,
                table=test_case.target_table,
                loader=test_case.loader_name,
                cursor_column="updated_at",
            ),
            loader_function=DiscoveredLoaderFunction(
                file_path=Path("python/nodes/loaders/raw.py"),
                relative_path=Path("python/nodes/loaders/raw.py"),
                name=test_case.loader_name,
                function=raw_orders_loader,
            ),
            adapter=LoaderContextTestAdapter(),
            connection_config={"database": "loader.duckdb"},
            connection=object(),
            statement_recorder=StatementRecorder(),
            runtime=LoadRuntimeParams(
                run_id=test_case.run_id,
                target=test_case.target,
                vars=test_case.vars,
                is_reload=test_case.is_reload,
                start_cursor_ts=test_case.start_cursor_ts,
                end_cursor_ts=test_case.end_cursor_ts,
                start_cursor_int=test_case.start_cursor_int,
                end_cursor_int=test_case.end_cursor_int,
            ),
        )

    context: LoaderContext = observed_contexts[0]
    assert result.status == test_case.expected_status
    assert result.rows_loaded == test_case.expected_rows_loaded
    assert context.destination == test_case.expected_target
    assert context.destination_database == test_case.database
    assert context.destination_schema == test_case.schema
    assert context.destination_name == test_case.target_table
    assert context.run_id == test_case.run_id
    assert context.target == test_case.target
    assert context.vars == test_case.vars
    assert context.is_reload is test_case.is_reload
    assert context.start_cursor_ts == test_case.start_cursor_ts
    assert context.end_cursor_ts == test_case.end_cursor_ts
    assert context.start_cursor_int == test_case.start_cursor_int
    assert context.end_cursor_int == test_case.end_cursor_int
    assert context.current_cursor_value == test_case.expected_current_cursor_value
    cost_context: CostResourceContext | None = observed_cost_contexts[0]
    assert cost_context is not None
    assert cost_context.resource_type == "loader"
    assert cost_context.resource_name == test_case.loader_name
    assert cost_context.phase == "load"
    assert cost_context.attempt == 1


@pytest.mark.parametrize(
    "test_case",
    [
        SourceLoadExecutionContextTestCase(
            description="injects provider into source loader execution",
            source_name="raw_orders",
            loader_name="raw_orders_loader",
            target_table="orders",
            database=None,
            schema=None,
            run_id="run-123",
            target="dev",
            vars={},
            is_reload=False,
            start_cursor_ts=None,
            end_cursor_ts=None,
            start_cursor_int=None,
            end_cursor_int=None,
            expected_target="orders",
            expected_current_cursor_value=None,
            expected_status=ExecutionStatus.SUCCESS,
            expected_rows_loaded=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_parameter_when_executing_source_loader_then_provider_is_injected(
    test_case: SourceLoadExecutionContextTestCase,
) -> None:
    observed_labels: list[str] = []

    def raw_orders_loader(
        ctx: LoaderContext,
        slack_provider: ExecutionSlackProvider,
    ) -> None:
        observed_labels.append(f"{ctx.target}:{slack_provider.label}")
        return None

    providers: ProviderContainer = ProviderSession(
        {"slack_provider": ExecutionSlackProvider(label="slack")}
    ).providers

    result: LoadExecutionResult = execute_source_load(
        source_entry=SourceEntry(
            name=test_case.source_name,
            table="__loader__raw_orders",
            loader=test_case.loader_name,
            meta={"sqlbuild_loader_node": True},
        ),
        loader_function=DiscoveredLoaderFunction(
            file_path=Path("python/nodes/loaders/raw.py"),
            relative_path=Path("python/nodes/loaders/raw.py"),
            name=test_case.loader_name,
            function=raw_orders_loader,
            destination="staging_raw_orders",
        ),
        adapter=LoaderContextTestAdapter(),
        connection_config={},
        connection=object(),
        statement_recorder=StatementRecorder(),
        runtime=LoadRuntimeParams(
            run_id=test_case.run_id,
            target=test_case.target,
            vars=test_case.vars,
            is_reload=test_case.is_reload,
            providers=providers,
        ),
    )

    assert result.status == test_case.expected_status
    assert result.rows_loaded == test_case.expected_rows_loaded
    assert tuple(observed_labels) == ("dev:slack",)


@pytest.mark.parametrize(
    "test_case",
    [
        SourceLoadExecutionContextTestCase(
            description="exposes providers on source loader context",
            source_name="raw_orders",
            loader_name="raw_orders_loader",
            target_table="orders",
            database=None,
            schema=None,
            run_id="run-123",
            target="dev",
            vars={},
            is_reload=False,
            start_cursor_ts=None,
            end_cursor_ts=None,
            start_cursor_int=None,
            end_cursor_int=None,
            expected_target="orders",
            expected_current_cursor_value=None,
            expected_status=ExecutionStatus.SUCCESS,
            expected_rows_loaded=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_container_when_executing_source_loader_then_context_exposes_providers(
    test_case: SourceLoadExecutionContextTestCase,
) -> None:
    observed_labels: list[str] = []

    def raw_orders_loader(ctx: LoaderContext) -> None:
        attr_provider: ExecutionSlackProvider = cast(
            ExecutionSlackProvider, ctx.providers.slack_provider
        )
        item_provider: ExecutionSlackProvider = cast(
            ExecutionSlackProvider, ctx.providers["slack_provider"]
        )
        observed_labels.append(f"{attr_provider.label}:{item_provider.label}")
        return None

    providers: ProviderContainer = ProviderSession(
        {"slack_provider": ExecutionSlackProvider(label="slack")}
    ).providers

    result: LoadExecutionResult = execute_source_load(
        source_entry=SourceEntry(
            name=test_case.source_name,
            table="__loader__raw_orders",
            loader=test_case.loader_name,
            meta={"sqlbuild_loader_node": True},
        ),
        loader_function=DiscoveredLoaderFunction(
            file_path=Path("python/nodes/loaders/raw.py"),
            relative_path=Path("python/nodes/loaders/raw.py"),
            name=test_case.loader_name,
            function=raw_orders_loader,
            destination="staging_raw_orders",
        ),
        adapter=LoaderContextTestAdapter(),
        connection_config={},
        connection=object(),
        statement_recorder=StatementRecorder(),
        runtime=LoadRuntimeParams(
            run_id=test_case.run_id,
            target=test_case.target,
            vars=test_case.vars,
            is_reload=test_case.is_reload,
            providers=providers,
        ),
    )

    assert result.status == test_case.expected_status
    assert result.rows_loaded == test_case.expected_rows_loaded
    assert tuple(observed_labels) == ("slack:slack",)


@pytest.mark.parametrize(
    "test_case",
    [
        SourceLoadExecutionContextTestCase(
            description="normalizes missing provider container as source loader failure",
            source_name="raw_orders",
            loader_name="raw_orders_loader",
            target_table="orders",
            database=None,
            schema=None,
            run_id="run-123",
            target="dev",
            vars={},
            is_reload=False,
            start_cursor_ts=None,
            end_cursor_ts=None,
            start_cursor_int=None,
            end_cursor_int=None,
            expected_target="orders",
            expected_current_cursor_value=None,
            expected_status=ExecutionStatus.FAILED,
            expected_rows_loaded=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_provider_container_when_executing_source_loader_then_failure_is_recorded(
    test_case: SourceLoadExecutionContextTestCase,
) -> None:
    def raw_orders_loader(
        ctx: LoaderContext,
        slack_provider: ExecutionSlackProvider,
    ) -> None:
        del ctx, slack_provider
        return None

    result: LoadExecutionResult = execute_source_load(
        source_entry=SourceEntry(
            name=test_case.source_name,
            table="__loader__raw_orders",
            loader=test_case.loader_name,
            meta={"sqlbuild_loader_node": True},
        ),
        loader_function=DiscoveredLoaderFunction(
            file_path=Path("python/nodes/loaders/raw.py"),
            relative_path=Path("python/nodes/loaders/raw.py"),
            name=test_case.loader_name,
            function=raw_orders_loader,
            destination="staging_raw_orders",
        ),
        adapter=LoaderContextTestAdapter(),
        connection_config={},
        connection=object(),
        statement_recorder=StatementRecorder(),
        runtime=LoadRuntimeParams(
            run_id=test_case.run_id,
            target=test_case.target,
            vars=test_case.vars,
            is_reload=test_case.is_reload,
        ),
    )

    assert result.status == test_case.expected_status
    assert result.rows_loaded == test_case.expected_rows_loaded
    assert (
        "Provider parameter 'slack_provider' requires provider 'slack_provider', "
        "but no provider container is available"
    ) in (result.error_message or "")


@pytest.mark.parametrize(
    "test_case",
    [
        SourceLoadNoneReturnTestCase(
            description="allows targeted self-managed intermediate loader to return none",
            source_name="fetch_orders",
            loader_name="fetch_orders",
            loader_target="staging_fetch_orders",
            expected_status=ExecutionStatus.SUCCESS,
            expected_rows_loaded=0,
        ),
        SourceLoadNoneReturnTestCase(
            description="fails untargeted self-managed intermediate loader returning none",
            source_name="fetch_orders",
            loader_name="fetch_orders",
            loader_target=None,
            expected_status=ExecutionStatus.FAILED,
            expected_rows_loaded=0,
            expected_error_fragment="returned no rows and has no destination declared",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_self_managed_intermediate_loader_when_returning_none_then_applies_target_rule(
    test_case: SourceLoadNoneReturnTestCase,
) -> None:
    def fetch_orders(ctx: LoaderContext) -> None:
        del ctx
        return None

    result: LoadExecutionResult = execute_source_load(
        source_entry=SourceEntry(
            name=test_case.source_name,
            table="__loader__fetch_orders",
            loader=test_case.loader_name,
            meta={"sqlbuild_loader_node": True},
        ),
        loader_function=DiscoveredLoaderFunction(
            file_path=Path("python/nodes/loaders/raw.py"),
            relative_path=Path("python/nodes/loaders/raw.py"),
            name=test_case.loader_name,
            function=fetch_orders,
            destination=test_case.loader_target,
        ),
        adapter=LoaderContextTestAdapter(),
        connection_config={},
        connection=object(),
        statement_recorder=StatementRecorder(),
        runtime=LoadRuntimeParams(
            run_id="run-1",
            target=None,
            vars={},
            is_reload=False,
        ),
    )

    assert result.status == test_case.expected_status
    assert result.rows_loaded == test_case.expected_rows_loaded
    assert test_case.expected_error_fragment in (result.error_message or "")
