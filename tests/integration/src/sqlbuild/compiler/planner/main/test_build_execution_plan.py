from __future__ import annotations

import re
from dataclasses import replace
from typing import Any

import pytest

from sqlbuild._native import (
    BackfillAction,
    CursorBounds,
    CursorsConfig,
    FutureCursorAction,
    FutureCursorsConfig,
    LocalConfig,
    ModelPlanEntry,
    PlanAction,
    PlanOutput,
    PlanReason,
    PlanWarning,
    ProjectConfig,
    ResolvedTableType,
    TableType,
    TableTypeSource,
    TargetConfig,
    WarningSeverity,
    compiled_model_record,
    format_plan,
    planner_identity_work_info,
    project_with,
)
from sqlbuild.adapter.contract.types import BuiltinAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.planner.main._test_types import (
    BuildExecutionPlanTestCase,
    FormatPlanIntegrationTestCase,
    FutureCursorPlannerErrorTestCase,
    FutureCursorPlannerTestCase,
    PlannerChangeDetectionWorkTestCase,
    SourceCursorInputPlanErrorTestCase,
    TableTypePlanAssemblyTestCase,
)
from tests.integration.src.sqlbuild.compiler.planner.main.helpers import (
    build_execution_plan_from_kwargs,
    build_future_cursor_project,
    build_project_from_format_test_case,
    build_project_from_source_cursor_input_test_case,
    build_project_from_test_case,
    write_previous_function_fingerprints,
)


