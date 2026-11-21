from __future__ import annotations

import re
from dataclasses import replace
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import (
    CompiledRelationLocation,
    CostContext,
    DiscoveredHookFunction,
    ModelPlanEntry,
    PythonHookEntry,
    SqlHookEntry,
)
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.cost.models import CostResourceContext
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.run._helpers.execution.hooks import execute_hooks, render_hooks
from sqlbuild.executor.run._helpers.materializations.view import execute_view_entry
from sqlbuild.executor.run.models import (
    HookContext,
    HookExecutionResult,
    HookRunContext,
    ModelExecutionResult,
    ModelMaterializationContext,
)
from sqlbuild.executor.run.types import HookPhase
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.hooks import HookContext as PublicHookContext
from sqlbuild.observability import (
    EventDispatcher,
    LifecycleEvent,
    dispatcher_scope,
    invocation_scope,
)
from sqlbuild.refs import model, source
from tests.unit.src.sqlbuild.executor.python_nodes._helpers.helpers import (
    python_operation_events,
)
from tests.unit.src.sqlbuild.executor.run._helpers._test_types import (
    ExecuteHooksTestCase,
    PublicHookContextExportTestCase,
    PythonHookContextParameterTestCase,
    PythonHookExecutionTestCase,
    PythonHookHardCodedRelationTestCase,
    PythonHookInvalidReturnTestCase,
    PythonHookInvocationTestCase,
    PythonHookReadsTestCase,
    PythonHookRuntimeErrorTestCase,
    PythonHookSkipTestCase,
    PythonHookUndeclaredReadTestCase,
    RenderHooksTestCase,
)
from tests.unit.src.sqlbuild.executor.run._helpers.helpers import (
    build_result_model_plan_entry,
    run_python_hook_with_relations,
)


