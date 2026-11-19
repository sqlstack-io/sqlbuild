"""Tests for plan output formatting."""

from __future__ import annotations

import json

import pytest

from sqlbuild._native import (
    BackfillAction,
    ColumnRenameHint,
    CursorBounds,
    CursorInputRelation,
    DisplayOptions,
    GraphResourceKind,
    MaterializationType,
    PlanAction,
    PlanOutput,
    PlanReason,
    PythonIdentityStatus,
    PythonPlanEntry,
    PythonRunPhase,
    SchemaChangeKind,
    SourceWriteStrategy,
    WarningSeverity,
    format_plan,
)
from sqlbuild.compiler.compile.types import FunctionLanguage
from sqlbuild.compiler.python_nodes.types import PythonNodeKind
from tests.unit.src.sqlbuild.cli.output.main.plan._test_types import (
    FormatPlanColorTestCase,
    FormatPlanTestCase,
    RuntimeBoundsLineTestCase,
)
from tests.unit.src.sqlbuild.cli.output.main.plan.helpers import (
    build_discovered_provider_usage,
    build_function_entry,
    build_model_entry,
    build_plan_output,
    build_plan_provider_usage,
    build_schema_finding,
    build_seed_entry,
    build_source_load_entry,
    build_table_type_entry,
    build_warning,
)


