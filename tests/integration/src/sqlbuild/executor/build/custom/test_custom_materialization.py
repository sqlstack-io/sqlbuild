"""Integration tests for custom materialization executor lifecycle."""

from __future__ import annotations

from pathlib import Path
from types import SimpleNamespace
from typing import Any, cast

import duckdb
import pytest

from sqlbuild._native import (
    AuditPlanEntry,
    CompiledRelationLocation,
    DiscoveredHookFunction,
    ModelPlanEntry,
    PlanReason,
    PythonHookEntry,
    SqlHookEntry,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.auditing.types import AuditOutcome
from sqlbuild.executor.custom.models import (
    MaterializationContext,
    MaterializationResult,
)
from sqlbuild.executor.run.models import ModelExecutionResult
from sqlbuild.executor.run.types import ExecutionPhase
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.observability import (
    EventDispatcher,
    LifecycleEvent,
    dispatcher_scope,
    invocation_scope,
)
from tests.integration.src.sqlbuild.executor.build.custom._test_types import (
    CleanupTestCase,
    ContextVerificationTestCase,
    CustomFailureTestCase,
    CustomSuccessTestCase,
    FrameworkAuditTestCase,
    HookTestCase,
    UserAuditTestCase,
)
from tests.integration.src.sqlbuild.executor.build.custom.helpers import (
    build_cleanup_fn,
    build_custom_plan_entry,
    build_failing_audit,
    build_passing_audit,
    build_simple_fn,
    build_user_audit_fn,
    fail_custom_hook,
    insert_custom_hook_log,
    operation_events,
    relation_exists,
    resolve_fn,
    row_count,
    run_custom_entry,
)


@pytest.mark.parametrize(
    "test_case",
    (
        CustomFailureTestCase(
            description="custom materialization returns unsupported object",
            model_sql="SELECT 1 AS id",
            expected_status=ExecutionStatus.FAILED,
            expected_failed_phase=ExecutionPhase.CUSTOM_MATERIALIZATION,
            expected_error_fragment="must return MaterializationResult",
            fn_name="malformed_custom",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_malformed_custom_return_when_executing_then_operation_fails_once(
    test_case: CustomFailureTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    def malformed_custom(ctx: MaterializationContext) -> object:
        del ctx
        return SimpleNamespace(failed=False)

    with invocation_scope("inv-malformed-custom"), dispatcher_scope(dispatcher):
        result: ModelExecutionResult = run_custom_entry(
            adapter=adapter,
            connection=connection,
            entry=build_custom_plan_entry(sql=test_case.model_sql),
            materialize_fn=cast(Any, malformed_custom),
        )

    observed_operations: tuple[LifecycleEvent, ...] = operation_events(events)
    assert result.status == test_case.expected_status
    assert result.failed_phase == test_case.expected_failed_phase
    assert test_case.expected_error_fragment in (result.error_message or "")
    assert tuple(event.event_type for event in observed_operations) == (
        "operation_started",
        "operation_failed",
    )
    assert observed_operations[0].payload["operation_name"] == "python_materialization"


@pytest.mark.parametrize(
    "test_case",
    [
        CustomSuccessTestCase(
            description="simple table creation via custom materialization",
            model_sql="SELECT 1 AS id, 'alice' AS name UNION ALL SELECT 2, 'bob'",
            expected_status=ExecutionStatus.SUCCESS,
            expected_row_count=2,
            fn_name="simple",
        ),
        CustomSuccessTestCase(
            description="staging with cleanup via custom materialization",
            model_sql="SELECT 1 AS id, 'alice' AS name",
            expected_status=ExecutionStatus.SUCCESS,
            expected_row_count=1,
            fn_name="staging",
        ),
        CustomSuccessTestCase(
            description="user runs audits and returns results",
            model_sql="SELECT 1 AS id, 'alice' AS name",
            expected_status=ExecutionStatus.SUCCESS,
            expected_row_count=1,
            fn_name="audit_running",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_custom_materialization_when_executing_then_succeeds(
    test_case: CustomSuccessTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    entry: ModelPlanEntry = build_custom_plan_entry(sql=test_case.model_sql)

    result: ModelExecutionResult = run_custom_entry(
        adapter=adapter,
        connection=connection,
        entry=entry,
        materialize_fn=resolve_fn(test_case.fn_name),
    )

    assert result.status == test_case.expected_status
    assert row_count(connection, qualified_name="main.test_model") == test_case.expected_row_count


@pytest.mark.parametrize(
    "test_case",
    [
        CustomFailureTestCase(
            description="user returns failed with error message",
            model_sql="SELECT 1 AS id",
            expected_status=ExecutionStatus.FAILED,
            expected_failed_phase=ExecutionPhase.CUSTOM_MATERIALIZATION,
            expected_error_fragment="user-reported failure",
            fn_name="failing",
        ),
        CustomFailureTestCase(
            description="user function raises exception",
            model_sql="SELECT 1 AS id",
            expected_status=ExecutionStatus.FAILED,
            expected_failed_phase=ExecutionPhase.CUSTOM_MATERIALIZATION,
            expected_error_fragment="materialization crashed",
            fn_name="excepting",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_custom_materialization_when_failing_then_reports_failure(
    test_case: CustomFailureTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    entry: ModelPlanEntry = build_custom_plan_entry(sql=test_case.model_sql)

    result: ModelExecutionResult = run_custom_entry(
        adapter=adapter,
        connection=connection,
        entry=entry,
        materialize_fn=resolve_fn(test_case.fn_name),
    )

    assert result.status == test_case.expected_status
    assert result.failed_phase == test_case.expected_failed_phase
    assert result.error_message is not None
    assert test_case.expected_error_fragment in result.error_message


@pytest.mark.parametrize(
    "test_case",
    [
        HookTestCase(
            description="SQL and Python hooks execute around custom materialization",
            pre_hook=[
                SqlHookEntry(
                    statement=(
                        "CREATE TABLE main.hook_marker (phase VARCHAR); "
                        "INSERT INTO main.hook_marker VALUES ('sql_pre')"
                    )
                ),
                PythonHookEntry(name="insert_hook_log", kwargs={"phase": "python_pre"}),
            ],
            post_hook=[
                PythonHookEntry(name="insert_hook_log", kwargs={"phase": "python_post"}),
                SqlHookEntry(statement="INSERT INTO main.hook_marker VALUES ('sql_post')"),
            ],
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("sql/hooks/python/custom.py"),
                    name="insert_hook_log",
                    function=insert_custom_hook_log,
                ),
            ),
            expected_status=ExecutionStatus.SUCCESS,
            expected_table_exists=True,
            expected_query_results=(
                (
                    "SELECT phase FROM main.hook_marker ORDER BY rowid",
                    (("sql_pre",), ("python_pre",), ("python_post",), ("sql_post",)),
                ),
            ),
        ),
        HookTestCase(
            description="pre_hook failure skips materialization",
            pre_hook=[SqlHookEntry(statement="SELECT * FROM nonexistent_table_for_hook")],
            post_hook=None,
            expected_status=ExecutionStatus.FAILED,
            expected_failed_phase=ExecutionPhase.PRE_HOOK,
            expected_table_exists=False,
        ),
        HookTestCase(
            description="post_hook failure after successful materialization",
            pre_hook=None,
            post_hook=[SqlHookEntry(statement="SELECT * FROM nonexistent_table_for_hook")],
            expected_status=ExecutionStatus.FAILED,
            expected_failed_phase=ExecutionPhase.POST_HOOK,
            expected_table_exists=True,
        ),
        HookTestCase(
            description="python pre_hook failure skips custom materialization",
            pre_hook=[PythonHookEntry(name="fail_hook", kwargs={"message": "custom pre failed"})],
            post_hook=None,
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("sql/hooks/python/custom.py"),
                    name="fail_hook",
                    function=fail_custom_hook,
                ),
            ),
            expected_status=ExecutionStatus.FAILED,
            expected_failed_phase=ExecutionPhase.PRE_HOOK,
            expected_error_fragment='pre_hooks[0] python("fail_hook") failed: custom pre failed',
            expected_table_exists=False,
        ),
        HookTestCase(
            description="python post_hook failure after custom materialization",
            pre_hook=None,
            post_hook=[PythonHookEntry(name="fail_hook", kwargs={"message": "custom post failed"})],
            hook_functions=(
                DiscoveredHookFunction(
                    file_path=Path(__file__),
                    relative_path=Path("sql/hooks/python/custom.py"),
                    name="fail_hook",
                    function=fail_custom_hook,
                ),
            ),
            expected_status=ExecutionStatus.FAILED,
            expected_failed_phase=ExecutionPhase.POST_HOOK,
            expected_error_fragment='post_hooks[0] python("fail_hook") failed: custom post failed',
            expected_table_exists=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_custom_materialization_with_hooks_when_executing_then_handles_hooks(
    test_case: HookTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    entry: ModelPlanEntry = build_custom_plan_entry(
        sql="SELECT 1 AS id",
        pre_hooks=test_case.pre_hook,
        post_hooks=test_case.post_hook,
    )
    assert all(
        isinstance(hook_function, DiscoveredHookFunction)
        for hook_function in test_case.hook_functions
    )

    result: ModelExecutionResult = run_custom_entry(
        adapter=adapter,
        connection=connection,
        entry=entry,
        materialize_fn=build_simple_fn(),
        hook_functions=cast(tuple[DiscoveredHookFunction, ...], test_case.hook_functions),
    )

    assert result.status == test_case.expected_status
    assert result.failed_phase == test_case.expected_failed_phase
    assert (test_case.expected_error_fragment or "") in (result.error_message or "")
    assert (
        relation_exists(connection, schema="main", name="test_model")
        == test_case.expected_table_exists
    )
    for query, expected_rows in test_case.expected_query_results:
        actual_rows: tuple[tuple[object, ...], ...] = tuple(
            tuple(row) for row in connection.execute(query).fetchall()
        )
        assert actual_rows == expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        FrameworkAuditTestCase(
            description="framework runs passing audit when user does not call run_audits",
            audit_passes=True,
            audit=build_passing_audit(name="check_empty", target_name="test_model"),
            expected_status=ExecutionStatus.SUCCESS,
            expected_audit_count=1,
        ),
        FrameworkAuditTestCase(
            description="framework audit error blocks post_hook",
            audit_passes=False,
            audit=build_failing_audit(name="check_rows", target_name="test_model"),
            expected_status=ExecutionStatus.FAILED,
            expected_failed_phase=ExecutionPhase.AUDIT,
            expected_audit_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_custom_materialization_when_framework_runs_audits_then_handles_outcome(
    test_case: FrameworkAuditTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    entry: ModelPlanEntry = build_custom_plan_entry(sql="SELECT 1 AS id")
    model_locations: dict[str, CompiledRelationLocation] = {"test_model": entry.destination}
    audit: AuditPlanEntry = test_case.audit

    result: ModelExecutionResult = run_custom_entry(
        adapter=adapter,
        connection=connection,
        entry=entry,
        materialize_fn=build_user_audit_fn(expect_pass=test_case.audit_passes),
        model_audits=(audit,),
        model_locations=model_locations,
    )
    actual_audit: AuditPlanEntry = test_case.audit

    result: ModelExecutionResult = run_custom_entry(
        adapter=adapter,
        connection=connection,
        entry=entry,
        materialize_fn=build_simple_fn(),
        model_audits=(actual_audit,),
        model_locations=model_locations,
    )

    assert result.status == test_case.expected_status
    assert result.failed_phase == test_case.expected_failed_phase
    assert len(result.audit_results) == test_case.expected_audit_count


@pytest.mark.parametrize(
    "test_case",
    [
        UserAuditTestCase(
            description="user calls run_audits with passing audit against staging",
            audit_passes=True,
            audit=build_passing_audit(name="check_empty", target_name="test_model"),
            expected_status=ExecutionStatus.SUCCESS,
            expected_audit_count=1,
            expected_audit_outcome=AuditOutcome.PASS,
        ),
        UserAuditTestCase(
            description="user calls run_audits with failing audit and returns failed",
            audit_passes=False,
            audit=build_failing_audit(name="check_rows", target_name="test_model"),
            expected_status=ExecutionStatus.FAILED,
            expected_audit_count=1,
            expected_audit_outcome=AuditOutcome.ERROR,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_custom_materialization_when_user_runs_audits_then_handles_outcome(
    test_case: UserAuditTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    entry: ModelPlanEntry = build_custom_plan_entry(sql="SELECT 1 AS id")
    model_locations: dict[str, CompiledRelationLocation] = {"test_model": entry.destination}
    actual_audit: AuditPlanEntry = test_case.audit

    result: ModelExecutionResult = run_custom_entry(
        adapter=adapter,
        connection=connection,
        entry=entry,
        materialize_fn=build_user_audit_fn(expect_pass=test_case.audit_passes),
        model_audits=(actual_audit,),
        model_locations=model_locations,
    )

    assert result.status == test_case.expected_status
    assert len(result.audit_results) == test_case.expected_audit_count
    assert result.audit_results[0].outcome == test_case.expected_audit_outcome


@pytest.mark.parametrize(
    "test_case",
    [
        CleanupTestCase(
            description="cleanup relations dropped on success",
            user_fails=False,
            expected_staging_exists=False,
        ),
        CleanupTestCase(
            description="cleanup relations kept on failure",
            user_fails=True,
            expected_staging_exists=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_custom_materialization_with_cleanup_when_completing_then_handles_cleanup(
    test_case: CleanupTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    entry: ModelPlanEntry = build_custom_plan_entry(sql="SELECT 1 AS id")

    run_custom_entry(
        adapter=adapter,
        connection=connection,
        entry=entry,
        materialize_fn=build_cleanup_fn(fail=test_case.user_fails),
    )

    assert (
        relation_exists(connection, schema="main", name="test_model__staging")
        == test_case.expected_staging_exists
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ContextVerificationTestCase(
            description="context fields populated for first run with full refresh",
            reason=PlanReason.FULL_REFRESH,
            custom_config={"tracking_schema": "meta"},
            custom_placeholders={"start": "'2020-01-01'"},
            target="prod",
            effective_vars={"user": "kevin"},
            expected_is_first_run=True,
            expected_is_full_refresh=True,
            expected_query_changed=False,
            expected_config_key="tracking_schema",
            expected_config_value="meta",
            expected_placeholder_key="start",
            expected_placeholder_value="'2020-01-01'",
            expected_target="prod",
            expected_var_key="user",
            expected_var_value="kevin",
            expected_qualified_name="meta.partition_state",
            expected_destination_schema_qualified_name="main.partition_state",
            expected_preserved_qualified_name="external.partition_state",
        ),
        ContextVerificationTestCase(
            description="context fields populated for query changed run",
            reason=PlanReason.QUERY_CHANGED,
            custom_config={"mode": "incremental"},
            custom_placeholders={},
            target="dev",
            effective_vars={"schema_prefix": "staging"},
            expected_is_first_run=True,
            expected_is_full_refresh=False,
            expected_query_changed=True,
            expected_config_key="mode",
            expected_config_value="incremental",
            expected_placeholder_key="",
            expected_placeholder_value="",
            expected_target="dev",
            expected_var_key="schema_prefix",
            expected_var_value="staging",
            expected_qualified_name="meta.partition_state",
            expected_destination_schema_qualified_name="main.partition_state",
            expected_preserved_qualified_name="external.partition_state",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_custom_materialization_when_executing_then_context_fields_populated(
    test_case: ContextVerificationTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    entry: ModelPlanEntry = build_custom_plan_entry(
        sql="SELECT 1 AS id",
        reason=test_case.reason,
        custom_config=test_case.custom_config,
        custom_placeholders=test_case.custom_placeholders,
    )

    captured: dict[str, Any] = {}

    def materialize(ctx: MaterializationContext) -> MaterializationResult:
        captured["is_first_run"] = ctx.is_first_run
        captured["is_full_refresh"] = ctx.is_full_refresh
        captured["query_changed"] = ctx.query_changed
        captured["config"] = ctx.config
        captured["placeholders"] = ctx.placeholders
        captured["relation"] = ctx.destination
        captured["target"] = ctx.build_target
        captured["vars"] = ctx.vars
        captured["qualified_name"] = ctx.qualify_name(
            name="partition_state", schema="meta", database=None
        )
        captured["destination_schema_qualified_name"] = ctx.qualify_in_destination_schema(
            "partition_state"
        )
        captured["preserved_qualified_name"] = ctx.qualify_in_destination_schema(
            "external.partition_state"
        )
        ctx.adapter.create_table_as(
            connection=ctx.connection,
            destination=ctx.destination,
            sql=ctx.sql,
            statement_recorder=ctx.statement_recorder,
        )
        return MaterializationResult(relation=ctx.destination)

    run_custom_entry(
        adapter=adapter,
        connection=connection,
        entry=entry,
        materialize_fn=materialize,
        target=test_case.target,
        effective_vars=test_case.effective_vars,
    )

    assert captured["is_first_run"] == test_case.expected_is_first_run
    assert captured["is_full_refresh"] == test_case.expected_is_full_refresh
    assert captured["query_changed"] == test_case.expected_query_changed
    assert captured["config"][test_case.expected_config_key] == test_case.expected_config_value
    assert captured["target"] == test_case.expected_target
    assert captured["relation"] == "main.test_model"
    assert captured["vars"][test_case.expected_var_key] == test_case.expected_var_value
    assert captured["qualified_name"] == test_case.expected_qualified_name
    assert (
        captured["destination_schema_qualified_name"]
        == test_case.expected_destination_schema_qualified_name
    )
    assert captured["preserved_qualified_name"] == test_case.expected_preserved_qualified_name