@pytest.mark.parametrize(
    "test_case",
    [
        PlannerChangeDetectionWorkTestCase(
            description="selection diagnostics disabled",
            selection_diagnostics=False,
            expected_stale_detection_calls={},
            expected_warning_count=0,
        ),
        PlannerChangeDetectionWorkTestCase(
            description="selection diagnostics enabled",
            selection_diagnostics=True,
            expected_stale_detection_calls={"stg_orders": 1},
            expected_warning_count=0,
            expected_identity_builds=2,
        ),
        PlannerChangeDetectionWorkTestCase(
            description="selection diagnostics preserve independent query tracking",
            selection_diagnostics=True,
            expected_stale_detection_calls={"stg_orders": 1, "orders": 1},
            expected_warning_count=0,
            query_change_tracking=False,
            expected_identity_builds=2,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_selected_model_when_planning_then_each_required_model_is_change_detected_once(
    test_case: PlannerChangeDetectionWorkTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    connection.execute("CREATE TABLE staging.stg_orders AS SELECT 1 AS order_id")
    project: CompiledProjectView = build_project_from_test_case(
        BuildExecutionPlanTestCase(
            description=test_case.description,
            setup_sql=(),
            model_locations={"stg_orders": "staging", "orders": "staging"},
            model_configs={
                "stg_orders": {"materialized": "table"},
                "orders": {"materialized": "table"},
            },
            model_queries={
                "stg_orders": "SELECT 1 AS order_id",
                "orders": "SELECT * FROM staging.stg_orders",
            },
            model_deps={"orders": ("stg_orders",)},
            full_refresh=False,
            select=("orders",),
            expected_action={},
            expected_reason={},
        )
    )
    project = project_with(
        project=project,
        settings=replace(
            project.settings,
            query_change_tracking=test_case.query_change_tracking,
        ),
    )
    planner_identity_work_info(reset=True)

    plan: PlanOutput = build_execution_plan_from_kwargs(
        project=project,
        adapter=adapter,
        connection=connection,
        select=("orders",),
        selection_diagnostics=test_case.selection_diagnostics,
    )

    identity_builds, stale_detections = planner_identity_work_info()
    assert stale_detections == test_case.expected_stale_detection_calls
    assert identity_builds == test_case.expected_identity_builds
    assert len(plan.warnings) == test_case.expected_warning_count


@pytest.mark.parametrize(
    "test_case",
    [
        BuildExecutionPlanTestCase(
            description="first run table model produces create_table action",
            setup_sql=(),
            model_locations={"orders": "staging"},
            model_configs={"orders": {"materialized": "table"}},
            model_queries={"orders": "SELECT 1 AS id"},
            full_refresh=False,
            expected_action={"orders": PlanAction.CREATE_TABLE},
            expected_reason={"orders": PlanReason.FIRST_RUN},
            expected_ddl_fragments={"orders": "CREATE OR REPLACE TABLE staging.orders AS"},
            expected_progress_fragments=(
                "Inspecting warehouse state...",
                "Inspected warehouse state. (",
                "Generating plan...",
                "Generated plan. (",
            ),
        ),
        BuildExecutionPlanTestCase(
            description="existing table with no change always rebuilds",
            setup_sql=("CREATE TABLE staging.orders AS SELECT 1 AS id",),
            model_locations={"orders": "staging"},
            model_configs={"orders": {"materialized": "table"}},
            model_queries={"orders": "SELECT 1 AS id"},
            full_refresh=False,
            expected_action={"orders": PlanAction.CREATE_TABLE},
            expected_reason={"orders": PlanReason.NO_CHANGE},
            expected_ddl_fragments={"orders": "CREATE OR REPLACE TABLE staging.orders AS"},
        ),
        BuildExecutionPlanTestCase(
            description="full refresh forces create_table on existing table",
            setup_sql=("CREATE TABLE staging.orders AS SELECT 1 AS id",),
            model_locations={"orders": "staging"},
            model_configs={"orders": {"materialized": "table"}},
            model_queries={"orders": "SELECT 1 AS id"},
            full_refresh=True,
            expected_action={"orders": PlanAction.CREATE_TABLE},
            expected_reason={"orders": PlanReason.FULL_REFRESH},
            expected_ddl_fragments={"orders": "CREATE OR REPLACE TABLE staging.orders AS"},
        ),
        BuildExecutionPlanTestCase(
            description="unset model full refresh follows absent CLI flag",
            setup_sql=("CREATE TABLE staging.orders AS SELECT 1 AS id",),
            model_locations={"orders": "staging"},
            model_configs={
                "orders": {"materialized": "incremental", "incremental_strategy": "append"}
            },
            model_queries={"orders": "SELECT 1 AS id"},
            full_refresh=False,
            expected_action={"orders": PlanAction.INCREMENTAL_APPEND},
            expected_reason={"orders": PlanReason.NORMAL_INCREMENTAL},
        ),
        BuildExecutionPlanTestCase(
            description="unset model full refresh follows CLI flag",
            setup_sql=("CREATE TABLE staging.orders AS SELECT 1 AS id",),
            model_locations={"orders": "staging"},
            model_configs={
                "orders": {"materialized": "incremental", "incremental_strategy": "append"}
            },
            model_queries={"orders": "SELECT 1 AS id"},
            full_refresh=True,
            expected_action={"orders": PlanAction.CREATE_TABLE},
            expected_reason={"orders": PlanReason.FULL_REFRESH},
        ),
        BuildExecutionPlanTestCase(
            description="false model full refresh stays incremental without CLI flag",
            setup_sql=("CREATE TABLE staging.orders AS SELECT 1 AS id",),
            model_locations={"orders": "staging"},
            model_configs={
                "orders": {
                    "materialized": "incremental",
                    "incremental_strategy": "append",
                    "full_refresh": False,
                }
            },
            model_queries={"orders": "SELECT 1 AS id"},
            full_refresh=False,
            expected_action={"orders": PlanAction.INCREMENTAL_APPEND},
            expected_reason={"orders": PlanReason.NORMAL_INCREMENTAL},
        ),
        BuildExecutionPlanTestCase(
            description="false model full refresh opts out of CLI flag",
            setup_sql=("CREATE TABLE staging.orders AS SELECT 1 AS id",),
            model_locations={"orders": "staging"},
            model_configs={
                "orders": {
                    "materialized": "incremental",
                    "incremental_strategy": "append",
                    "full_refresh": False,
                }
            },
            model_queries={"orders": "SELECT 1 AS id"},
            full_refresh=True,
            expected_action={"orders": PlanAction.INCREMENTAL_APPEND},
            expected_reason={"orders": PlanReason.NORMAL_INCREMENTAL},
        ),
        BuildExecutionPlanTestCase(
            description="true model full refresh forces without CLI flag",
            setup_sql=("CREATE TABLE staging.orders AS SELECT 1 AS id",),
            model_locations={"orders": "staging"},
            model_configs={
                "orders": {
                    "materialized": "incremental",
                    "incremental_strategy": "append",
                    "full_refresh": True,
                }
            },
            model_queries={"orders": "SELECT 1 AS id"},
            full_refresh=False,
            expected_action={"orders": PlanAction.CREATE_TABLE},
            expected_reason={"orders": PlanReason.FULL_REFRESH},
        ),
        BuildExecutionPlanTestCase(
            description="true model full refresh remains forced with CLI flag",
            setup_sql=("CREATE TABLE staging.orders AS SELECT 1 AS id",),
            model_locations={"orders": "staging"},
            model_configs={
                "orders": {
                    "materialized": "incremental",
                    "incremental_strategy": "append",
                    "full_refresh": True,
                }
            },
            model_queries={"orders": "SELECT 1 AS id"},
            full_refresh=True,
            expected_action={"orders": PlanAction.CREATE_TABLE},
            expected_reason={"orders": PlanReason.FULL_REFRESH},
        ),
        BuildExecutionPlanTestCase(
            description="mixed model full refresh values resolve independently",
            setup_sql=(
                "CREATE TABLE staging.default_orders AS SELECT 1 AS id",
                "CREATE TABLE staging.protected_orders AS SELECT 1 AS id",
                "CREATE TABLE staging.forced_orders AS SELECT 1 AS id",
            ),
            model_locations={
                "default_orders": "staging",
                "protected_orders": "staging",
                "forced_orders": "staging",
            },
            model_configs={
                "default_orders": {
                    "materialized": "incremental",
                    "incremental_strategy": "append",
                },
                "protected_orders": {
                    "materialized": "incremental",
                    "incremental_strategy": "append",
                    "full_refresh": False,
                },
                "forced_orders": {
                    "materialized": "incremental",
                    "incremental_strategy": "append",
                    "full_refresh": True,
                },
            },
            model_queries={
                "default_orders": "SELECT 1 AS id",
                "protected_orders": "SELECT 1 AS id",
                "forced_orders": "SELECT 1 AS id",
            },
            full_refresh=True,
            expected_action={
                "default_orders": PlanAction.CREATE_TABLE,
                "protected_orders": PlanAction.INCREMENTAL_APPEND,
                "forced_orders": PlanAction.CREATE_TABLE,
            },
            expected_reason={
                "default_orders": PlanReason.FULL_REFRESH,
                "protected_orders": PlanReason.NORMAL_INCREMENTAL,
                "forced_orders": PlanReason.FULL_REFRESH,
            },
        ),
        BuildExecutionPlanTestCase(
            description="forced upstream refresh does not cascade into opted out downstream",
            setup_sql=(
                "CREATE TABLE staging.upstream_orders AS SELECT 1 AS id",
                "CREATE TABLE staging.downstream_orders AS SELECT 1 AS id",
            ),
            model_locations={
                "upstream_orders": "staging",
                "downstream_orders": "staging",
            },
            model_configs={
                "upstream_orders": {
                    "materialized": "incremental",
                    "incremental_strategy": "append",
                    "full_refresh": True,
                },
                "downstream_orders": {
                    "materialized": "incremental",
                    "incremental_strategy": "append",
                    "full_refresh": False,
                },
            },
            model_queries={
                "upstream_orders": "SELECT 1 AS id",
                "downstream_orders": "SELECT 1 AS id",
            },
            model_deps={"downstream_orders": ("upstream_orders",)},
            full_refresh=True,
            expected_action={
                "upstream_orders": PlanAction.CREATE_TABLE,
                "downstream_orders": PlanAction.INCREMENTAL_APPEND,
            },
            expected_reason={
                "upstream_orders": PlanReason.FULL_REFRESH,
                "downstream_orders": PlanReason.NORMAL_INCREMENTAL,
            },
        ),
        BuildExecutionPlanTestCase(
            description="view model produces create_view action",
            setup_sql=(),
            model_locations={"orders_view": "staging"},
            model_configs={"orders_view": {"materialized": "view"}},
            model_queries={"orders_view": "SELECT 1 AS id"},
            full_refresh=False,
            expected_action={"orders_view": PlanAction.CREATE_VIEW},
            expected_reason={"orders_view": PlanReason.FIRST_RUN},
            expected_ddl_fragments={"orders_view": "CREATE OR REPLACE VIEW staging.orders_view AS"},
        ),
        BuildExecutionPlanTestCase(
            description="multiple models planned in correct order",
            setup_sql=(),
            model_locations={
                "stg_orders": "staging",
                "fact_orders": "staging",
            },
            model_configs={
                "stg_orders": {"materialized": "view"},
                "fact_orders": {"materialized": "table"},
            },
            model_queries={
                "stg_orders": "SELECT 1 AS id",
                "fact_orders": "SELECT 1 AS id",
            },
            full_refresh=False,
            expected_action={
                "stg_orders": PlanAction.CREATE_VIEW,
                "fact_orders": PlanAction.CREATE_TABLE,
            },
            expected_reason={
                "stg_orders": PlanReason.FIRST_RUN,
                "fact_orders": PlanReason.FIRST_RUN,
            },
        ),
        BuildExecutionPlanTestCase(
            description="seed appears in plan output seed entries",
            setup_sql=(),
            model_locations={"orders": "staging"},
            model_configs={"orders": {"materialized": "table"}},
            model_queries={"orders": "SELECT 1 AS id"},
            seed_locations={"country_codes": "staging"},
            full_refresh=False,
            expected_action={"orders": PlanAction.CREATE_TABLE},
            expected_reason={"orders": PlanReason.FIRST_RUN},
            expected_seed_names=("country_codes",),
        ),
        BuildExecutionPlanTestCase(
            description="select filters plan to selected models only",
            setup_sql=(),
            model_locations={
                "stg_orders": "staging",
                "fact_orders": "staging",
            },
            model_configs={
                "stg_orders": {"materialized": "view"},
                "fact_orders": {"materialized": "table"},
            },
            model_queries={
                "stg_orders": "SELECT 1 AS id",
                "fact_orders": "SELECT 1 AS id",
            },
            full_refresh=False,
            select=("stg_orders",),
            expected_action={"stg_orders": PlanAction.CREATE_VIEW},
            expected_reason={"stg_orders": PlanReason.FIRST_RUN},
            expected_model_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_when_building_plan_then_produces_expected_output(
    test_case: BuildExecutionPlanTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    sql: str
    for sql in test_case.setup_sql:
        connection.execute(sql)

    project: Any = build_project_from_test_case(test_case)

    progress_messages: list[str] = []
    plan: PlanOutput = build_execution_plan_from_kwargs(
        project=project,
        adapter=adapter,
        connection=connection,
        full_refresh=test_case.full_refresh,
        select=test_case.select,
        on_progress=progress_messages.append,
    )

    entry_map: dict[str, ModelPlanEntry] = {e.name: e for e in plan.model_entries}

    model_name: str
    expected_action: PlanAction
    for model_name, expected_action in test_case.expected_action.items():
        assert entry_map[model_name].action == expected_action

    expected_reason: PlanReason
    for model_name, expected_reason in test_case.expected_reason.items():
        assert entry_map[model_name].reason == expected_reason

    expected_fragment: str
    for model_name, expected_fragment in test_case.expected_ddl_fragments.items():
        assert expected_fragment in entry_map[model_name].logical_ddl

    seed_names: tuple[str, ...] = tuple(e.name for e in plan.seed_entries)
    expected_seed_name: str
    for expected_seed_name in test_case.expected_seed_names:
        assert expected_seed_name in seed_names

    expected_count: int = test_case.expected_model_count or len(test_case.expected_action)
    assert len(plan.model_entries) == expected_count

    progress_output: str = "\n".join(progress_messages)
    expected_progress_fragment: str
    for expected_progress_fragment in test_case.expected_progress_fragments:
        assert expected_progress_fragment in progress_output


@pytest.mark.parametrize(
    "test_case",
    [
        TableTypePlanAssemblyTestCase(
            description="selected view and table under permanent target default",
            expected_entry_names=("orders",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_view_and_table_with_target_default_when_assembling_plan_then_only_table_has_drift(
    test_case: TableTypePlanAssemblyTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    connection.execute("CREATE OR REPLACE VIEW staging.orders_view AS SELECT 1 AS id")
    connection.execute("CREATE OR REPLACE TABLE staging.orders AS SELECT 1 AS id")
    project: CompiledProjectView = build_project_from_test_case(
        BuildExecutionPlanTestCase(
            description=test_case.description,
            setup_sql=(),
            model_locations={"orders_view": "staging", "orders": "staging"},
            model_configs={
                "orders_view": {"materialized": "view"},
                "orders": {"materialized": "table"},
            },
            model_queries={"orders_view": "SELECT 1 AS id", "orders": "SELECT 1 AS id"},
            full_refresh=False,
            expected_action={},
            expected_reason={},
        )
    )
    view_model, table_model = project.models
    project = project_with(
        project=project,
        effective_target_name="test",
        models=(
            view_model,
            replace(
                compiled_model_record(table_model),
                config=replace(
                    table_model.config,
                    table_type=ResolvedTableType(
                        value=TableType.PERMANENT,
                        source=TableTypeSource.TARGET,
                        declared=True,
                    ),
                ),
            ),
        ),
    )
    monkeypatch.setattr(adapter, "adapter_name", BuiltinAdapter.SNOWFLAKE.value)

    plan: PlanOutput = build_execution_plan_from_kwargs(
        project=project,
        adapter=adapter,
        connection=connection,
        project_config=ProjectConfig(
            name="test",
            adapter=BuiltinAdapter.SNOWFLAKE.value,
            targets={"test": TargetConfig(default_table_type=TableType.PERMANENT)},
        ),
        local_config=LocalConfig(),
    )

    assert tuple(entry.model_name for entry in plan.table_type_entries) == (
        test_case.expected_entry_names
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SourceCursorInputPlanErrorTestCase(
            description="source cursor input column missing from warehouse metadata",
            setup_sql=(
                "CREATE SCHEMA raw",
                "CREATE TABLE raw.orders (order_id INTEGER, event_time TIMESTAMP)",
            ),
            model_name="orders_incremental",
            source_name="raw_orders",
            source_schema="raw",
            source_table="orders",
            cursor_column="event_time",
            cursor_input_column="loaded_at",
            expected_error_fragment=(
                "model 'orders_incremental': cursor_inputs references source 'raw_orders' "
                "column 'loaded_at'"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_source_cursor_input_column_when_building_plan_then_reports_planner_error(
    test_case: SourceCursorInputPlanErrorTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    sql: str
    for sql in test_case.setup_sql:
        connection.execute(sql)

    project: Any = build_project_from_source_cursor_input_test_case(test_case)

    plan: PlanOutput = build_execution_plan_from_kwargs(
        project=project,
        adapter=adapter,
        connection=connection,
        full_refresh=False,
    )

    errors: list[PlanWarning] = list(
        filter(None, (entry.planning_error for entry in plan.model_entries))
    )
    assert [error.model_name for error in errors] == [test_case.model_name]
    assert test_case.expected_error_fragment in errors[0].message


@pytest.mark.parametrize(
    "test_case",
    [
        BuildExecutionPlanTestCase(
            description="heuristic cursor type mismatch produces warning",
            setup_sql=("CREATE TABLE staging.events AS SELECT 1 AS event_id, 100 AS event_time",),
            model_locations={"events": "staging"},
            model_configs={
                "events": {
                    "materialized": "incremental",
                    "incremental_strategy": "delete_insert",
                    "cursor": "event_time",
                    "cursor_type": "timestamp",
                    "unique_key": "event_id",
                },
            },
            model_queries={"events": "SELECT 1 AS event_id, 100 AS event_time"},
            full_refresh=False,
            expected_action={"events": PlanAction.INCREMENTAL_DELETE_INSERT},
            expected_reason={"events": PlanReason.NORMAL_INCREMENTAL},
            expected_warning_count=1,
            expected_warning_severity=WarningSeverity.WARNING,
            expected_warning_fragment="appears to be integer",
            expected_progress_fragments=(
                "Inspecting cursor bounds (1/1): staging.events.event_time [max]...",
                "Inspected cursor bounds (1/1): staging.events.event_time [max] (",
                "Gathered cursor bounds (1/1 logical values; 1 physical relation reads). (",
            ),
        ),
        BuildExecutionPlanTestCase(
            description="sql_analysis cursor type mismatch produces error",
            setup_sql=("CREATE TABLE staging.events AS SELECT 1 AS event_id, 100 AS event_time",),
            model_locations={"events": "staging"},
            model_configs={
                "events": {
                    "materialized": "incremental",
                    "incremental_strategy": "delete_insert",
                    "cursor": "event_time",
                    "cursor_type": "timestamp",
                    "unique_key": "event_id",
                },
            },
            model_queries={"events": "SELECT 1 AS event_id, 100 AS event_time"},
            full_refresh=False,
            effective_connection={"sql_analysis": True},
            expected_action={"events": PlanAction.INCREMENTAL_DELETE_INSERT},
            expected_reason={"events": PlanReason.NORMAL_INCREMENTAL},
            expected_warning_count=1,
            expected_warning_severity=WarningSeverity.ERROR,
            expected_warning_fragment="which is integer",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cursor_type_mismatch_when_building_plan_then_produces_warning(
    test_case: BuildExecutionPlanTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    sql: str
    for sql in test_case.setup_sql:
        connection.execute(sql)

    project: Any = build_project_from_test_case(test_case)

    progress_messages: list[str] = []
    plan: PlanOutput = build_execution_plan_from_kwargs(
        project=project,
        adapter=adapter,
        connection=connection,
        full_refresh=test_case.full_refresh,
        on_progress=progress_messages.append,
    )

    entry_map: dict[str, ModelPlanEntry] = {e.name: e for e in plan.model_entries}

    model_name: str
    expected_action: PlanAction
    for model_name, expected_action in test_case.expected_action.items():
        assert entry_map[model_name].action == expected_action

    assert len(plan.warnings) == test_case.expected_warning_count
    warning: PlanWarning = plan.warnings[0]
    assert warning.severity == test_case.expected_warning_severity
    assert test_case.expected_warning_fragment is not None
    assert test_case.expected_warning_fragment in warning.message

    progress_output: str = "\n".join(progress_messages)
    expected_progress_fragment: str
    for expected_progress_fragment in test_case.expected_progress_fragments:
        assert expected_progress_fragment in progress_output


@pytest.mark.parametrize(
    "test_case",
    [
        BuildExecutionPlanTestCase(
            description="upstream first run does not replay existing downstream table",
            setup_sql=("CREATE TABLE staging.fact_orders AS SELECT 1 AS id",),
            model_locations={
                "stg_orders": "staging",
                "fact_orders": "staging",
            },
            model_configs={
                "stg_orders": {"materialized": "table"},
                "fact_orders": {"materialized": "table"},
            },
            model_queries={
                "stg_orders": "SELECT 1 AS id",
                "fact_orders": "SELECT 1 AS id",
            },
            model_deps={"fact_orders": ("stg_orders",)},
            full_refresh=False,
            expected_action={
                "stg_orders": PlanAction.CREATE_TABLE,
                "fact_orders": PlanAction.CREATE_TABLE,
            },
            expected_reason={
                "stg_orders": PlanReason.FIRST_RUN,
                "fact_orders": PlanReason.NO_CHANGE,
            },
            expected_backfill_action={"fact_orders": BackfillAction.FORWARD_ONLY},
        ),
        BuildExecutionPlanTestCase(
            description="upstream first run leaves existing incremental downstream incremental",
            setup_sql=("CREATE TABLE staging.fact_orders AS SELECT 1 AS id",),
            model_locations={
                "stg_orders": "staging",
                "fact_orders": "staging",
            },
            model_configs={
                "stg_orders": {"materialized": "table"},
                "fact_orders": {
                    "materialized": "incremental",
                    "incremental_strategy": "delete_insert",
                    "cursor": "id",
                    "cursor_type": "integer",
                    "unique_key": "id",
                },
            },
            model_queries={
                "stg_orders": "SELECT 1 AS id",
                "fact_orders": "SELECT 1 AS id",
            },
            model_deps={"fact_orders": ("stg_orders",)},
            full_refresh=False,
            expected_action={
                "stg_orders": PlanAction.CREATE_TABLE,
                "fact_orders": PlanAction.INCREMENTAL_DELETE_INSERT,
            },
            expected_reason={
                "stg_orders": PlanReason.FIRST_RUN,
                "fact_orders": PlanReason.NORMAL_INCREMENTAL,
            },
            expected_backfill_action={"fact_orders": BackfillAction.FORWARD_ONLY},
        ),
        BuildExecutionPlanTestCase(
            description="downstream replay policy applies only to its own change",
            setup_sql=(
                "CREATE TABLE staging.fact_orders AS SELECT 1 AS id",
                "CREATE TABLE staging.order_metrics AS SELECT 1 AS id",
            ),
            model_locations={
                "stg_orders": "staging",
                "fact_orders": "staging",
                "order_metrics": "staging",
            },
            model_configs={
                "stg_orders": {"materialized": "table"},
                "fact_orders": {
                    "materialized": "incremental",
                    "incremental_strategy": "delete_insert",
                    "cursor": "id",
                    "cursor_type": "integer",
                    "unique_key": "id",
                    "replay_on_change": "bounded-1d",
                },
                "order_metrics": {
                    "materialized": "incremental",
                    "incremental_strategy": "delete_insert",
                    "cursor": "id",
                    "cursor_type": "integer",
                    "unique_key": "id",
                },
            },
            model_queries={
                "stg_orders": "SELECT 1 AS id",
                "fact_orders": "SELECT id FROM stg_orders",
                "order_metrics": "SELECT id FROM fact_orders",
            },
            model_deps={
                "fact_orders": ("stg_orders",),
                "order_metrics": ("fact_orders",),
            },
            full_refresh=False,
            expected_action={
                "stg_orders": PlanAction.CREATE_TABLE,
                "fact_orders": PlanAction.INCREMENTAL_DELETE_INSERT,
                "order_metrics": PlanAction.INCREMENTAL_DELETE_INSERT,
            },
            expected_reason={
                "stg_orders": PlanReason.FIRST_RUN,
                "fact_orders": PlanReason.NORMAL_INCREMENTAL,
                "order_metrics": PlanReason.NORMAL_INCREMENTAL,
            },
            expected_backfill_action={
                "fact_orders": BackfillAction.FORWARD_ONLY,
                "order_metrics": BackfillAction.FORWARD_ONLY,
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_upstream_first_run_when_building_plan_then_downstream_keeps_own_backfill(
    test_case: BuildExecutionPlanTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    sql: str
    for sql in test_case.setup_sql:
        connection.execute(sql)
    write_previous_function_fingerprints(
        test_case=test_case,
        adapter=adapter,
        connection=connection,
    )

    project: Any = build_project_from_test_case(test_case)

    plan: PlanOutput = build_execution_plan_from_kwargs(
        project=project,
        adapter=adapter,
        connection=connection,
        full_refresh=test_case.full_refresh,
    )

    entry_map: dict[str, ModelPlanEntry] = {e.name: e for e in plan.model_entries}

    model_name: str
    expected_action: PlanAction
    for model_name, expected_action in test_case.expected_action.items():
        assert entry_map[model_name].action == expected_action

    expected_reason: PlanReason
    for model_name, expected_reason in test_case.expected_reason.items():
        assert entry_map[model_name].reason == expected_reason

    expected_fragment: str
    for model_name, expected_fragment in test_case.expected_ddl_fragments.items():
        assert expected_fragment in entry_map[model_name].logical_ddl

    expected_backfill: BackfillAction
    for model_name, expected_backfill in test_case.expected_backfill_action.items():
        assert entry_map[model_name].backfill.action == expected_backfill


@pytest.mark.parametrize(
    "test_case",
    [
        FormatPlanIntegrationTestCase(
            description="new project formats with first run group and seeds",
            setup_sql=(),
            model_locations={
                "stg_orders": "staging",
                "dim_customers": "staging",
            },
            model_configs={
                "stg_orders": {"materialized": "view"},
                "dim_customers": {"materialized": "table"},
            },
            model_queries={
                "stg_orders": "SELECT 1 AS id",
                "dim_customers": "SELECT 1 AS id",
            },
            seed_locations={"country_codes": "staging"},
            full_refresh=False,
            expected_format_fragments=(
                "Plan ready\033[0m  \033[2m3 selected",
                "\033[1mFirst run\033[0m \033[2m(2)\033[0m",
                "stg_orders",
                "view",
                "dim_customers",
                "table",
                "\033[1mSeeds\033[0m \033[2m(1)\033[0m",
                "country_codes",
            ),
            unexpected_format_fragments=("Normal", "Query changed", "Upstream changed"),
        ),
        FormatPlanIntegrationTestCase(
            description="upstream first run formats downstream table as routine work",
            setup_sql=("CREATE TABLE staging.fact_orders AS SELECT 1 AS id",),
            model_locations={
                "stg_orders": "staging",
                "fact_orders": "staging",
            },
            model_configs={
                "stg_orders": {"materialized": "table"},
                "fact_orders": {"materialized": "table"},
            },
            model_queries={
                "stg_orders": "SELECT 1 AS id",
                "fact_orders": "SELECT 1 AS id",
            },
            model_deps={"fact_orders": ("stg_orders",)},
            full_refresh=False,
            expected_format_fragments=(
                "Plan ready",
                "\033[1mFirst run\033[0m \033[2m(1)\033[0m",
                "stg_orders",
                "\033[1mModels\033[0m \033[2m(1)\033[0m",
                "fact_orders",
            ),
            unexpected_format_fragments=("Upstream changed", "full rebuild", "cause"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_real_plan_when_formatting_then_contains_expected_fragments(
    test_case: FormatPlanIntegrationTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    sql: str
    for sql in test_case.setup_sql:
        connection.execute(sql)

    project: Any = build_project_from_format_test_case(test_case)

    plan: PlanOutput = build_execution_plan_from_kwargs(
        project=project,
        adapter=adapter,
        connection=connection,
        full_refresh=test_case.full_refresh,
    )

    result: str = format_plan(plan=plan, full_refresh=test_case.full_refresh)

    fragment: str
    for fragment in test_case.expected_format_fragments:
        assert fragment in result, f"Expected '{fragment}' in output:\n{result}"
    for fragment in test_case.unexpected_format_fragments:
        assert fragment not in result, f"Did not expect '{fragment}' in output:\n{result}"


@pytest.mark.parametrize(
    "test_case",
    [
        FutureCursorPlannerTestCase(
            description="timestamp upstream is capped during real planning",
            warehouse_type="TIMESTAMP",
            minimum="2026-01-01 00:00:00",
            maximum="2500-01-01 00:00:00",
            expected_relation="raw.events",
        ),
        FutureCursorPlannerTestCase(
            description="date upstream is capped during real planning",
            warehouse_type="DATE",
            minimum="2026-01-01",
            maximum="2500-01-01",
            expected_relation="raw.events",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_future_upstream_when_building_real_plan_then_cap_has_structured_evidence(
    test_case: FutureCursorPlannerTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    project: CompiledProjectView = build_future_cursor_project()
    connection.execute("CREATE SCHEMA IF NOT EXISTS raw")
    connection.execute("CREATE SCHEMA IF NOT EXISTS staging")
    connection.execute(f"CREATE TABLE raw.events (occurred_at {test_case.warehouse_type})")
    connection.execute(
        "INSERT INTO raw.events VALUES (CAST(? AS "
        f"{test_case.warehouse_type})), (CAST(? AS {test_case.warehouse_type}))",
        [test_case.minimum, test_case.maximum],
    )

    plan: PlanOutput = build_execution_plan_from_kwargs(
        project=project,
        adapter=adapter,
        connection=connection,
        project_config=ProjectConfig(
            name="future_cursor",
            adapter="duckdb",
            cursors=CursorsConfig(future=FutureCursorsConfig("1d", FutureCursorAction.CAP)),
        ),
    )

    bounds: CursorBounds | None = plan.model_entries[0].cursor_bounds
    assert bounds is not None
    assert bounds.future_safety is not None
    assert bounds.future_safety.action == "cap"
    assert bounds.future_safety.inputs[0].relation == test_case.expected_relation
    assert bounds.future_safety.inputs[0].cursor_column == "occurred_at"
    assert bounds.future_safety.determining_relation == "raw.events"


@pytest.mark.parametrize(
    "test_case",
    [
        FutureCursorPlannerErrorTestCase(
            "future planner error", "future cursor safety limit exceeded"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_future_upstream_and_error_policy_when_building_real_plan_then_fails_closed(
    test_case: FutureCursorPlannerErrorTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    project: CompiledProjectView = build_future_cursor_project()
    connection.execute("CREATE SCHEMA IF NOT EXISTS raw")
    connection.execute("CREATE SCHEMA IF NOT EXISTS staging")
    connection.execute("CREATE TABLE raw.events (occurred_at TIMESTAMP)")
    connection.execute("INSERT INTO raw.events VALUES (TIMESTAMP '2500-01-01')")

    plan: PlanOutput = build_execution_plan_from_kwargs(
        project=project,
        adapter=adapter,
        connection=connection,
        project_config=ProjectConfig(
            name="future_cursor",
            adapter="duckdb",
            cursors=CursorsConfig(future=FutureCursorsConfig("1d", FutureCursorAction.ERROR)),
        ),
    )

    errors: list[PlanWarning] = list(
        filter(None, (entry.planning_error for entry in plan.model_entries))
    )
    assert re.search(test_case.expected_error_fragment, errors[0].message), errors