@pytest.mark.parametrize(
    "test_case",
    [
        PublicHookContextExportTestCase(
            description="exports hook context from public hooks module",
            expected_export_name="HookContext",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_public_hooks_module_when_importing_then_hook_context_is_exported(
    test_case: PublicHookContextExportTestCase,
) -> None:
    assert PublicHookContext.__name__ == test_case.expected_export_name
    assert PublicHookContext is HookContext


@pytest.mark.parametrize(
    "test_case",
    [
        RenderHooksTestCase(
            description="renders only SQL entries from mixed typed hooks",
            hooks=[
                SqlHookEntry(statement="CREATE TABLE hook_log AS SELECT 1"),
                PythonHookEntry(name="notify", kwargs={"message": "done"}),
                SqlHookEntry(statement="INSERT INTO hook_log SELECT 2"),
            ],
            expected_statements=(
                "CREATE TABLE hook_log AS SELECT 1",
                "INSERT INTO hook_log SELECT 2",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_typed_hooks_when_rendering_then_returns_expected_statements(
    test_case: RenderHooksTestCase,
) -> None:
    rendered: tuple[str, ...] = render_hooks(
        hooks=test_case.hooks,
        phase=HookPhase.POST_HOOKS,
    )

    assert rendered == test_case.expected_statements


@pytest.mark.parametrize(
    "test_case",
    [
        ExecuteHooksTestCase(
            description="executes typed SQL statements in order",
            hooks=[
                SqlHookEntry(statement="CREATE TABLE hook_log AS SELECT 1"),
                SqlHookEntry(statement="INSERT INTO hook_log SELECT 2"),
            ],
            expected_rows=((1,), (2,)),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_typed_sql_hooks_when_executing_then_sql_statements_run_in_order(
    test_case: ExecuteHooksTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with invocation_scope("inv-sql-hook-order"), dispatcher_scope(dispatcher):
        execute_hooks(
            connection=connection,
            adapter=adapter,
            hooks=test_case.hooks,
            phase=HookPhase.PRE_HOOKS,
        )

    rows: list[tuple[int]] = connection.execute("SELECT * FROM hook_log ORDER BY 1").fetchall()
    operation_events: tuple[LifecycleEvent, ...] = python_operation_events(events)
    starts: tuple[LifecycleEvent, LifecycleEvent] = (
        operation_events[0],
        operation_events[2],
    )
    assert tuple(rows) == test_case.expected_rows
    assert tuple(event.payload["hook_index"] for event in starts) == (0, 1)
    assert tuple(event.payload["hook_phase"] for event in starts) == ("pre_hooks", "pre_hooks")
    assert tuple(event.payload["hook_type"] for event in starts) == ("sql", "sql")
    assert all("hook_name" not in event.payload for event in starts)
    assert "CREATE TABLE" not in repr(starts)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonHookExecutionTestCase(
            description="fails clearly when runtime hook registry is missing entry",
            hooks=[PythonHookEntry(name="notify", kwargs={"message": "done"})],
            expected_error_fragment=(
                r"post_hooks\[0\] python\(\"notify\"\) was not found in the runtime hook registry"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_hook_without_runtime_registry_when_executing_then_it_fails_clearly(
    test_case: PythonHookExecutionTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})

    with pytest.raises(ExecutorInputError, match=test_case.expected_error_fragment):
        execute_hooks(
            connection=connection,
            adapter=adapter,
            hooks=test_case.hooks,
            phase=HookPhase.POST_HOOKS,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        PythonHookInvocationTestCase(
            description="invokes function with context and kwargs",
            expected_message="done",
            expected_rows=[(1,)],
            expected_model_name="orders",
            expected_phase="post_hooks",
            expected_hook_name="notify",
            expected_hook_index=0,
            expected_run_id="run-1",
            expected_target="dev",
            expected_vars={"channel": "alerts"},
            expected_destination_name="orders",
            expected_destination_schema="main",
            expected_adapter_name="duckdb",
            expected_recorded_events=(
                "CREATE TABLE hook_log AS SELECT 1 AS value",
                "SELECT value FROM hook_log",
                "done",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_hook_when_executing_then_invokes_function_with_context_and_kwargs(
    test_case: PythonHookInvocationTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    statement_recorder: StatementRecorder = StatementRecorder()
    captured: list[tuple[HookContext, str, list[tuple[object, ...]]]] = []
    observed_cost_contexts: list[CostResourceContext | None] = []

    def notify(ctx: HookContext, message: str) -> None:
        observed_cost_contexts.append(CostContext.current())
        ctx.execute_sql("CREATE TABLE hook_log AS SELECT 1 AS value")
        rows: list[tuple[object, ...]] = ctx.query("SELECT value FROM hook_log")
        ctx.log.info(message)
        captured.append((ctx, message, rows))

    with CostContext.scope(
        run_id="run-1",
        resource_type="model",
        resource_name="orders",
        phase="execute",
    ):
        execute_hooks(
            connection=connection,
            adapter=adapter,
            hooks=[PythonHookEntry(name="notify", kwargs={"message": "done"})],
            phase=HookPhase.POST_HOOKS,
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("hooks/python/notifications.py"),
                    name="notify",
                    function=notify,
                ),
            ),
            hook_run=HookRunContext(
                model_name="orders",
                destination=CompiledRelationLocation(
                    database=None,
                    schema="main",
                    name="orders",
                    qualified_name=None,
                ),
                run_id="run-1",
                target="dev",
                effective_vars={"channel": "alerts"},
                statement_recorder=statement_recorder,
            ),
        )

    ctx, message, rows = captured[0]
    assert message == test_case.expected_message
    assert rows == test_case.expected_rows
    assert ctx.model_name == test_case.expected_model_name
    assert ctx.phase == test_case.expected_phase
    assert ctx.hook_name == test_case.expected_hook_name
    assert ctx.hook_index == test_case.expected_hook_index
    assert ctx.run_id == test_case.expected_run_id
    assert ctx.target == test_case.expected_target
    assert ctx.vars == test_case.expected_vars
    assert ctx.destination.name == test_case.expected_destination_name
    assert ctx.destination.schema == test_case.expected_destination_schema
    assert ctx.adapter_name == test_case.expected_adapter_name
    assert tuple(event.content for event in statement_recorder.snapshot()) == (
        test_case.expected_recorded_events
    )
    cost_context: CostResourceContext | None = observed_cost_contexts[0]
    assert cost_context is not None
    assert cost_context.resource_type == "hook"
    assert cost_context.resource_name == test_case.expected_hook_name
    assert cost_context.phase == test_case.expected_phase
    assert cost_context.attempt == 1


@pytest.mark.parametrize(
    "test_case",
    [
        PythonHookSkipTestCase(
            description="records skipped Python hook result",
            expected_skipped=True,
            expected_status="skipped",
            expected_skip_reason="external dependency disabled",
            expected_skip_mode="hard",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_hook_returns_skip_when_executing_then_records_skipped_hook_result(
    test_case: PythonHookSkipTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    hook_results: list[HookExecutionResult] = []

    def maybe_skip(ctx: HookContext) -> object:
        return ctx.skip(reason=test_case.expected_skip_reason, mode=test_case.expected_skip_mode)

    skipped: bool = execute_hooks(
        connection=connection,
        adapter=adapter,
        hooks=[PythonHookEntry(name="maybe_skip", kwargs={})],
        phase=HookPhase.PRE_HOOKS,
        hook_functions=(
            DiscoveredHookFunction(
                file_path=Path(__file__),
                relative_path=Path("hooks/python/maybe_skip.py"),
                name="maybe_skip",
                function=maybe_skip,
            ),
        ),
        hook_results=hook_results,
        hook_run=HookRunContext(
            model_name="orders",
            destination=CompiledRelationLocation(
                database=None,
                schema="main",
                name="orders",
                qualified_name=None,
            ),
        ),
    )

    assert skipped is test_case.expected_skipped
    assert hook_results[0].status.value == test_case.expected_status
    assert hook_results[0].skip_mode is not None
    assert hook_results[0].skip_mode.value == test_case.expected_skip_mode
    assert hook_results[0].skip_reason == test_case.expected_skip_reason


@pytest.mark.parametrize(
    "test_case",
    (
        PythonHookSkipTestCase(
            description="skipped pre hook stops remaining hooks",
            expected_skipped=True,
            expected_status="skipped",
            expected_skip_reason="stop pre hooks",
            hook_phase="pre_hooks",
        ),
        PythonHookSkipTestCase(
            description="skipped post hook stops remaining hooks",
            expected_skipped=True,
            expected_status="skipped",
            expected_skip_reason="stop post hooks",
            hook_phase="post_hooks",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_python_hook_returns_skip_when_executing_phase_then_later_hooks_do_not_run(
    test_case: PythonHookSkipTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    events: list[str] = []
    phase: HookPhase = HookPhase(test_case.hook_phase)

    def maybe_skip(ctx: HookContext) -> object:
        events.append("skip")
        return ctx.skip(reason=test_case.expected_skip_reason)

    def should_not_run(ctx: HookContext) -> None:
        events.append("later")

    skipped: bool = execute_hooks(
        connection=connection,
        adapter=adapter,
        hooks=[
            PythonHookEntry(name="maybe_skip", kwargs={}),
            PythonHookEntry(name="should_not_run", kwargs={}),
        ],
        phase=phase,
        hook_functions=(
            DiscoveredHookFunction(
                file_path=Path(__file__),
                relative_path=Path("hooks/python/maybe_skip.py"),
                name="maybe_skip",
                function=maybe_skip,
            ),
            DiscoveredHookFunction(
                file_path=Path(__file__),
                relative_path=Path("hooks/python/should_not_run.py"),
                name="should_not_run",
                function=should_not_run,
            ),
        ),
        hook_run=HookRunContext(
            model_name="orders",
            destination=CompiledRelationLocation(
                database=None,
                schema="main",
                name="orders",
                qualified_name=None,
            ),
        ),
    )

    assert skipped is test_case.expected_skipped
    assert tuple(events) == ("skip",)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonHookSkipTestCase(
            description="pre-hook skip skips view materialization",
            expected_skipped=True,
            expected_status="skipped",
            expected_skip_reason="source is disabled",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_pre_hook_returns_skip_when_executing_view_then_model_is_skipped(
    test_case: PythonHookSkipTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})

    def maybe_skip(ctx: HookContext) -> object:
        return ctx.skip(reason=test_case.expected_skip_reason)

    entry: ModelPlanEntry = replace(
        build_result_model_plan_entry(),
        pre_hooks=(PythonHookEntry(name="maybe_skip", kwargs={}),),
    )
    result: ModelExecutionResult = execute_view_entry(
        context=ModelMaterializationContext(
            entry=entry,
            adapter=adapter,
            connection=connection,
            model_locations={},
            seed_locations={},
            source_map={},
            python_source_read_map={},
            model_audits=(),
            run_id="run-1",
            query_change_tracking=False,
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("hooks/python/maybe_skip.py"),
                    name="maybe_skip",
                    function=maybe_skip,
                ),
            ),
        ),
    )

    assert result.status == ExecutionStatus.SKIPPED
    assert result.hook_results[0].status.value == test_case.expected_status
    assert result.skip_mode is not None
    assert result.skip_mode.value == test_case.expected_skip_mode
    assert result.hook_results[0].skip_reason == test_case.expected_skip_reason
    assert result.skip_reason == test_case.expected_skip_reason
    assert not adapter.relation_exists(
        connection=connection,
        database=entry.destination.database,
        schema=entry.destination.schema,
        name=entry.destination.name,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        PythonHookInvalidReturnTestCase(
            description="fails when Python hook returns payload",
            returned={"payload": 1},
            expected_error_fragment=(
                r"post_hooks\[0\] python\(\"invalid_return\"\) returned unsupported value; "
                r"return None or ctx\.skip\(\.\.\.\)"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_hook_returns_payload_when_executing_then_it_fails_clearly(
    test_case: PythonHookInvalidReturnTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    events: list[LifecycleEvent] = []

    def invalid_return(ctx: HookContext) -> object:
        return test_case.returned

    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)
    with invocation_scope("inv-malformed-hook"), dispatcher_scope(dispatcher):
        with pytest.raises(ExecutorInputError, match=test_case.expected_error_fragment):
            execute_hooks(
                connection=connection,
                adapter=adapter,
                hooks=[PythonHookEntry(name="invalid_return", kwargs={})],
                phase=HookPhase.POST_HOOKS,
                hook_functions=(
                    DiscoveredHookFunction(
                        file_path=Path(__file__),
                        relative_path=Path("hooks/python/invalid_return.py"),
                        name="invalid_return",
                        function=invalid_return,
                    ),
                ),
                hook_run=HookRunContext(
                    model_name="orders",
                    destination=CompiledRelationLocation(
                        database=None,
                        schema="main",
                        name="orders",
                        qualified_name=None,
                    ),
                ),
            )

    operation_events: tuple[LifecycleEvent, ...] = python_operation_events(events)
    assert tuple(event.event_type for event in operation_events) == (
        "operation_started",
        "operation_failed",
    )
    assert operation_events[0].payload["operation_name"] == "python_hook"
    assert operation_events[0].payload["hook_phase"] == "post_hooks"
    assert operation_events[0].payload["hook_index"] == 0
    assert operation_events[0].payload["hook_type"] == "python"
    assert operation_events[0].payload["hook_name"] == "invalid_return"


@pytest.mark.parametrize(
    "test_case",
    [
        PythonHookContextParameterTestCase(
            description="injects context parameter aliases and ignores return values",
            hook_name="aliases",
            expected_context_count=3,
            expected_return_ignored=None,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_hooks_when_executing_then_injects_supported_context_aliases(
    test_case: PythonHookContextParameterTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})
    captured_contexts: list[HookContext] = []

    def uses_ctx(ctx: HookContext) -> object:
        captured_contexts.append(ctx)
        return test_case.expected_return_ignored

    def uses_context(context: HookContext) -> object:
        captured_contexts.append(context)
        return test_case.expected_return_ignored

    def uses_hook_context(hook_context: HookContext) -> object:
        captured_contexts.append(hook_context)
        return test_case.expected_return_ignored

    execute_hooks(
        connection=connection,
        adapter=adapter,
        hooks=[
            PythonHookEntry(name="uses_ctx", kwargs={}),
            PythonHookEntry(name="uses_context", kwargs={}),
            PythonHookEntry(name="uses_hook_context", kwargs={}),
        ],
        phase=HookPhase.PRE_HOOKS,
        hook_functions=(
            DiscoveredHookFunction(
                file_path=Path(__file__),
                relative_path=Path("hooks/python/aliases.py"),
                name="uses_ctx",
                function=uses_ctx,
            ),
            DiscoveredHookFunction(
                file_path=Path(__file__),
                relative_path=Path("hooks/python/aliases.py"),
                name="uses_context",
                function=uses_context,
            ),
            DiscoveredHookFunction(
                file_path=Path(__file__),
                relative_path=Path("hooks/python/aliases.py"),
                name="uses_hook_context",
                function=uses_hook_context,
            ),
        ),
        hook_run=HookRunContext(
            model_name="orders",
            destination=CompiledRelationLocation(
                database=None,
                schema="main",
                name="orders",
                qualified_name=None,
            ),
            run_id="run-1",
        ),
    )

    assert len(captured_contexts) == test_case.expected_context_count
    assert tuple(ctx.hook_index for ctx in captured_contexts) == (0, 1, 2)
    assert all(ctx.phase == HookPhase.PRE_HOOKS for ctx in captured_contexts)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonHookRuntimeErrorTestCase(
            description="wraps Python hook exceptions with hook label",
            expected_error_fragment=r"post_hooks\[0\] python\(\"explode\"\) failed: boom",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_hook_raises_when_executing_then_reports_hook_label(
    test_case: PythonHookRuntimeErrorTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})

    def explode(ctx: HookContext) -> None:
        raise RuntimeError("boom")

    with pytest.raises(ExecutorInputError, match=test_case.expected_error_fragment):
        execute_hooks(
            connection=connection,
            adapter=adapter,
            hooks=[PythonHookEntry(name="explode", kwargs={})],
            phase=HookPhase.POST_HOOKS,
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("hooks/python/explode.py"),
                    name="explode",
                    function=explode,
                ),
            ),
            hook_run=HookRunContext(
                model_name="orders",
                destination=CompiledRelationLocation(
                    database=None,
                    schema="main",
                    name="orders",
                    qualified_name=None,
                ),
                run_id="run-1",
            ),
        )


@pytest.mark.parametrize(
    "test_case",
    [
        PythonHookExecutionTestCase(
            description="fails clearly for invalid hook entry shape",
            hooks=[SqlHookEntry(statement="SELECT 1"), object()],
            expected_error_fragment=(
                r"post_hooks\[1\] must be a SQL or Python hook entry, got object"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_hook_entry_when_executing_then_it_reports_hook_index(
    test_case: PythonHookExecutionTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": ":memory:"})

    with pytest.raises(ExecutorInputError, match=test_case.expected_error_fragment):
        execute_hooks(
            connection=connection,
            adapter=adapter,
            hooks=test_case.hooks,
            phase=HookPhase.POST_HOOKS,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        PythonHookReadsTestCase(
            description="declared model read resolves to the planned relation",
            reads=(model("customers"),),
            requested=model("customers"),
            expected_relation="main.customers",
        ),
        PythonHookReadsTestCase(
            description="declared source read resolves to the planned source relation",
            reads=(source("raw_orders"),),
            requested=source("raw_orders"),
            expected_relation="raw.orders",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_hook_reads_when_resolving_relation_then_returns_declared_relations_only(
    test_case: PythonHookReadsTestCase,
) -> None:
    resolved: list[str] = []

    def refresh_lookup(ctx: HookContext) -> None:
        resolved.append(ctx.relation(test_case.requested))

    run_python_hook_with_relations(function=refresh_lookup, reads=test_case.reads)

    assert resolved == [test_case.expected_relation]


@pytest.mark.parametrize(
    "test_case",
    [
        PythonHookUndeclaredReadTestCase(
            description="undeclared read raises",
            reads=(model("customers"),),
            requested=model("orders"),
            expected_error_fragment=(
                "SQL relation ref 'orders' must be declared in @hook(reads=...) before use"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_hook_undeclared_read_when_resolving_relation_then_raises(
    test_case: PythonHookUndeclaredReadTestCase,
) -> None:
    def refresh_lookup(ctx: HookContext) -> None:
        ctx.relation(test_case.requested)

    with pytest.raises(ExecutorInputError, match=re.escape(test_case.expected_error_fragment)):
        run_python_hook_with_relations(function=refresh_lookup, reads=test_case.reads)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonHookHardCodedRelationTestCase(
            description="hard-coded model name warns and the query still runs",
            enforce_explicit_references=True,
            expected_warning_fragments=(
                "[P008] hook 'refresh_lookup' on model 'orders' named model:customers",
            ),
        ),
        PythonHookHardCodedRelationTestCase(
            description="enforcement disabled does not warn",
            enforce_explicit_references=False,
            expected_warning_fragments=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_hook_hard_coding_a_project_relation_when_running_then_warns_only(
    test_case: PythonHookHardCodedRelationTestCase,
) -> None:
    rows: list[list[tuple[object, ...]]] = []
    warnings: list[str] = []

    def refresh_lookup(ctx: HookContext) -> None:
        name: str = "customers"
        rows.append(ctx.query(f"SELECT customer_id FROM {name}"))

    run_python_hook_with_relations(
        function=refresh_lookup,
        reads=(),
        enforce_explicit_references=test_case.enforce_explicit_references,
        warnings=warnings,
    )

    assert rows == [[(1,)]]
    assert len(warnings) == len(test_case.expected_warning_fragments)
    assert all(
        fragment in warning
        for warning, fragment in zip(warnings, test_case.expected_warning_fragments, strict=True)
    )