@pytest.mark.parametrize(
    "test_case",
    [
        FormatPlanTestCase(
            description="changes-only pruned models are visible as current skips",
            plan_output=build_plan_output(
                metadata={"direct_pruned_model_names": ("customer_revenue_check",)}
            ),
            expected_fragments=(
                "Plan ready  0 selected",
                "Skipped current models (1 already up to date)",
            ),
            unexpected_fragments=("customer_revenue_check", "changes-only", "Execution"),
        ),
        FormatPlanTestCase(
            description="verbose changes-only pruned models show current model names",
            plan_output=build_plan_output(
                metadata={"direct_pruned_model_names": ("customer_revenue_check",)}
            ),
            display_options=DisplayOptions(max_entries_per_section=None),
            expected_fragments=(
                "Skipped current models (1 already up to date)",
                "customer_revenue_check",
                "up to date",
            ),
            unexpected_fragments=("changes-only",),
        ),
        FormatPlanTestCase(
            description="source freshness age warnings and errors are visible",
            plan_output=build_plan_output(
                metadata={
                    "direct_source_freshness": {
                        "observed_source_names": ("raw.orders", "raw.payments"),
                        "changed_source_names": (),
                        "unchanged_source_names": ("raw.orders",),
                        "unknown_source_names": (),
                        "age_warning_source_names": ("raw.payments",),
                        "age_error_source_names": ("raw.orders",),
                        "stale_model_names": (),
                        "blocked_model_names": ("fact_orders",),
                    }
                }
            ),
            expected_fragments=(
                "Source freshness",
                "age warnings:",
                "raw.payments",
                "age errors:",
                "raw.orders",
                "source-blocked models:",
                "fact_orders",
            ),
        ),
        FormatPlanTestCase(
            description="direct non-changes-only output hides freshness diagnostics only",
            plan_output=build_plan_output(
                source_load_entries=(build_source_load_entry(name="raw_orders"),),
                warnings=(
                    build_warning(
                        model_name=None,
                        message=(
                            "Stale inputs detected\n\n  Affected selected models:\n    fact_orders"
                        ),
                        code="S302",
                    ),
                    build_warning(
                        model_name="fact_orders",
                        message="schema change requires rebuild",
                    ),
                ),
                metadata={
                    "direct_source_freshness": {
                        "observed_source_names": ("raw_orders",),
                        "changed_source_names": ("raw_orders",),
                        "unchanged_source_names": (),
                        "unknown_source_names": (),
                        "stale_model_names": ("fact_orders",),
                    }
                },
            ),
            include_direct_freshness_diagnostics=False,
            expected_fragments=(
                "Sources to load (1)",
                "raw_orders",
                "Warnings (1)",
                "schema change requires rebuild",
            ),
            unexpected_fragments=(
                "Source freshness",
                "Stale inputs detected",
                "Warnings (2)",
            ),
        ),
        FormatPlanTestCase(
            description="routine models section shows names with strategy and cursor type",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="stg_orders",
                        action=PlanAction.CREATE_VIEW,
                        reason=PlanReason.NO_CHANGE,
                        materialization_type=MaterializationType.VIEW,
                    ),
                    build_model_entry(
                        name="fact_orders",
                        action=PlanAction.INCREMENTAL_DELETE_INSERT,
                        reason=PlanReason.NORMAL_INCREMENTAL,
                        materialization_type=MaterializationType.INCREMENTAL,
                        incremental_strategy="delete_insert",
                        cursor_type="timestamp",
                    ),
                    build_model_entry(
                        name="fact_events",
                        action=PlanAction.INCREMENTAL_DELETE_INSERT,
                        reason=PlanReason.NORMAL_INCREMENTAL,
                        materialization_type=MaterializationType.INCREMENTAL,
                        incremental_strategy="delete_insert",
                        cursor_type="integer",
                        incremental_mode="microbatch",
                    ),
                ),
            ),
            expected_fragments=(
                "Models (3)",
                "stg_orders",
                "fact_orders",
                "fact_events",
                "delete_insert (timestamp)",
                "delete_insert (integer, microbatch)",
                "view",
            ),
            unexpected_fragments=("Normal",),
        ),
        FormatPlanTestCase(
            description="planner-resolved microbatch shows requested and effective work",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="stg_events",
                        action=PlanAction.CREATE_TABLE,
                        reason=PlanReason.FIRST_RUN,
                        materialization_type=MaterializationType.INCREMENTAL,
                        incremental_strategy="delete_insert",
                        incremental_mode="microbatch",
                        cursor_column="event_date",
                        cursor_type="timestamp",
                        cursor_grain="day",
                        batch_size="1mo",
                        microbatch_range=CursorBounds(
                            start="2014-01-01",
                            end="2014-04-01",
                        ),
                        start_cursor_override="2014-01-01",
                        end_cursor_override="2014-03-31",
                    ),
                ),
            ),
            expected_fragments=(
                "cursor  event_date (timestamp)",
                "requested  2014-01-01 -> 2014-03-31",
                "range  2014-01-01 \u2192 2014-03-31",
                "grain  day",
                "batch size  1mo",
                "batches  3 x 1mo",
                "bounds  planner-resolved",
            ),
            unexpected_fragments=("at run time",),
        ),
        FormatPlanTestCase(
            description="runtime-owned microbatch marks range and count as deferred",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="clean_events",
                        action=PlanAction.CREATE_TABLE,
                        reason=PlanReason.FIRST_RUN,
                        materialization_type=MaterializationType.INCREMENTAL,
                        incremental_strategy="delete_insert",
                        incremental_mode="microbatch",
                        cursor_column="event_date",
                        cursor_type="timestamp",
                        cursor_grain="day",
                        batch_size="effective",
                        cursor_input_relations=(
                            CursorInputRelation(
                                relation="stg_events",
                                cursor_column="event_date",
                                cursor_grain="month",
                                is_model_backed=True,
                                is_runtime_produced=True,
                                input_name="stg_events",
                                input_kind=GraphResourceKind.MODEL,
                            ),
                        ),
                    ),
                ),
            ),
            expected_fragments=(
                "grain  day -> month (effective)",
                "batch size  effective -> 1mo",
                "batches  counted at run time, once model stg_events is up to date",
                "bounds  computed at run time, once model stg_events is up to date",
            ),
            unexpected_fragments=("batches  3 x", "bounds  planner-resolved"),
        ),
        FormatPlanTestCase(
            description="native effective batch size is shown without resolution noise",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="daily_events",
                        action=PlanAction.INCREMENTAL_DELETE_INSERT,
                        reason=PlanReason.NORMAL_INCREMENTAL,
                        materialization_type=MaterializationType.INCREMENTAL,
                        incremental_strategy="delete_insert",
                        incremental_mode="microbatch",
                        cursor_column="event_date",
                        cursor_type="timestamp",
                        cursor_grain="day",
                        batch_size="effective",
                        microbatch_range=CursorBounds(
                            start="2026-01-01",
                            end="2026-01-03",
                        ),
                    ),
                ),
            ),
            expected_fragments=(
                "grain  day",
                "batch size  1d",
                "batches  2 x 1d",
            ),
            unexpected_fragments=("batch size  effective ->",),
        ),
        FormatPlanTestCase(
            description="human plan output omits identity hashes",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="fact_orders",
                        action=PlanAction.INCREMENTAL_DELETE_INSERT,
                        reason=PlanReason.QUERY_CHANGED,
                        materialization_type=MaterializationType.INCREMENTAL,
                        fingerprint_version_hash="expected_hash",
                        previous_version_hash="built_hash",
                    ),
                ),
            ),
            expected_fragments=("Query changed (1)", "fact_orders"),
            unexpected_fragments=("expected_hash", "built_hash", "version_hash"),
        ),
        FormatPlanTestCase(
            description="first run shows materialization label with strategy and microbatch",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="stg_orders",
                        action=PlanAction.CREATE_VIEW,
                        reason=PlanReason.FIRST_RUN,
                        materialization_type=MaterializationType.VIEW,
                    ),
                    build_model_entry(
                        name="fact_orders",
                        action=PlanAction.CREATE_TABLE,
                        reason=PlanReason.FIRST_RUN,
                        materialization_type=MaterializationType.INCREMENTAL,
                        incremental_strategy="delete_insert",
                        cursor_type="timestamp",
                        incremental_mode="microbatch",
                    ),
                ),
            ),
            expected_fragments=(
                "First run (2)",
                "stg_orders",
                "view",
                "fact_orders",
                "delete_insert (timestamp, microbatch)",
            ),
        ),
        FormatPlanTestCase(
            description="snapshot models show strategy and historical shape labels",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="customer_current_snapshot",
                        action=PlanAction.SNAPSHOT,
                        reason=PlanReason.FIRST_RUN,
                        materialization_type=MaterializationType.SNAPSHOT,
                        snapshot_strategy="timestamp",
                    ),
                    build_model_entry(
                        name="customer_daily_snapshot",
                        action=PlanAction.SNAPSHOT,
                        reason=PlanReason.FIRST_RUN,
                        materialization_type=MaterializationType.SNAPSHOT,
                        snapshot_strategy="check",
                        observed_at_column="observed_at",
                        historical_input="snapshot",
                    ),
                    build_model_entry(
                        name="customer_changes_snapshot",
                        action=PlanAction.SNAPSHOT,
                        reason=PlanReason.FIRST_RUN,
                        materialization_type=MaterializationType.SNAPSHOT,
                        snapshot_strategy="timestamp",
                        observed_at_column="loaded_at",
                        historical_input="changes",
                    ),
                ),
            ),
            expected_fragments=(
                "First run (3)",
                "customer_current_snapshot",
                "snapshot (timestamp)",
                "customer_daily_snapshot",
                "snapshot (check, historical snapshot)",
                "customer_changes_snapshot",
                "snapshot (timestamp, historical changes)",
            ),
        ),
        FormatPlanTestCase(
            description="query changed shows action policy cursor and mode",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="fact_orders",
                        action=PlanAction.INCREMENTAL_DELETE_INSERT,
                        reason=PlanReason.QUERY_CHANGED,
                        materialization_type=MaterializationType.INCREMENTAL,
                        backfill_action=BackfillAction.BOUNDED,
                        backfill_duration="30d",
                        cursor_column="event_time",
                        cursor_type="timestamp",
                        incremental_mode="microbatch",
                        cursor_bounds=CursorBounds(start="2026-03-26", end="2026-04-25"),
                        previous_query_sql="SELECT order_id FROM raw",
                        query_changed=True,
                    ),
                ),
            ),
            expected_fragments=(
                "Query changed (1)",
                "fact_orders",
                "rebuild last 30d",
                "cursor  event_time",
                "mode  microbatch",
                "2026-03-26",
                "2026-04-24",
                "policy  replay_on_change=bounded-30d",
                "query diff:",
            ),
        ),
        FormatPlanTestCase(
            description="schema changed shows schema diff and policy",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="dim_customers",
                        action=PlanAction.CREATE_TABLE,
                        reason=PlanReason.SCHEMA_CHANGED,
                        backfill_action=BackfillAction.BOUNDED,
                        backfill_duration="7d",
                        schema_findings=(
                            build_schema_finding(
                                kind=SchemaChangeKind.COLUMN_ADDED,
                                column_name="discount",
                                expected_type="FLOAT",
                            ),
                        ),
                    ),
                ),
            ),
            expected_fragments=(
                "Schema changed (1)",
                "dim_customers",
                "rebuild last 7d, add column",
                "schema diff:",
                "+ discount",
                "added",
            ),
        ),
        FormatPlanTestCase(
            description="near-match rename hint follows the added column",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="fct_orders",
                        action=PlanAction.INCREMENTAL_APPEND,
                        reason=PlanReason.QUERY_CHANGED,
                        materialization_type=MaterializationType.INCREMENTAL,
                        backfill_action=BackfillAction.FORWARD_ONLY,
                        schema_findings=(
                            build_schema_finding(
                                kind=SchemaChangeKind.COLUMN_ADDED, column_name="revenue"
                            ),
                            build_schema_finding(
                                kind=SchemaChangeKind.COLUMN_REMOVED, column_name="amount"
                            ),
                        ),
                        column_rename_hints=(
                            ColumnRenameHint(
                                model_name="fct_orders",
                                added_column="revenue",
                                candidate_columns=("amount",),
                            ),
                        ),
                    ),
                ),
            ),
            expected_fragments=(
                "+ revenue   (added)\n        similar to amount; if this is a rename, add "
                "revenue (migrate_from amount)\n      - amount   (removed)",
            ),
        ),
        FormatPlanTestCase(
            description="config changed shows config diff without query diff",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="fact_orders",
                        action=PlanAction.CREATE_TABLE,
                        reason=PlanReason.CONFIG_CHANGED,
                        previous_query_sql="SELECT order_id FROM raw",
                        previous_metadata_json=(
                            '{"config":{"materialized":"view"},"model_name":"fact_orders"}'
                        ),
                        fingerprint_metadata_json=(
                            '{"config":{"materialized":"table"},"model_name":"fact_orders"}'
                        ),
                        config_changed=True,
                    ),
                ),
            ),
            expected_fragments=(
                "Config changed (1)",
                "fact_orders",
                "config diff:",
                '"materialized": "view"',
                '"materialized": "table"',
            ),
            unexpected_fragments=("query diff:",),
        ),
        FormatPlanTestCase(
            description="query and config changes show both diffs",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="fact_orders",
                        action=PlanAction.CREATE_TABLE,
                        reason=PlanReason.QUERY_CHANGED,
                        previous_query_sql="SELECT order_id FROM raw",
                        previous_metadata_json=(
                            '{"config":{"materialized":"view"},"model_name":"fact_orders"}'
                        ),
                        fingerprint_metadata_json=(
                            '{"config":{"materialized":"table"},"model_name":"fact_orders"}'
                        ),
                        query_changed=True,
                        config_changed=True,
                    ),
                ),
            ),
            expected_fragments=(
                "Query changed (1)",
                "fact_orders",
                "query diff:",
                "config diff:",
                '"materialized": "view"',
                '"materialized": "table"',
            ),
        ),
        FormatPlanTestCase(
            description="full rebuild hides cursor range placeholders",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="hourly_order_activity",
                        action=PlanAction.CREATE_TABLE,
                        reason=PlanReason.QUERY_CHANGED,
                        materialization_type=MaterializationType.INCREMENTAL,
                        backfill_action=BackfillAction.FULL,
                        cursor_column="activity_hour",
                        cursor_type="timestamp",
                        incremental_mode="microbatch",
                        cursor_bounds=CursorBounds(
                            start="__SQB_CURSOR_START__",
                            end="__SQB_CURSOR_END__",
                        ),
                        previous_query_sql="SELECT activity_hour FROM raw",
                    ),
                ),
            ),
            expected_fragments=(
                "hourly_order_activity",
                "full rebuild",
                "cursor  activity_hour",
                "mode  microbatch",
            ),
            unexpected_fragments=(
                "range:",
                "__SQB_CURSOR_START__",
                "__SQB_CURSOR_END__",
            ),
        ),
        FormatPlanTestCase(
            description="direct caller of a changed function shows function changed cause",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="hourly_order_activity",
                        action=PlanAction.INCREMENTAL_DELETE_INSERT,
                        reason=PlanReason.FUNCTION_CHANGED,
                        materialization_type=MaterializationType.INCREMENTAL,
                        incremental_strategy="delete_insert",
                        backfill_action=BackfillAction.FORWARD_ONLY,
                        changed_functions=("is_completed_order",),
                    ),
                ),
            ),
            expected_fragments=(
                "Function changed (1)",
                "hourly_order_activity",
                "continue forward",
                "cause  function is_completed_order changed",
            ),
            unexpected_fragments=("Upstream changed", "Normal"),
        ),
        FormatPlanTestCase(
            description="seeds section shows seed names",
            plan_output=build_plan_output(
                model_entries=(build_model_entry(name="orders", action=PlanAction.CREATE_TABLE),),
                seed_entries=(build_seed_entry(name="country_codes"),),
            ),
            expected_fragments=(
                "Seeds (1)",
                "country_codes",
                "first_run",
            ),
        ),
        FormatPlanTestCase(
            description="seeds section shows changed seed reason",
            plan_output=build_plan_output(
                model_entries=(build_model_entry(name="orders", action=PlanAction.CREATE_TABLE),),
                seed_entries=(
                    build_seed_entry(name="country_codes", reason=PlanReason.CONFIG_CHANGED),
                ),
            ),
            expected_fragments=(
                "Seeds (1)",
                "country_codes  (seed_changed)",
            ),
        ),
        FormatPlanTestCase(
            description="functions section shows names and udf language",
            plan_output=build_plan_output(
                model_entries=(build_model_entry(name="orders", action=PlanAction.CREATE_TABLE),),
                function_entries=(
                    build_function_entry(name="is_completed_order", language=FunctionLanguage.SQL),
                    build_function_entry(
                        name="is_completed_order_py", language=FunctionLanguage.PYTHON
                    ),
                ),
            ),
            expected_fragments=(
                "Plan ready  3 selected",
                "Functions (2)",
                "is_completed_order",
                "sql udf",
                "is_completed_order_py",
                "python udf",
            ),
        ),
        FormatPlanTestCase(
            description="header includes source loads when sources will load",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(name="stg_orders", action=PlanAction.CREATE_TABLE),
                ),
                source_load_entries=(build_source_load_entry(name="raw_orders"),),
            ),
            expected_fragments=(
                "Plan ready  1 selected, 1 source to load",
                "Sources to load (1)",
                "raw_orders",
            ),
        ),
        FormatPlanTestCase(
            description="plan shows python lifecycle sections",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(name="fact_orders", action=PlanAction.CREATE_TABLE),
                ),
                source_load_entries=(build_source_load_entry(name="raw_orders"),),
            ),
            python_plan_entries=(
                PythonPlanEntry(
                    name="prepare_orders",
                    kind=PythonNodeKind.ASSET,
                    phase=PythonRunPhase.PRE_SQL_INGRESS,
                ),
                PythonPlanEntry(
                    name="publish_orders",
                    kind=PythonNodeKind.ASSET,
                    phase=PythonRunPhase.PRE_SQL_INGRESS,
                ),
                PythonPlanEntry(
                    name="profile_fact_orders",
                    kind=PythonNodeKind.ASSET,
                    phase=PythonRunPhase.READ_SIDE,
                ),
            ),
            expected_fragments=(
                "Plan ready  1 selected, 1 source to load, 3 Python nodes",
                "Python ingress (2)",
                "prepare_orders",
                "task",
                "publish_orders",
                "asset",
                "Python read-side (1)",
                "profile_fact_orders",
            ),
            expected_ordered_fragments=(
                "Python ingress (2)",
                "Sources to load (1)",
                "First run (1)",
                "Python read-side (1)",
            ),
        ),
        FormatPlanTestCase(
            description="changed python identity shows source and dependency diffs",
            plan_output=build_plan_output(),
            python_plan_entries=(
                PythonPlanEntry(
                    name="prepare_orders",
                    kind=PythonNodeKind.ASSET,
                    phase=PythonRunPhase.PRE_SQL_INGRESS,
                    identity_status=PythonIdentityStatus.CHANGED,
                    previous_definition_json=json.dumps(
                        {"source_text": "def prepare_orders(ctx):\n    return 1\n"},
                        sort_keys=True,
                    ),
                    current_definition_json=json.dumps(
                        {"source_text": "def prepare_orders(ctx):\n    return 2\n"},
                        sort_keys=True,
                    ),
                    previous_metadata_json=json.dumps(
                        {
                            "dependencies": [
                                {
                                    "module": "python.tasks.helpers",
                                    "qualname": "order_label",
                                    "source_path": "python/tasks/helpers.py",
                                    "source_text": "def order_label():\n    return 'old'\n",
                                }
                            ]
                        },
                        sort_keys=True,
                    ),
                    current_metadata_json=json.dumps(
                        {
                            "dependencies": [
                                {
                                    "module": "python.tasks.helpers",
                                    "qualname": "order_label",
                                    "source_path": "python/tasks/helpers.py",
                                    "source_text": "def order_label():\n    return 'new'\n",
                                }
                            ]
                        },
                        sort_keys=True,
                    ),
                ),
            ),
            expected_fragments=(
                "Python ingress (1)",
                "prepare_orders",
                "task (changed)",
                "python diff:",
                "source diff:",
                "dependency diff:",
                "-    return 1",
                "+    return 2",
                "python/tasks/helpers.py :: python.tasks.helpers :: order_label",
                "-    return 'old'",
                "+    return 'new'",
            ),
        ),
        FormatPlanTestCase(
            description="header includes source reloads when sources will reload",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(name="stg_orders", action=PlanAction.CREATE_TABLE),
                ),
                source_load_entries=(build_source_load_entry(name="raw_orders", is_reload=True),),
            ),
            full_refresh=True,
            expected_fragments=(
                "Plan ready  1 selected, 1 source to reload",
                "Sources to reload (1)",
                "raw_orders",
            ),
        ),
        FormatPlanTestCase(
            description="source load section formats strategy details",
            plan_output=build_plan_output(
                source_load_entries=(
                    build_source_load_entry(
                        name="raw_events",
                        write_strategy=SourceWriteStrategy.DELETE_INSERT,
                        cursor_column="event_at",
                    ),
                    build_source_load_entry(
                        name="raw_orders",
                        write_strategy=SourceWriteStrategy.MERGE,
                        unique_key=("order_id", "updated_at"),
                    ),
                    build_source_load_entry(name="raw_self_managed", write_strategy=None),
                ),
            ),
            expected_fragments=(
                "Plan ready  0 selected, 3 sources to load",
                "raw_events",
                "delete_insert (cursor: event_at)",
                "raw_orders",
                "merge (unique_key: order_id, updated_at)",
                "raw_self_managed",
                "self-managed",
            ),
        ),
        FormatPlanTestCase(
            description="function query change shows diff without replay policy",
            plan_output=build_plan_output(
                model_entries=(build_model_entry(name="orders", action=PlanAction.CREATE_TABLE),),
                function_entries=(
                    build_function_entry(
                        name="is_completed_order",
                        language=FunctionLanguage.SQL,
                        reason=PlanReason.QUERY_CHANGED,
                        previous_query_sql="returns=BOOLEAN\nbody=\norder_status = 'completed'",
                    ),
                ),
            ),
            expected_fragments=(
                "Changed functions (1)",
                "is_completed_order",
                "sql udf",
                "query diff:",
                "--- previous",
                "+++ current",
            ),
        ),
        FormatPlanTestCase(
            description="warnings section shows warning messages",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="orders",
                        action=PlanAction.SKIP,
                        reason=PlanReason.NO_CHANGE,
                        backfill_action=BackfillAction.FORWARD_ONLY,
                    ),
                ),
                warnings=(
                    build_warning(
                        model_name="stg_customers",
                        message="type change detected",
                        severity=WarningSeverity.WARNING,
                    ),
                ),
            ),
            expected_fragments=(
                "Warnings (1)",
                "stg_customers",
                "type change detected",
            ),
        ),
        FormatPlanTestCase(
            description="error-severity diagnostics render under an errors heading",
            plan_output=build_plan_output(
                warnings=(
                    build_warning(
                        model_name="stg_customer_orders",
                        message="migrate_from origin main.stg_orders does not exist",
                        severity=WarningSeverity.ERROR,
                        code="M102",
                    ),
                    build_warning(
                        model_name="stg_customers",
                        message="type change detected",
                        severity=WarningSeverity.WARNING,
                    ),
                ),
            ),
            expected_fragments=(
                "Errors (1)\n└── stg_customer_orders\n"
                "    └── migrate_from origin main.stg_orders does not exist",
                "Warnings (1)\n└── stg_customers\n    └── type change detected",
            ),
            unexpected_fragments=("Warnings (2)",),
        ),
        FormatPlanTestCase(
            description="multiline warning messages terminate their child tree",
            plan_output=build_plan_output(
                warnings=(
                    build_warning(
                        model_name="stg_customers",
                        message="type change detected\nrebuild required",
                        severity=WarningSeverity.WARNING,
                    ),
                ),
            ),
            expected_fragments=(
                "├── type change detected",
                "└── rebuild required",
            ),
        ),
        FormatPlanTestCase(
            description="full refresh shows aggregate counts with incremental detail",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="stg_orders",
                        action=PlanAction.CREATE_VIEW,
                        reason=PlanReason.FULL_REFRESH,
                        materialization_type=MaterializationType.VIEW,
                    ),
                    build_model_entry(
                        name="dim_customers",
                        action=PlanAction.CREATE_TABLE,
                        reason=PlanReason.FULL_REFRESH,
                    ),
                    build_model_entry(
                        name="fact_orders",
                        action=PlanAction.CREATE_TABLE,
                        reason=PlanReason.FULL_REFRESH,
                        materialization_type=MaterializationType.INCREMENTAL,
                        incremental_strategy="delete_insert",
                        cursor_type="timestamp",
                        incremental_mode="microbatch",
                    ),
                ),
            ),
            full_refresh=True,
            expected_fragments=(
                "Plan ready  full refresh, 3 selected",
                "Full refresh (3)",
                "view",
                "table",
                "delete_insert (timestamp, microbatch)",
            ),
            unexpected_fragments=("Normal", "Query changed", "First run"),
        ),
        FormatPlanTestCase(
            description="mixed plan shows full refresh alongside incremental work",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="rebuilt_orders",
                        action=PlanAction.CREATE_TABLE,
                        reason=PlanReason.FULL_REFRESH,
                        materialization_type=MaterializationType.INCREMENTAL,
                        incremental_strategy="delete_insert",
                        cursor_type="timestamp",
                    ),
                    build_model_entry(
                        name="appended_orders",
                        action=PlanAction.INCREMENTAL_APPEND,
                        reason=PlanReason.NORMAL_INCREMENTAL,
                        materialization_type=MaterializationType.INCREMENTAL,
                        incremental_strategy="append",
                        cursor_type="integer",
                        backfill_action=BackfillAction.FORWARD_ONLY,
                    ),
                    build_model_entry(
                        name="microbatched_orders",
                        action=PlanAction.INCREMENTAL_DELETE_INSERT,
                        reason=PlanReason.QUERY_CHANGED,
                        materialization_type=MaterializationType.INCREMENTAL,
                        incremental_strategy="delete_insert",
                        incremental_mode="microbatch",
                        cursor_type="timestamp",
                        backfill_action=BackfillAction.BOUNDED,
                        backfill_duration="7d",
                    ),
                ),
            ),
            full_refresh=True,
            expected_fragments=(
                "Plan ready  3 selected",
                "Full refresh (1)",
                "rebuilt_orders",
                "full rebuild",
                "Query changed (1)",
                "microbatched_orders",
                "Models (1)",
                "appended_orders",
            ),
            unexpected_fragments=("Plan ready  full refresh",),
        ),
        FormatPlanTestCase(
            description="empty plan shows only header and selected zero",
            plan_output=build_plan_output(),
            expected_fragments=("Plan ready  0 selected",),
            unexpected_fragments=("Normal", "Seeds", "Warnings"),
        ),
        FormatPlanTestCase(
            description="non-microbatch incremental omits mode line",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="fact_orders",
                        action=PlanAction.INCREMENTAL_DELETE_INSERT,
                        reason=PlanReason.QUERY_CHANGED,
                        materialization_type=MaterializationType.INCREMENTAL,
                        backfill_action=BackfillAction.BOUNDED,
                        backfill_duration="30d",
                        cursor_column="event_time",
                        cursor_type="timestamp",
                    ),
                ),
            ),
            expected_fragments=("cursor  event_time",),
            unexpected_fragments=("mode:",),
        ),
        FormatPlanTestCase(
            description="custom materialization shows name with custom suffix in normal section",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="fact_orders",
                        action=PlanAction.CUSTOM,
                        reason=PlanReason.NO_CHANGE,
                        materialization_type=MaterializationType.CUSTOM,
                        custom_materialization_name="partition_tracked",
                    ),
                ),
            ),
            expected_fragments=("partition_tracked (custom)",),
        ),
        FormatPlanTestCase(
            description="custom materialization shows name with custom suffix in first run section",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="fact_orders",
                        action=PlanAction.CUSTOM,
                        reason=PlanReason.FIRST_RUN,
                        materialization_type=MaterializationType.CUSTOM,
                        custom_materialization_name="atomic_swap",
                    ),
                ),
            ),
            expected_fragments=("atomic_swap (custom)",),
        ),
        FormatPlanTestCase(
            description="changed sections appear before routine resource sections",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="fact_orders",
                        action=PlanAction.CREATE_TABLE,
                        reason=PlanReason.QUERY_CHANGED,
                        backfill_action=BackfillAction.FULL,
                        previous_query_sql="SELECT order_id FROM raw",
                    ),
                    build_model_entry(
                        name="stg_orders",
                        action=PlanAction.CREATE_VIEW,
                        reason=PlanReason.NO_CHANGE,
                        materialization_type=MaterializationType.VIEW,
                    ),
                ),
                function_entries=(
                    build_function_entry(name="is_completed_order"),
                    build_function_entry(
                        name="normalize_email",
                        reason=PlanReason.QUERY_CHANGED,
                        previous_query_sql="returns=TEXT\nbody=old_email",
                    ),
                ),
                seed_entries=(build_seed_entry(name="waffle_types"),),
            ),
            expected_fragments=(
                "normalize_email",
                "is_completed_order",
                "fact_orders",
                "stg_orders",
                "waffle_types",
            ),
            unexpected_fragments=("Functions (2", "Models (2"),
            expected_ordered_fragments=(
                "Changed functions (1)",
                "Query changed (1)",
                "Models (1)",
                "Functions (1)",
                "Seeds (1)",
            ),
        ),
        FormatPlanTestCase(
            description="detail rows align value column to longest displayed name",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="hourly_activity_with_daily_context",
                        action=PlanAction.INCREMENTAL_DELETE_INSERT,
                        reason=PlanReason.FIRST_RUN,
                        materialization_type=MaterializationType.INCREMENTAL,
                        incremental_strategy="delete_insert",
                        cursor_type="timestamp",
                        incremental_mode="microbatch",
                    ),
                    build_model_entry(
                        name="order_status_index",
                        action=PlanAction.INCREMENTAL_DELETE_INSERT,
                        reason=PlanReason.FIRST_RUN,
                        materialization_type=MaterializationType.INCREMENTAL,
                        incremental_strategy="delete_insert",
                        cursor_type="integer",
                    ),
                ),
            ),
            expected_fragments=(
                "\u251c\u2500\u2500 hourly_activity_with_daily_context "
                "delete_insert (timestamp, microbatch)",
                "\u2514\u2500\u2500 order_status_index                 delete_insert (integer)",
            ),
        ),
        FormatPlanTestCase(
            description="direct metadata caps remaining stale models after partial selection",
            plan_output=build_plan_output(
                metadata={
                    "direct_remaining_stale_model_names": tuple(
                        f"model_{index:02d}" for index in range(55)
                    ),
                },
            ),
            expected_fragments=(
                "Remaining stale",
                "models outside selection: 55",
                "model set: model_00",
                "... (+35 more; use --verbose to show all)",
            ),
        ),
        FormatPlanTestCase(
            description="direct metadata shows full remaining stale set in verbose output",
            plan_output=build_plan_output(
                metadata={
                    "direct_remaining_stale_model_names": tuple(
                        f"model_{index:02d}" for index in range(3)
                    ),
                },
            ),
            display_options=DisplayOptions(max_entries_per_section=None),
            expected_fragments=("model set: model_00, model_01, model_02",),
            unexpected_fragments=("use --verbose",),
        ),
        FormatPlanTestCase(
            description="direct selection diagnostics default hides stale warning and section",
            plan_output=build_plan_output(
                warnings=(
                    build_warning(
                        model_name=None,
                        message="Stale inputs detected\n\n  Affected selected models:\n    fact_orders",
                    ),
                ),
                metadata={
                    "selection_diagnostics": {"mode": "direct", "enabled": False},
                    "direct_remaining_stale_model_names": ("orders_rollup",),
                },
            ),
            include_direct_freshness_diagnostics=True,
            unexpected_fragments=(
                "Stale inputs detected",
                "Remaining stale",
                "orders_rollup",
            ),
        ),
        FormatPlanTestCase(
            description="direct selection diagnostics flag restores stale warning and section",
            plan_output=build_plan_output(
                warnings=(
                    build_warning(
                        model_name=None,
                        message="Stale inputs detected\n\n  Affected selected models:\n    fact_orders",
                    ),
                ),
                metadata={
                    "selection_diagnostics": {"mode": "direct", "enabled": True},
                    "direct_remaining_stale_model_names": ("orders_rollup",),
                },
            ),
            include_direct_freshness_diagnostics=False,
            expected_fragments=(
                "Stale inputs detected",
                "Remaining stale",
                "orders_rollup",
            ),
        ),
        FormatPlanTestCase(
            description="provider usages show compact selected Python surface counts",
            plan_output=build_plan_output(
                provider_usages=(
                    build_plan_provider_usage(
                        provider_name="marker_provider",
                        consumer_kind="loader",
                        consumer_name="raw_orders",
                    ),
                    build_plan_provider_usage(
                        provider_name="marker_provider",
                        consumer_kind="hook",
                        consumer_name="mark_pre",
                    ),
                ),
            ),
            python_plan_entries=(
                PythonPlanEntry(
                    name="publish_orders",
                    kind=PythonNodeKind.ASSET,
                    phase=PythonRunPhase.READ_SIDE,
                    provider_usages=(build_discovered_provider_usage(),),
                ),
            ),
            expected_fragments=(
                "Providers",
                "marker_provider  used by 3 selected Python surfaces",
            ),
        ),
        FormatPlanTestCase(
            description="provider usages show verbose selected Python surface details",
            plan_output=build_plan_output(
                provider_usages=(
                    build_plan_provider_usage(
                        provider_name="marker_provider",
                        consumer_kind="loader",
                        consumer_name="raw_orders",
                    ),
                    build_plan_provider_usage(
                        provider_name="marker_provider",
                        consumer_kind="hook",
                        consumer_name="mark_pre",
                    ),
                ),
            ),
            display_options=DisplayOptions(max_entries_per_section=None),
            python_plan_entries=(
                PythonPlanEntry(
                    name="publish_orders",
                    kind=PythonNodeKind.ASSET,
                    phase=PythonRunPhase.READ_SIDE,
                    provider_usages=(build_discovered_provider_usage(),),
                ),
            ),
            expected_fragments=(
                "Providers",
                "\u2514\u2500\u2500 marker_provider",
                "    ├── hook mark_pre (MarkerProvider)",
                "    ├── loader raw_orders (MarkerProvider)",
                "    └── task publish_orders (MarkerProvider)",
            ),
            unexpected_fragments=("used by 3 selected Python surfaces", "parameter"),
        ),
        FormatPlanTestCase(
            description="table-type conversions and warnings are capped like other sections",
            plan_output=build_plan_output(
                table_type_entries=tuple(
                    build_table_type_entry(model_name=f"orders_{index:02d}") for index in range(25)
                ),
                warnings=tuple(
                    build_warning(
                        model_name=f"customers_{index:02d}",
                        message="enforced column status type mismatch",
                    )
                    for index in range(25)
                ),
            ),
            expected_fragments=(
                "orders_19 desired=permanent actual=transient source=target",
                "customers_19",
                "Warnings (25)",
                "... and 5 more (use --verbose to show all)",
            ),
            unexpected_fragments=("orders_20 ", "customers_20"),
        ),
        FormatPlanTestCase(
            description="verbose output shows every table-type conversion and warning",
            plan_output=build_plan_output(
                table_type_entries=tuple(
                    build_table_type_entry(model_name=f"orders_{index:02d}") for index in range(25)
                ),
                warnings=tuple(
                    build_warning(
                        model_name=f"customers_{index:02d}",
                        message="enforced column status type mismatch",
                    )
                    for index in range(25)
                ),
            ),
            display_options=DisplayOptions(max_entries_per_section=None),
            expected_fragments=("orders_24 desired=permanent", "customers_24"),
            unexpected_fragments=("use --verbose",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_plan_output_when_formatting_then_contains_expected_fragments(
    test_case: FormatPlanTestCase,
) -> None:
    result: str = format_plan(
        plan=test_case.plan_output,
        full_refresh=test_case.full_refresh,
        use_color=False,
        display_options=test_case.display_options,
        include_direct_freshness_diagnostics=(test_case.include_direct_freshness_diagnostics),
        python_plan_entries=test_case.python_plan_entries,
    )

    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in result, f"Expected '{fragment}' in output:\n{result}"
    for fragment in test_case.unexpected_fragments:
        assert fragment not in result, f"Did not expect '{fragment}' in output:\n{result}"
    previous_index: int = -1
    for fragment in test_case.expected_ordered_fragments:
        current_index: int = result.index(fragment)
        assert current_index > previous_index, result
        previous_index = current_index


@pytest.mark.parametrize(
    "test_case",
    [
        FormatPlanColorTestCase(
            description="styles header names warnings and diffs semantically",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="dim_customers",
                        action=PlanAction.CREATE_TABLE,
                        reason=PlanReason.SCHEMA_CHANGED,
                        schema_findings=(
                            build_schema_finding(
                                kind=SchemaChangeKind.COLUMN_ADDED,
                                column_name="discount",
                                expected_type="FLOAT",
                            ),
                        ),
                        previous_query_sql="SELECT old_column FROM raw",
                    ),
                ),
                warnings=(
                    build_warning(
                        model_name="dim_customers",
                        message="schema change requires rebuild",
                        severity=WarningSeverity.WARNING,
                    ),
                ),
            ),
            expected_fragments=(
                "\033[34m\033[1mPlan ready\033[0m  \033[2m1 selected\033[0m",
                "dim_customers",
                "\033[32m      + discount  FLOAT   (added)\033[0m",
                "\033[33m\033[1mWarnings (1)\033[0m",
                "\033[2m\u2514\u2500\u2500\033[0m dim_customers",
                "    \033[2m\u2514\u2500\u2500\033[0m \033[33mschema change requires rebuild\033[0m",
            ),
        ),
        FormatPlanColorTestCase(
            description="styles provider names and consumers semantically",
            plan_output=build_plan_output(
                provider_usages=(
                    build_plan_provider_usage(
                        provider_name="marker_provider",
                        consumer_kind="loader",
                        consumer_name="raw_orders",
                    ),
                ),
            ),
            expected_fragments=(
                "marker_provider",
                "\033[2mused by 1 selected Python surface\033[0m",
            ),
        ),
        FormatPlanColorTestCase(
            description="styles source freshness and diff metadata semantically",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="fact_orders",
                        action=PlanAction.CREATE_TABLE,
                        reason=PlanReason.QUERY_CHANGED,
                        previous_query_sql="SELECT old_amount FROM raw_orders",
                        query_changed=True,
                    ),
                ),
                metadata={
                    "direct_source_freshness": {
                        "observed_source_names": ("raw_orders",),
                        "changed_source_names": ("raw_orders",),
                        "unchanged_source_names": (),
                        "unknown_source_names": (),
                        "stale_model_names": ("fact_orders",),
                    }
                },
            ),
            expected_fragments=(
                "\033[2mobserved:\033[0m 1",
                "\033[2mobserved set:\033[0m raw_orders",
                "\033[2mchanged:\033[0m \033[33m1\033[0m",
                "\033[2mchanged set:\033[0m \033[33mraw_orders\033[0m",
                "\033[2munchanged:\033[0m \033[2m0\033[0m",
                "\033[2msource-stale models:\033[0m \033[33mfact_orders\033[0m",
                "\033[2mquery diff:\033[0m",
                "\033[2m      --- previous\033[0m",
                "\033[2m      +++ current\033[0m",
                "\033[2m      @@",
            ),
        ),
        FormatPlanColorTestCase(
            description="dims routine kinds and accents changed view actions",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="stg_orders",
                        action=PlanAction.CREATE_VIEW,
                        reason=PlanReason.NO_CHANGE,
                        materialization_type=MaterializationType.VIEW,
                    ),
                    build_model_entry(
                        name="stg_payments",
                        action=PlanAction.CREATE_VIEW,
                        reason=PlanReason.QUERY_CHANGED,
                        materialization_type=MaterializationType.VIEW,
                        backfill_action=BackfillAction.FORWARD_ONLY,
                        previous_query_sql="SELECT payment_id FROM raw_payments",
                    ),
                    build_model_entry(
                        name="fact_payments",
                        action=PlanAction.INCREMENTAL_DELETE_INSERT,
                        reason=PlanReason.QUERY_CHANGED,
                        materialization_type=MaterializationType.INCREMENTAL,
                        backfill_action=BackfillAction.FORWARD_ONLY,
                        previous_query_sql="SELECT payment_id FROM stg_payments",
                    ),
                    build_model_entry(
                        name="partitioned_payments",
                        action=PlanAction.CUSTOM,
                        reason=PlanReason.QUERY_CHANGED,
                        materialization_type=MaterializationType.CUSTOM,
                        backfill_action=BackfillAction.FORWARD_ONLY,
                        custom_materialization_name="partition_tracked",
                        previous_query_sql="SELECT payment_id FROM stg_payments",
                    ),
                ),
            ),
            expected_fragments=(
                "stg_orders",
                "\033[2mview\033[0m",
                "stg_payments",
                "\033[34mrecreate view\033[0m",
                "fact_payments",
                "\033[34mcontinue forward\033[0m",
                "partitioned_payments",
                "\033[34mrun partition_tracked\033[0m",
            ),
        ),
        FormatPlanColorTestCase(
            description="leaf values mentioning runtime keep the plain value style",
            plan_output=build_plan_output(
                model_entries=(
                    build_model_entry(
                        name="session_events",
                        action=PlanAction.INCREMENTAL_DELETE_INSERT,
                        reason=PlanReason.NORMAL_INCREMENTAL,
                        materialization_type=MaterializationType.INCREMENTAL,
                        incremental_strategy="delete_insert",
                        cursor_column="runtime_started_at",
                        cursor_type="timestamp",
                    ),
                ),
            ),
            expected_fragments=("\033[2mcursor\033[0m  runtime_started_at (timestamp)",),
        ),
        FormatPlanColorTestCase(
            description="styles python dependency diff headers as metadata",
            plan_output=build_plan_output(),
            python_plan_entries=(
                PythonPlanEntry(
                    name="prepare_orders",
                    kind=PythonNodeKind.ASSET,
                    phase=PythonRunPhase.PRE_SQL_INGRESS,
                    identity_status=PythonIdentityStatus.CHANGED,
                    previous_metadata_json=json.dumps(
                        {
                            "dependencies": [
                                {
                                    "module": "python.tasks.helpers",
                                    "qualname": "order_label",
                                    "source_path": "python/tasks/helpers.py",
                                    "source_text": "def order_label():\n    return 'old'\n",
                                }
                            ]
                        },
                        sort_keys=True,
                    ),
                    current_metadata_json=json.dumps(
                        {
                            "dependencies": [
                                {
                                    "module": "python.tasks.helpers",
                                    "qualname": "order_label",
                                    "source_path": "python/tasks/helpers.py",
                                    "source_text": "def order_label():\n    return 'new'\n",
                                }
                            ]
                        },
                        sort_keys=True,
                    ),
                ),
            ),
            expected_fragments=(
                "\033[2m         # python/tasks/helpers.py :: python.tasks.helpers :: order_label\033[0m",
                "\033[38;5;167m      -    return 'old'\033[0m",
                "\033[32m      +    return 'new'\033[0m",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_plan_output_when_formatting_with_color_then_styles_semantic_parts(
    test_case: FormatPlanColorTestCase,
) -> None:
    result: str = format_plan(
        plan=test_case.plan_output,
        use_color=True,
        python_plan_entries=test_case.python_plan_entries,
    )

    for fragment in test_case.expected_fragments:
        assert fragment in result, f"Expected '{fragment}' in output:\n{result}"


@pytest.mark.parametrize(
    "test_case",
    [
        RuntimeBoundsLineTestCase(
            description="model cursor input",
            cursor_input_relations=(
                CursorInputRelation(
                    relation="analytics.stg_payments",
                    cursor_column="paid_at",
                    is_model_backed=True,
                    is_runtime_produced=True,
                    input_name="stg_payments",
                    input_kind=GraphResourceKind.MODEL,
                ),
            ),
            expected_bounds_value="computed at run time, once model stg_payments is up to date",
        ),
        RuntimeBoundsLineTestCase(
            description="seed cursor input",
            cursor_input_relations=(
                CursorInputRelation(
                    relation="analytics.payments",
                    cursor_column="paid_at",
                    is_runtime_produced=True,
                    input_name="payments",
                    input_kind=GraphResourceKind.SEED,
                ),
            ),
            expected_bounds_value="computed at run time, once seed payments is up to date",
        ),
        RuntimeBoundsLineTestCase(
            description="source cursor input",
            cursor_input_relations=(
                CursorInputRelation(
                    relation="raw.payments",
                    cursor_column="paid_at",
                    is_runtime_produced=True,
                    input_name="raw_payments",
                    input_kind=GraphResourceKind.SOURCE,
                ),
            ),
            expected_bounds_value="computed at run time, once source raw_payments is up to date",
        ),
        RuntimeBoundsLineTestCase(
            description="several cursor inputs name only those refreshed in this run",
            cursor_input_relations=(
                CursorInputRelation(
                    relation="analytics.payments",
                    cursor_column="paid_at",
                    is_runtime_produced=True,
                    input_name="payments",
                    input_kind=GraphResourceKind.SEED,
                ),
                CursorInputRelation(
                    relation="raw.refunds",
                    cursor_column="refunded_at",
                    is_runtime_produced=False,
                    input_name="raw_refunds",
                    input_kind=GraphResourceKind.SOURCE,
                ),
                CursorInputRelation(
                    relation="analytics.stg_orders",
                    cursor_column="ordered_at",
                    is_model_backed=True,
                    is_runtime_produced=True,
                    input_name="stg_orders",
                    input_kind=GraphResourceKind.MODEL,
                ),
            ),
            expected_bounds_value=(
                "computed at run time, once seed payments and model stg_orders are up to date"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cursor_input_refreshed_in_run_when_formatting_plan_then_bounds_line_names_input(
    test_case: RuntimeBoundsLineTestCase,
) -> None:
    plan_output: PlanOutput = build_plan_output(
        model_entries=(
            build_model_entry(
                name="daily_revenue",
                action=PlanAction.INCREMENTAL_DELETE_INSERT,
                reason=PlanReason.NORMAL_INCREMENTAL,
                materialization_type=MaterializationType.INCREMENTAL,
                incremental_strategy="delete_insert",
                cursor_column="revenue_date",
                cursor_type="timestamp",
                cursor_input_relations=test_case.cursor_input_relations,
            ),
        ),
    )

    plain: str = format_plan(plan=plan_output, use_color=False)
    colored: str = format_plan(plan=plan_output, use_color=True)

    assert f"bounds  {test_case.expected_bounds_value}" in plain, plain
    assert "runtime-owned" not in plain
    assert "model-backed" not in plain
    colored_lines: list[str] = colored.splitlines()
    assert (
        f"    \033[2m\u2514\u2500\u2500\033[0m \033[2mbounds\033[0m  {test_case.expected_bounds_value}"
        in colored_lines
    ), colored
    assert (
        "    \033[2m\u251c\u2500\u2500\033[0m \033[2mcursor\033[0m  revenue_date (timestamp)"
        in colored_lines
    ), colored
