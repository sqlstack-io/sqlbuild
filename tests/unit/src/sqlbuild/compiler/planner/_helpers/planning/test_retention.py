from __future__ import annotations

from dataclasses import replace
from datetime import UTC, datetime
from unittest.mock import Mock

import pytest

from sqlbuild._native import (
    Fingerprint,
    LocalConfig,
    MaterializationType,
    PlannerInputError,
    PlannerRuntime,
    PlannerScope,
    PlannerWarehouseState,
    ProjectConfig,
    ResolvedTableType,
    RetentionDirection,
    RetentionPlanEntry,
    RetentionPlanPhase,
    TableType,
    TableTypeDowngradePolicy,
    TableTypePlanEntry,
    TableTypeSource,
    TargetConfig,
    WarehouseFingerprints,
    plan_retention,
    plan_table_types,
)
from sqlbuild.adapter.contract.models import (
    RelationInfo,
    RenderedRetentionChange,
    RetentionState,
)
from sqlbuild.adapter.contract.types import (
    BuiltinAdapter,
    RetentionChangePhase,
    RetentionScope,
)
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from tests.unit.src.sqlbuild.compiler.planner._helpers.planning._test_types import (
    ForeignSnapshotRelationTestCase,
    RetentionPlanningErrorTestCase,
    RetentionPlanningTestCase,
    SnapshotRetentionPlanningTestCase,
    TableTypePlanningTestCase,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.planning.helpers import (
    build_retention_planner_inputs,
    retention_mock_adapter,
)

_EXISTING_ORDERS: dict[str, RelationInfo] = {
    "orders": RelationInfo(
        database="warehouse",
        schema="analytics",
        name="orders",
        relation_type="table",
    )
}


@pytest.mark.parametrize(
    "test_case",
    [
        TableTypePlanningTestCase(
            description="transient live table plans permanent upgrade",
            desired_type=TableType.PERMANENT,
            live_is_transient=True,
            relation_exists=True,
            downgrade_policy=TableTypeDowngradePolicy.REQUIRE_CONFIRMATION,
            expected_entry_count=1,
            expected_actual_type="transient",
            expected_downgrade=False,
        ),
        TableTypePlanningTestCase(
            description="permanent live table plans transient downgrade with target policy",
            desired_type=TableType.TRANSIENT,
            live_is_transient=False,
            relation_exists=True,
            downgrade_policy=TableTypeDowngradePolicy.DENY,
            expected_entry_count=1,
            expected_actual_type="permanent",
            expected_downgrade=True,
        ),
        TableTypePlanningTestCase(
            description="unknown live metadata plans fail-closed execution entry",
            desired_type=TableType.PERMANENT,
            live_is_transient=None,
            relation_exists=True,
            downgrade_policy=TableTypeDowngradePolicy.ALLOW,
            expected_entry_count=1,
            expected_actual_type=None,
            expected_downgrade=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_table_type_drift_when_planning_then_independent_entry_tracks_actual_and_policy(
    test_case: TableTypePlanningTestCase,
) -> None:
    adapter: Mock = retention_mock_adapter(adapter_name=BuiltinAdapter.SNOWFLAKE.value)
    adapter.maximum_identifier_length.return_value = 255
    relation: RelationInfo = RelationInfo(
        database="warehouse",
        schema="analytics",
        name="orders",
        relation_type="BASE TABLE",
        is_transient=test_case.live_is_transient,
    )
    runtime, warehouse, scope = build_retention_planner_inputs(
        adapter=adapter,
        desired_days=1,
        existing_relations={"orders": relation},
        config_values={"materialized": "table"},
        table_type=ResolvedTableType(
            value=test_case.desired_type,
            source=TableTypeSource.MODEL,
            declared=True,
        ),
        table_type_downgrade=test_case.downgrade_policy,
    )

    entries: tuple[TableTypePlanEntry, ...] = plan_table_types(
        runtime=runtime, warehouse=warehouse, scope=scope
    )

    assert len(entries) == test_case.expected_entry_count
    assert entries[0].actual_type == test_case.expected_actual_type
    assert entries[0].downgrade is test_case.expected_downgrade
    assert entries[0].downgrade_policy == test_case.downgrade_policy.value


@pytest.mark.parametrize(
    "test_case",
    [
        TableTypePlanningTestCase(
            description="matched permanent table has no conversion",
            desired_type=TableType.PERMANENT,
            live_is_transient=False,
            relation_exists=True,
            downgrade_policy=TableTypeDowngradePolicy.REQUIRE_CONFIRMATION,
            expected_entry_count=0,
            expected_actual_type=None,
            expected_downgrade=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_matched_or_missing_table_when_planning_type_then_no_entry_is_created(
    test_case: TableTypePlanningTestCase,
) -> None:
    adapter: Mock = retention_mock_adapter(adapter_name=BuiltinAdapter.SNOWFLAKE.value)
    adapter.maximum_identifier_length.return_value = 255
    runtime, warehouse, scope = build_retention_planner_inputs(
        adapter=adapter,
        desired_days=1,
        existing_relations={
            "orders": RelationInfo(
                database="warehouse",
                schema="analytics",
                name="orders",
                relation_type="BASE TABLE",
                is_transient=test_case.live_is_transient,
            )
        },
        config_values={"materialized": "table"},
        table_type=ResolvedTableType(
            value=test_case.desired_type,
            source=TableTypeSource.MODEL,
            declared=True,
        ),
    )
    entries: tuple[TableTypePlanEntry, ...] = plan_table_types(
        runtime=runtime, warehouse=warehouse, scope=scope
    )

    assert len(entries) == test_case.expected_entry_count


@pytest.mark.parametrize(
    "test_case",
    [
        TableTypePlanningTestCase(
            description="missing relation has no conversion",
            desired_type=TableType.PERMANENT,
            live_is_transient=None,
            relation_exists=False,
            downgrade_policy=TableTypeDowngradePolicy.REQUIRE_CONFIRMATION,
            expected_entry_count=0,
            expected_actual_type=None,
            expected_downgrade=False,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_table_when_planning_type_then_no_entry_is_created(
    test_case: TableTypePlanningTestCase,
) -> None:
    adapter: Mock = retention_mock_adapter(adapter_name=BuiltinAdapter.SNOWFLAKE.value)
    adapter.maximum_identifier_length.return_value = 255
    runtime, warehouse, scope = build_retention_planner_inputs(
        adapter=adapter,
        desired_days=1,
        existing_relations={},
        config_values={"materialized": "table"},
        table_type=ResolvedTableType(
            value=test_case.desired_type,
            source=TableTypeSource.MODEL,
            declared=True,
        ),
    )

    entries: tuple[TableTypePlanEntry, ...] = plan_table_types(
        runtime=runtime, warehouse=warehouse, scope=scope
    )

    assert len(entries) == test_case.expected_entry_count


@pytest.mark.parametrize(
    "test_case",
    [
        TableTypePlanningTestCase(
            description="table relation plans table type drift",
            desired_type=TableType.PERMANENT,
            live_is_transient=None,
            relation_exists=True,
            downgrade_policy=TableTypeDowngradePolicy.REQUIRE_CONFIRMATION,
            expected_entry_count=1,
            expected_actual_type=None,
            expected_downgrade=False,
            materialized=MaterializationType.TABLE,
        ),
        TableTypePlanningTestCase(
            description="incremental relation plans table type drift",
            desired_type=TableType.PERMANENT,
            live_is_transient=None,
            relation_exists=True,
            downgrade_policy=TableTypeDowngradePolicy.REQUIRE_CONFIRMATION,
            expected_entry_count=1,
            expected_actual_type=None,
            expected_downgrade=False,
            materialized=MaterializationType.INCREMENTAL,
        ),
        TableTypePlanningTestCase(
            description="microbatch incremental relation plans table type drift",
            desired_type=TableType.PERMANENT,
            live_is_transient=None,
            relation_exists=True,
            downgrade_policy=TableTypeDowngradePolicy.REQUIRE_CONFIRMATION,
            expected_entry_count=1,
            expected_actual_type=None,
            expected_downgrade=False,
            materialized=MaterializationType.INCREMENTAL,
            additional_config=(("incremental_mode", "microbatch"),),
        ),
        TableTypePlanningTestCase(
            description="snapshot relation plans table type drift",
            desired_type=TableType.PERMANENT,
            live_is_transient=None,
            relation_exists=True,
            downgrade_policy=TableTypeDowngradePolicy.REQUIRE_CONFIRMATION,
            expected_entry_count=1,
            expected_actual_type=None,
            expected_downgrade=False,
            materialized=MaterializationType.SNAPSHOT,
        ),
        TableTypePlanningTestCase(
            description="warehouse view with unknown table metadata has no drift",
            desired_type=TableType.PERMANENT,
            live_is_transient=None,
            relation_exists=True,
            downgrade_policy=TableTypeDowngradePolicy.REQUIRE_CONFIRMATION,
            expected_entry_count=0,
            expected_actual_type=None,
            expected_downgrade=False,
            materialized=MaterializationType.VIEW,
            relation_type="VIEW",
        ),
        TableTypePlanningTestCase(
            description="ephemeral model has no table type drift",
            desired_type=TableType.PERMANENT,
            live_is_transient=None,
            relation_exists=True,
            downgrade_policy=TableTypeDowngradePolicy.REQUIRE_CONFIRMATION,
            expected_entry_count=0,
            expected_actual_type=None,
            expected_downgrade=False,
            materialized="ephemeral",
        ),
        TableTypePlanningTestCase(
            description="seed materialization has no table type drift",
            desired_type=TableType.PERMANENT,
            live_is_transient=None,
            relation_exists=True,
            downgrade_policy=TableTypeDowngradePolicy.REQUIRE_CONFIRMATION,
            expected_entry_count=0,
            expected_actual_type=None,
            expected_downgrade=False,
            materialized=MaterializationType.SEED,
        ),
        TableTypePlanningTestCase(
            description="custom materialization has no table type drift",
            desired_type=TableType.PERMANENT,
            live_is_transient=None,
            relation_exists=True,
            downgrade_policy=TableTypeDowngradePolicy.REQUIRE_CONFIRMATION,
            expected_entry_count=0,
            expected_actual_type=None,
            expected_downgrade=False,
            materialized=MaterializationType.CUSTOM,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_materialization_family_when_planning_table_types_then_only_tables_have_drift(
    test_case: TableTypePlanningTestCase,
) -> None:
    adapter: Mock = retention_mock_adapter(adapter_name=BuiltinAdapter.SNOWFLAKE.value)
    adapter.maximum_identifier_length.return_value = 255
    relation: RelationInfo = RelationInfo(
        database="warehouse",
        schema="analytics",
        name="orders",
        relation_type=test_case.relation_type,
        is_transient=test_case.live_is_transient,
    )
    config_values: dict[str, object] = {
        "materialized": test_case.materialized,
        **dict(test_case.additional_config),
    }
    runtime, warehouse, scope = build_retention_planner_inputs(
        adapter=adapter,
        desired_days=1,
        existing_relations={"orders": relation},
        config_values=config_values,
        table_type=ResolvedTableType(
            value=test_case.desired_type,
            source=TableTypeSource.TARGET,
            declared=True,
        ),
    )

    entries: tuple[TableTypePlanEntry, ...] = plan_table_types(
        runtime=runtime, warehouse=warehouse, scope=scope
    )

    assert len(entries) == test_case.expected_entry_count


@pytest.mark.parametrize(
    "test_case",
    [
        RetentionPlanningErrorTestCase(
            description="declared table type on DuckDB fails closed",
            desired_days=1,
            expected_error_fragment="not supported on this adapter",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_declared_table_type_on_non_snowflake_when_planning_then_raises(
    test_case: RetentionPlanningErrorTestCase,
) -> None:
    adapter: Mock = retention_mock_adapter(adapter_name=BuiltinAdapter.DUCKDB.value)
    runtime, warehouse, scope = build_retention_planner_inputs(
        adapter=adapter,
        desired_days=test_case.desired_days,
        existing_relations={},
        config_values={"materialized": "table"},
        table_type=ResolvedTableType(
            value=TableType.PERMANENT,
            source=TableTypeSource.MODEL,
            declared=True,
        ),
    )

    with pytest.raises(PlannerInputError, match=test_case.expected_error_fragment):
        plan_table_types(runtime=runtime, warehouse=warehouse, scope=scope)


@pytest.mark.parametrize(
    "test_case",
    [
        TableTypePlanningTestCase(
            description="selected current model still receives conversion entry",
            desired_type=TableType.PERMANENT,
            live_is_transient=True,
            relation_exists=True,
            downgrade_policy=TableTypeDowngradePolicy.REQUIRE_CONFIRMATION,
            expected_entry_count=1,
            expected_actual_type="transient",
            expected_downgrade=False,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_changes_only_current_model_when_planning_type_then_selection_still_produces_entry(
    test_case: TableTypePlanningTestCase,
) -> None:
    adapter: Mock = retention_mock_adapter(adapter_name=BuiltinAdapter.SNOWFLAKE.value)
    adapter.maximum_identifier_length.return_value = 255
    runtime: PlannerRuntime
    warehouse: PlannerWarehouseState
    scope: PlannerScope
    runtime, warehouse, scope = build_retention_planner_inputs(
        adapter=adapter,
        desired_days=1,
        existing_relations={
            "orders": RelationInfo(
                database="warehouse",
                schema="analytics",
                name="orders",
                relation_type="BASE TABLE",
                is_transient=test_case.live_is_transient,
            )
        },
        config_values={"materialized": "table"},
        table_type=ResolvedTableType(
            value=test_case.desired_type,
            source=TableTypeSource.MODEL,
            declared=True,
        ),
    )
    warehouse = PlannerWarehouseState(
        inspection_relations=warehouse.inspection_relations,
        snapshot=replace(
            warehouse.snapshot,
            fingerprints=WarehouseFingerprints(
                models={
                    "orders": Fingerprint(
                        node_type="model",
                        node_name="orders",
                        target_database="warehouse",
                        target_schema="analytics",
                        target_name="orders",
                        run_id="prior-run",
                        definition_hash="current",
                        schema_fingerprint="current",
                        definition="SELECT 1 AS order_id",
                        ts=datetime(2026, 1, 1, tzinfo=UTC),
                        version_hash="current",
                    )
                }
            ),
        ),
    )

    entries: tuple[TableTypePlanEntry, ...] = plan_table_types(
        runtime=runtime, warehouse=warehouse, scope=scope
    )

    assert len(entries) == test_case.expected_entry_count


@pytest.mark.parametrize(
    "test_case",
    [
        RetentionPlanningTestCase(
            description="matching relation remains metadata-only",
            desired_days=7,
            observed_state=RetentionState(
                request_id="orders",
                scope=RetentionScope.RELATION,
                configured_days=7,
                effective_days=7,
            ),
            expected_direction=RetentionDirection.MATCH,
            expected_phase=RetentionPlanPhase.NONE,
        ),
        RetentionPlanningTestCase(
            description="increase is planned before writes",
            desired_days=7,
            observed_state=RetentionState(
                request_id="orders",
                scope=RetentionScope.RELATION,
                configured_days=1,
                effective_days=1,
            ),
            expected_direction=RetentionDirection.INCREASE,
            expected_phase=RetentionPlanPhase.PRE,
        ),
        RetentionPlanningTestCase(
            description="decrease is planned after success",
            desired_days=1,
            observed_state=RetentionState(
                request_id="orders",
                scope=RetentionScope.RELATION,
                configured_days=7,
                effective_days=7,
            ),
            expected_direction=RetentionDirection.DECREASE,
            expected_phase=RetentionPlanPhase.POST,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_retention_policy_when_planning_then_orders_metadata_safely(
    test_case: RetentionPlanningTestCase,
) -> None:
    adapter: Mock = retention_mock_adapter(adapter_name=BuiltinAdapter.SNOWFLAKE.value)
    adapter.inspect_retention.return_value = test_case.observed_state
    rendered_phase: RetentionChangePhase = {
        RetentionPlanPhase.NONE: RetentionChangePhase.ALTER,
        RetentionPlanPhase.PRE: RetentionChangePhase.PREPARE,
        RetentionPlanPhase.POST: RetentionChangePhase.FINALIZE,
    }[test_case.expected_phase]
    adapter.render_retention_changes.return_value = (
        RenderedRetentionChange(phase=rendered_phase, statements=("ALTER RETENTION",)),
    )
    runtime: PlannerRuntime
    warehouse: PlannerWarehouseState
    scope: PlannerScope
    runtime, warehouse, scope = build_retention_planner_inputs(
        adapter=adapter,
        desired_days=test_case.desired_days,
        existing_relations=_EXISTING_ORDERS,
        config_values={},
    )

    entries: tuple[RetentionPlanEntry, ...] = plan_retention(
        runtime=runtime,
        warehouse=warehouse,
        scope=scope,
    )

    assert len(entries) == 1
    assert entries[0].direction == test_case.expected_direction
    assert entries[0].phase == test_case.expected_phase
    assert bool(entries[0].statements) is (test_case.expected_phase != RetentionPlanPhase.NONE)


@pytest.mark.parametrize(
    "test_case",
    [
        RetentionPlanningErrorTestCase(
            description="missing Snowflake relation above transient limit",
            desired_days=30,
            expected_error_fragment="set table_type permanent",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_snowflake_relation_above_transient_limit_when_planning_then_fails_closed(
    test_case: RetentionPlanningErrorTestCase,
) -> None:
    adapter: Mock = retention_mock_adapter(adapter_name=BuiltinAdapter.SNOWFLAKE.value)
    runtime, warehouse, scope = build_retention_planner_inputs(
        adapter=adapter,
        desired_days=test_case.desired_days,
        existing_relations={},
        config_values={},
    )

    with pytest.raises(PlannerInputError, match=test_case.expected_error_fragment):
        plan_retention(runtime=runtime, warehouse=warehouse, scope=scope)


@pytest.mark.parametrize(
    "test_case",
    [
        RetentionPlanningErrorTestCase(
            description="existing transient Snowflake relation above limit",
            desired_days=30,
            expected_error_fragment="transient tables cannot retain",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_existing_transient_snowflake_relation_above_limit_when_planning_then_fails_closed(
    test_case: RetentionPlanningErrorTestCase,
) -> None:
    adapter: Mock = retention_mock_adapter(adapter_name=BuiltinAdapter.SNOWFLAKE.value)
    adapter.inspect_retention.return_value = RetentionState(
        request_id="orders",
        scope=RetentionScope.RELATION,
        configured_days=1,
        effective_days=1,
        is_transient=True,
    )
    runtime, warehouse, scope = build_retention_planner_inputs(
        adapter=adapter,
        desired_days=test_case.desired_days,
        existing_relations=_EXISTING_ORDERS,
        config_values={},
    )

    with pytest.raises(PlannerInputError, match=test_case.expected_error_fragment):
        plan_retention(runtime=runtime, warehouse=warehouse, scope=scope)


@pytest.mark.parametrize(
    "test_case",
    [
        RetentionPlanningTestCase(
            description="transient table becoming permanent defers retention until conversion",
            desired_days=30,
            observed_state=RetentionState(
                request_id="orders",
                scope=RetentionScope.RELATION,
                configured_days=1,
                effective_days=1,
                is_transient=True,
            ),
            expected_direction=RetentionDirection.APPLY_AFTER_CREATE,
            expected_phase=RetentionPlanPhase.AFTER_CREATE,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_transient_live_table_and_permanent_effective_type_when_planning_then_converts_first(
    test_case: RetentionPlanningTestCase,
) -> None:
    adapter: Mock = retention_mock_adapter(adapter_name=BuiltinAdapter.SNOWFLAKE.value)
    adapter.maximum_identifier_length.return_value = 255
    adapter.inspect_retention.return_value = test_case.observed_state
    adapter.render_retention_changes.return_value = (
        RenderedRetentionChange(
            phase=RetentionChangePhase.ALTER,
            statements=("ALTER RETENTION",),
        ),
    )
    runtime, warehouse, scope = build_retention_planner_inputs(
        adapter=adapter,
        desired_days=test_case.desired_days,
        existing_relations={
            "orders": RelationInfo(
                database="warehouse",
                schema="analytics",
                name="orders",
                relation_type="BASE TABLE",
                is_transient=True,
            )
        },
        config_values={"materialized": "table"},
        table_type=ResolvedTableType(
            value=TableType.PERMANENT,
            source=TableTypeSource.MODEL,
            declared=True,
        ),
    )

    conversions: tuple[TableTypePlanEntry, ...] = plan_table_types(
        runtime=runtime, warehouse=warehouse, scope=scope
    )
    retention: tuple[RetentionPlanEntry, ...] = plan_retention(
        runtime=runtime, warehouse=warehouse, scope=scope
    )

    assert len(conversions) == 1
    assert retention[0].direction == test_case.expected_direction
    assert retention[0].phase == test_case.expected_phase


@pytest.mark.parametrize(
    "test_case",
    [
        RetentionPlanningTestCase(
            description="already permanent table plans ordinary retention increase",
            desired_days=30,
            observed_state=RetentionState(
                request_id="orders",
                scope=RetentionScope.RELATION,
                configured_days=1,
                effective_days=1,
                is_transient=False,
            ),
            expected_direction=RetentionDirection.INCREASE,
            expected_phase=RetentionPlanPhase.PRE,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_permanent_live_table_with_retention_drift_when_planning_then_only_alters_retention(
    test_case: RetentionPlanningTestCase,
) -> None:
    adapter: Mock = retention_mock_adapter(adapter_name=BuiltinAdapter.SNOWFLAKE.value)
    adapter.maximum_identifier_length.return_value = 255
    adapter.inspect_retention.return_value = test_case.observed_state
    adapter.render_retention_changes.return_value = (
        RenderedRetentionChange(
            phase=RetentionChangePhase.PREPARE,
            statements=("ALTER RETENTION",),
        ),
    )
    runtime, warehouse, scope = build_retention_planner_inputs(
        adapter=adapter,
        desired_days=test_case.desired_days,
        existing_relations={
            "orders": RelationInfo(
                database="warehouse",
                schema="analytics",
                name="orders",
                relation_type="BASE TABLE",
                is_transient=False,
            )
        },
        config_values={"materialized": "table"},
        table_type=ResolvedTableType(
            value=TableType.PERMANENT,
            source=TableTypeSource.MODEL,
            declared=True,
        ),
    )

    conversions: tuple[TableTypePlanEntry, ...] = plan_table_types(
        runtime=runtime, warehouse=warehouse, scope=scope
    )
    retention: tuple[RetentionPlanEntry, ...] = plan_retention(
        runtime=runtime, warehouse=warehouse, scope=scope
    )

    assert len(conversions) == 0
    assert retention[0].direction == test_case.expected_direction
    assert retention[0].phase == test_case.expected_phase


@pytest.mark.parametrize(
    "test_case",
    [
        RetentionPlanningTestCase(
            description="missing BigQuery dataset applies retention after creation",
            desired_days=7,
            observed_state=RetentionState(
                request_id="warehouse.analytics",
                scope=RetentionScope.NAMESPACE,
                configured_days=None,
                effective_days=7,
                exists=False,
            ),
            expected_direction=RetentionDirection.APPLY_AFTER_CREATE,
            expected_phase=RetentionPlanPhase.AFTER_CREATE,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_bigquery_dataset_when_planning_then_defers_retention_until_after_create(
    test_case: RetentionPlanningTestCase,
) -> None:
    adapter: Mock = retention_mock_adapter(adapter_name=BuiltinAdapter.BIGQUERY.value)
    adapter.inspect_retention.return_value = test_case.observed_state
    adapter.render_retention_changes.return_value = (
        RenderedRetentionChange(
            phase=RetentionChangePhase.ALTER,
            statements=("ALTER DATASET RETENTION",),
        ),
    )
    runtime, warehouse, scope = build_retention_planner_inputs(
        adapter=adapter,
        desired_days=test_case.desired_days,
        existing_relations={},
        config_values={},
    )
    runtime: PlannerRuntime = PlannerRuntime(
        project=runtime.project,
        dialect=runtime.dialect,
        reads=runtime.reads,
        invocation_time=runtime.invocation_time,
        project_config=ProjectConfig(
            name="test",
            adapter=BuiltinAdapter.BIGQUERY.value,
            targets={"test": TargetConfig(owns_time_travel_retention_namespace=True)},
        ),
        local_config=LocalConfig(),
    )

    entries: tuple[RetentionPlanEntry, ...] = plan_retention(
        runtime=runtime,
        warehouse=warehouse,
        scope=scope,
    )

    assert entries[0].direction == test_case.expected_direction
    assert entries[0].phase == test_case.expected_phase
    assert RetentionPlanPhase.PRE not in tuple(entry.phase for entry in entries)


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotRetentionPlanningTestCase(
            description="many matching tables plan from the snapshot without metadata queries",
            model_names=("orders", "customers", "products", "inventory"),
            live_days=7,
            desired_days=7,
            expected_directions=("match", "match", "match", "match"),
        ),
        SnapshotRetentionPlanningTestCase(
            description="drifted tables plan increases from the snapshot without metadata queries",
            model_names=("orders", "customers"),
            live_days=1,
            desired_days=7,
            expected_directions=("increase", "increase"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_snapshot_retention_when_planning_then_no_table_is_inspected(
    test_case: SnapshotRetentionPlanningTestCase,
) -> None:
    runtime: PlannerRuntime
    warehouse: PlannerWarehouseState
    scope: PlannerScope
    runtime, warehouse, scope = build_retention_planner_inputs(
        adapter=SnowflakeAdapter(),
        desired_days=test_case.desired_days,
        existing_relations={
            name: RelationInfo(
                database="warehouse",
                schema="analytics",
                name=name,
                relation_type="base table",
                is_transient=False,
                retention_days=test_case.live_days,
            )
            for name in test_case.model_names
        },
        config_values={},
        model_names=test_case.model_names,
    )

    entries: tuple[RetentionPlanEntry, ...] = plan_retention(
        runtime=runtime, warehouse=warehouse, scope=scope
    )

    assert tuple(entry.direction.value for entry in entries) == test_case.expected_directions


@pytest.mark.parametrize(
    "test_case",
    [
        ForeignSnapshotRelationTestCase(
            description="same-named table in another schema is not trusted for retention",
            snapshot_schema="staging",
            snapshot_days=0,
            live_days=7,
            desired_days=1,
            expected_direction="decrease",
            expected_inspections=1,
        ),
        ForeignSnapshotRelationTestCase(
            description="destination table in the snapshot is used without inspection",
            snapshot_schema="analytics",
            snapshot_days=7,
            live_days=7,
            desired_days=1,
            expected_direction="decrease",
            expected_inspections=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snapshot_relation_identity_when_planning_retention_then_only_destination_is_trusted(
    test_case: ForeignSnapshotRelationTestCase,
) -> None:
    adapter: Mock = Mock(wraps=SnowflakeAdapter())
    adapter.adapter_name = BuiltinAdapter.SNOWFLAKE.value
    adapter.inspect_retention.side_effect = lambda *, connection, request: RetentionState(
        request_id=request.request_id,
        scope=RetentionScope.RELATION,
        configured_days=None,
        effective_days=test_case.live_days,
        relation_kind="PERMANENT",
        is_transient=False,
    )
    runtime: PlannerRuntime
    warehouse: PlannerWarehouseState
    scope: PlannerScope
    runtime, warehouse, scope = build_retention_planner_inputs(
        adapter=adapter,
        desired_days=test_case.desired_days,
        existing_relations={
            "orders": RelationInfo(
                database="warehouse",
                schema=test_case.snapshot_schema,
                name="orders",
                relation_type="base table",
                is_transient=False,
                retention_days=test_case.snapshot_days,
            )
        },
        config_values={},
    )

    entries: tuple[RetentionPlanEntry, ...] = plan_retention(
        runtime=runtime, warehouse=warehouse, scope=scope
    )

    assert entries[0].direction.value == test_case.expected_direction
    assert adapter.inspect_retention.call_count == test_case.expected_inspections
