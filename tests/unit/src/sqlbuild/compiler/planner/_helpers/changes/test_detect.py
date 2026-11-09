from __future__ import annotations

from dataclasses import replace
from datetime import datetime

import pytest

from sqlbuild._native import (
    BackfillAction,
    BackfillResult,
    ChangeDetectionResult,
    ChangeKind,
    Fingerprint,
    InferredColumn,
    PlanAction,
    PlannerChangeResults,
    PlannerScope,
    PlanReason,
    PlanWarning,
    SqlHookEntry,
    WarehouseFingerprints,
    WarehouseSnapshot,
    WarningSeverity,
    build_model_warnings,
    build_version_identity_metadata_json,
    compute_query_hash,
    detect_changes,
    detect_model_changes,
    get_materialization_type,
    resolve_model_plan_action,
)
from sqlbuild.compiler.compile.types import CompiledModelView
from tests.unit.src.sqlbuild.compiler.planner._helpers.changes._test_helpers import (
    build_function_caller_metadata_case,
    build_metadata_json_with_audit_gate,
    build_model_from_metadata_test_case,
    build_model_from_test_case,
    build_project_for_function_metadata_detection,
    build_scope_for_function_metadata_detection,
    build_snapshot_for_metadata_test_case,
    build_snapshot_from_test_case,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.changes._test_types import (
    DetectFunctionCallerChangeTestCase,
    DetectModelChangesTestCase,
    DetectModelMetadataTestCase,
    DetectRenamedModelTestCase,
    DroppedRelationPlanActionTestCase,
)

_QUERY_SQL: str = "SELECT id, name FROM orders"
_MATCHING_HASH: str = compute_query_hash(query_sql=_QUERY_SQL, dialect=None)
_DIFFERENT_HASH: str = "completely_different_hash"
_OLD_SCHEME_HASH: str = "fingerprint_from_an_older_scheme"


@pytest.mark.parametrize(
    "test_case",
    [
        DetectModelChangesTestCase(
            description="detects first run when relation and fingerprint are missing",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={},
            schema_columns=(),
            relation_exists=False,
            fingerprint_query_hash=None,
            warehouse_column_names=(),
            sql_analysis_enabled=False,
            query_change_tracking=True,
            full_refresh=False,
            expected_change_kind=ChangeKind.FIRST_RUN,
            expected_backfill_action=BackfillAction.FULL,
        ),
        DetectModelChangesTestCase(
            description="detects first run when relation is missing despite matching fingerprint",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={},
            schema_columns=(),
            relation_exists=False,
            fingerprint_query_hash=_MATCHING_HASH,
            warehouse_column_names=(),
            sql_analysis_enabled=False,
            query_change_tracking=True,
            full_refresh=False,
            expected_change_kind=ChangeKind.FIRST_RUN,
            expected_backfill_action=BackfillAction.FULL,
            expected_recorded_build_relation_missing=True,
        ),
        DetectModelChangesTestCase(
            description="detects first run when relation is missing despite changed fingerprint",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={"replay_on_change": "bounded-30d"},
            schema_columns=(("id", "INTEGER"), ("status", "VARCHAR")),
            relation_exists=False,
            fingerprint_query_hash=_DIFFERENT_HASH,
            fingerprint_config_values={"materialized": "view"},
            warehouse_column_names=(),
            sql_analysis_enabled=False,
            query_change_tracking=True,
            full_refresh=False,
            expected_change_kind=ChangeKind.FIRST_RUN,
            expected_backfill_action=BackfillAction.FULL,
            expected_recorded_build_relation_missing=True,
        ),
        DetectModelChangesTestCase(
            description="keeps verified rename handover when migration destination is pending",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={},
            schema_columns=(),
            relation_exists=False,
            fingerprint_query_hash=_MATCHING_HASH,
            warehouse_column_names=(),
            sql_analysis_enabled=False,
            query_change_tracking=True,
            full_refresh=False,
            expected_change_kind=ChangeKind.RENAMED,
            expected_backfill_action=BackfillAction.FORWARD_ONLY,
            renamed_by_migration=True,
        ),
        DetectModelChangesTestCase(
            description="uses stored state once renamed destination exists",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={},
            schema_columns=(),
            relation_exists=True,
            fingerprint_query_hash=_MATCHING_HASH,
            warehouse_column_names=(),
            sql_analysis_enabled=False,
            query_change_tracking=True,
            full_refresh=False,
            expected_change_kind=ChangeKind.NO_CHANGE,
            expected_backfill_action=BackfillAction.FORWARD_ONLY,
            renamed_by_migration=True,
        ),
        DetectModelChangesTestCase(
            description="detects no change when query hash matches and no schema columns declared",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={},
            schema_columns=(),
            relation_exists=True,
            fingerprint_query_hash=_MATCHING_HASH,
            warehouse_column_names=(("id", "INTEGER"), ("name", "VARCHAR")),
            sql_analysis_enabled=False,
            query_change_tracking=True,
            full_refresh=False,
            expected_change_kind=ChangeKind.NO_CHANGE,
            expected_backfill_action=BackfillAction.FORWARD_ONLY,
        ),
        DetectModelChangesTestCase(
            description="detects query change when hash differs",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={"replay_on_change": "bounded-30d"},
            schema_columns=(),
            relation_exists=True,
            fingerprint_query_hash=_DIFFERENT_HASH,
            warehouse_column_names=(),
            sql_analysis_enabled=False,
            query_change_tracking=True,
            full_refresh=False,
            expected_change_kind=ChangeKind.QUERY_CHANGED,
            expected_backfill_action=BackfillAction.BOUNDED,
        ),
        DetectModelChangesTestCase(
            description="old-scheme fingerprint of the unchanged query is no change",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={"replay_on_change": "bounded-30d"},
            schema_columns=(),
            relation_exists=True,
            fingerprint_query_hash=_OLD_SCHEME_HASH,
            fingerprint_definition="select id,\n  name\nfrom orders -- staged",
            warehouse_column_names=(),
            sql_analysis_enabled=False,
            query_change_tracking=True,
            full_refresh=False,
            expected_change_kind=ChangeKind.NO_CHANGE,
            expected_backfill_action=BackfillAction.FORWARD_ONLY,
            expected_definition_hash=_OLD_SCHEME_HASH,
        ),
        DetectModelChangesTestCase(
            description="old-scheme fingerprint of an edited query is a query change",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={"replay_on_change": "bounded-30d"},
            schema_columns=(),
            relation_exists=True,
            fingerprint_query_hash=_OLD_SCHEME_HASH,
            fingerprint_definition="SELECT id FROM orders",
            warehouse_column_names=(),
            sql_analysis_enabled=False,
            query_change_tracking=True,
            full_refresh=False,
            expected_change_kind=ChangeKind.QUERY_CHANGED,
            expected_backfill_action=BackfillAction.BOUNDED,
        ),
        DetectModelChangesTestCase(
            description="old-scheme fingerprint with an empty definition is a query change once",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={"replay_on_change": "bounded-30d"},
            schema_columns=(),
            relation_exists=True,
            fingerprint_query_hash=_OLD_SCHEME_HASH,
            fingerprint_definition="",
            warehouse_column_names=(),
            sql_analysis_enabled=False,
            query_change_tracking=True,
            full_refresh=False,
            expected_change_kind=ChangeKind.QUERY_CHANGED,
            expected_backfill_action=BackfillAction.BOUNDED,
        ),
        DetectModelChangesTestCase(
            description="old-scheme fingerprint of an unchanged replaying incremental model never replays",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={"materialized": "incremental", "replay_on_change": "full"},
            fingerprint_config_values={"materialized": "incremental", "replay_on_change": "full"},
            schema_columns=(),
            relation_exists=True,
            fingerprint_query_hash=_OLD_SCHEME_HASH,
            fingerprint_definition=_QUERY_SQL,
            warehouse_column_names=(),
            sql_analysis_enabled=False,
            query_change_tracking=True,
            full_refresh=False,
            expected_change_kind=ChangeKind.NO_CHANGE,
            expected_backfill_action=BackfillAction.FORWARD_ONLY,
            expected_definition_hash=_OLD_SCHEME_HASH,
        ),
        DetectModelChangesTestCase(
            description="schema finding of an unchanged model never replays",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={"replay_on_change": "full"},
            schema_columns=(("id", "INTEGER"), ("status", "VARCHAR")),
            relation_exists=True,
            fingerprint_query_hash=_MATCHING_HASH,
            warehouse_column_names=(("id", "INTEGER"),),
            sql_analysis_enabled=False,
            query_change_tracking=True,
            full_refresh=False,
            expected_change_kind=ChangeKind.SCHEMA_CHANGED,
            expected_backfill_action=BackfillAction.FORWARD_ONLY,
        ),
        DetectModelChangesTestCase(
            description="returns full backfill when full refresh is requested",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={},
            schema_columns=(),
            relation_exists=True,
            fingerprint_query_hash=_MATCHING_HASH,
            warehouse_column_names=(),
            sql_analysis_enabled=False,
            query_change_tracking=True,
            full_refresh=True,
            expected_change_kind=ChangeKind.NO_CHANGE,
            expected_backfill_action=BackfillAction.FULL,
        ),
        DetectModelChangesTestCase(
            description="skips query change detection when tracking is disabled",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={},
            schema_columns=(),
            relation_exists=True,
            fingerprint_query_hash=_DIFFERENT_HASH,
            warehouse_column_names=(),
            sql_analysis_enabled=False,
            query_change_tracking=False,
            full_refresh=False,
            expected_change_kind=ChangeKind.NO_CHANGE,
            expected_backfill_action=BackfillAction.FORWARD_ONLY,
        ),
        DetectModelChangesTestCase(
            description="detects config change when version identity config differs",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={"materialized": "table"},
            fingerprint_config_values={"materialized": "view"},
            schema_columns=(),
            relation_exists=True,
            fingerprint_query_hash=_MATCHING_HASH,
            warehouse_column_names=(),
            sql_analysis_enabled=False,
            query_change_tracking=True,
            full_refresh=False,
            expected_change_kind=ChangeKind.CONFIG_CHANGED,
            expected_backfill_action=BackfillAction.FORWARD_ONLY,
        ),
        DetectModelChangesTestCase(
            description="ignores unknown config differences",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={"future_default_flag": "off"},
            fingerprint_config_values={},
            schema_columns=(),
            relation_exists=True,
            fingerprint_query_hash=_MATCHING_HASH,
            warehouse_column_names=(),
            sql_analysis_enabled=False,
            query_change_tracking=True,
            full_refresh=False,
            expected_change_kind=ChangeKind.NO_CHANGE,
            expected_backfill_action=BackfillAction.FORWARD_ONLY,
        ),
        DetectModelChangesTestCase(
            description="inferred upstream column never replays the model",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={"materialized": "incremental", "replay_on_change": "full"},
            fingerprint_config_values={"materialized": "incremental", "replay_on_change": "full"},
            schema_columns=(),
            relation_exists=True,
            fingerprint_query_hash=_MATCHING_HASH,
            warehouse_column_names=(("id", "INTEGER"),),
            sql_analysis_enabled=True,
            query_change_tracking=True,
            full_refresh=False,
            inferred_columns=(
                InferredColumn(name="id", type="INTEGER"),
                InferredColumn(name="channel", type="VARCHAR"),
            ),
            expected_change_kind=ChangeKind.SCHEMA_CHANGED,
            expected_backfill_action=BackfillAction.FORWARD_ONLY,
        ),
        DetectModelChangesTestCase(
            description="schema finding with an own header change follows the replay policy",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={"materialized": "incremental", "replay_on_change": "full"},
            fingerprint_config_values={
                "materialized": "incremental",
                "replay_on_change": "full",
                "on_schema_change": "fail",
            },
            schema_columns=(("id", "INTEGER"), ("channel", "VARCHAR")),
            relation_exists=True,
            fingerprint_query_hash=_MATCHING_HASH,
            warehouse_column_names=(("id", "INTEGER"),),
            sql_analysis_enabled=False,
            query_change_tracking=True,
            full_refresh=False,
            expected_change_kind=ChangeKind.SCHEMA_CHANGED,
            expected_backfill_action=BackfillAction.FULL,
        ),
        DetectModelChangesTestCase(
            description="undeclared column dropped from inferred columns never replays",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={"materialized": "incremental", "replay_on_change": "full"},
            fingerprint_config_values={"materialized": "incremental", "replay_on_change": "full"},
            schema_columns=(("id", "INTEGER"),),
            relation_exists=True,
            fingerprint_query_hash=_MATCHING_HASH,
            warehouse_column_names=(("id", "INTEGER"), ("event_date", "DATE")),
            sql_analysis_enabled=True,
            query_change_tracking=True,
            full_refresh=False,
            inferred_columns=(InferredColumn(name="id", type="INTEGER"),),
            expected_change_kind=ChangeKind.SCHEMA_CHANGED,
            expected_backfill_action=BackfillAction.FORWARD_ONLY,
        ),
        DetectModelChangesTestCase(
            description="declared column removed upstream of an unchanged model never replays",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={"materialized": "incremental", "replay_on_change": "full"},
            fingerprint_config_values={"materialized": "incremental", "replay_on_change": "full"},
            schema_columns=(("id", "INTEGER"),),
            schema_type_enforcement=True,
            relation_exists=True,
            fingerprint_query_hash=_MATCHING_HASH,
            warehouse_column_names=(("id", "INTEGER"), ("event_date", "DATE")),
            sql_analysis_enabled=False,
            query_change_tracking=True,
            full_refresh=False,
            expected_change_kind=ChangeKind.SCHEMA_CHANGED,
            expected_backfill_action=BackfillAction.FORWARD_ONLY,
        ),
        DetectModelChangesTestCase(
            description="schema finding after an own declared-column edit follows replay policy",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={"replay_on_change": "full"},
            schema_columns=(("id", "INTEGER"), ("status", "VARCHAR")),
            relation_exists=True,
            fingerprint_query_hash=_MATCHING_HASH,
            fingerprint_extra_metadata={"declared_columns_hash": "previous-declared-columns"},
            warehouse_column_names=(("id", "INTEGER"),),
            sql_analysis_enabled=False,
            query_change_tracking=True,
            full_refresh=False,
            expected_change_kind=ChangeKind.SCHEMA_CHANGED,
            expected_backfill_action=BackfillAction.FULL,
        ),
        DetectModelChangesTestCase(
            description="does not report passthrough columns removed for unresolved star",
            model_name="orders",
            query_sql=_QUERY_SQL,
            config_values={},
            schema_columns=(),
            relation_exists=True,
            fingerprint_query_hash=_MATCHING_HASH,
            warehouse_column_names=(("id", "INTEGER"), ("loaded_at", "TIMESTAMP")),
            sql_analysis_enabled=True,
            query_change_tracking=True,
            full_refresh=False,
            inferred_columns=(InferredColumn(name="loaded_at", type="TIMESTAMP"),),
            fast_lineage_has_star=True,
            expected_change_kind=ChangeKind.NO_CHANGE,
            expected_backfill_action=BackfillAction.FORWARD_ONLY,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_model_and_snapshot_when_detecting_changes_then_returns_expected(
    test_case: DetectModelChangesTestCase,
) -> None:
    model: CompiledModelView = build_model_from_test_case(test_case)
    snapshot: WarehouseSnapshot = build_snapshot_from_test_case(test_case)

    result: ChangeDetectionResult = detect_model_changes(
        model=model,
        snapshot=snapshot,
        sql_analysis_enabled=test_case.sql_analysis_enabled,
        query_change_tracking=test_case.query_change_tracking,
        full_refresh=test_case.full_refresh,
    )

    assert result.change_kind == test_case.expected_change_kind
    assert result.backfill.action == test_case.expected_backfill_action
    assert (
        result.recorded_build_relation_missing == test_case.expected_recorded_build_relation_missing
    )
    assert test_case.expected_definition_hash in (None, result.fingerprint_definition_hash)


@pytest.mark.parametrize(
    "test_case",
    [
        DroppedRelationPlanActionTestCase(
            description="incremental plans create table",
            config_values={"materialized": "incremental", "incremental_strategy": "delete_insert"},
            expected_action=PlanAction.CREATE_TABLE,
            expected_reason=PlanReason.FIRST_RUN,
        ),
        DroppedRelationPlanActionTestCase(
            description="table plans create table",
            config_values={"materialized": "table"},
            expected_action=PlanAction.CREATE_TABLE,
            expected_reason=PlanReason.FIRST_RUN,
        ),
        DroppedRelationPlanActionTestCase(
            description="view plans create view",
            config_values={"materialized": "view"},
            expected_action=PlanAction.CREATE_VIEW,
            expected_reason=PlanReason.FIRST_RUN,
        ),
        DroppedRelationPlanActionTestCase(
            description="snapshot plans first run",
            config_values={"materialized": "snapshot"},
            expected_action=PlanAction.SNAPSHOT,
            expected_reason=PlanReason.FIRST_RUN,
        ),
        DroppedRelationPlanActionTestCase(
            description="custom materialization plans first run",
            config_values={"materialized": "partition_tracked"},
            expected_action=PlanAction.CUSTOM,
            expected_reason=PlanReason.FIRST_RUN,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dropped_relation_with_recorded_build_when_planning_then_plans_first_run(
    test_case: DroppedRelationPlanActionTestCase,
) -> None:
    detection_case: DetectModelChangesTestCase = DetectModelChangesTestCase(
        description=test_case.description,
        model_name="orders",
        query_sql=_QUERY_SQL,
        config_values=test_case.config_values,
        schema_columns=(),
        relation_exists=False,
        fingerprint_query_hash=_MATCHING_HASH,
        warehouse_column_names=(),
        sql_analysis_enabled=False,
        query_change_tracking=True,
        full_refresh=False,
        expected_change_kind=ChangeKind.FIRST_RUN,
        expected_backfill_action=BackfillAction.FULL,
    )
    model: CompiledModelView = build_model_from_test_case(detection_case)

    change: ChangeDetectionResult = detect_model_changes(
        model=model,
        snapshot=build_snapshot_from_test_case(detection_case),
        sql_analysis_enabled=False,
        query_change_tracking=True,
        full_refresh=False,
    )
    action: PlanAction
    reason: PlanReason
    action, reason = resolve_model_plan_action(
        model=model, change_result=change, full_refresh=False
    )
    warnings: tuple[PlanWarning, ...] = build_model_warnings(
        model_name=model.name,
        materialization_type=get_materialization_type(model),
        change_result=change,
        schema_actions=(),
        on_schema_change=None,
        type_enforcement=False,
    )

    assert (action, reason) == (test_case.expected_action, test_case.expected_reason)
    assert change.backfill.action == BackfillAction.FULL
    assert change.previous_version_hash is None
    assert change.previous_metadata_json is None
    assert (change.query_changed, change.config_changed, change.schema_findings) == (
        False,
        False,
        (),
    )
    assert [warning.code for warning in warnings] == ["RECORDED_RELATION_MISSING"]
    assert warnings[0].severity == WarningSeverity.WARNING
    assert warnings[0].model_name == "orders"


@pytest.mark.parametrize(
    "test_case",
    [
        DetectModelMetadataTestCase(
            description="detects function change when dependent function hash changes",
            config_values={},
            schema_columns=(),
            deps=("is_large_order",),
            function_local_hashes={"is_large_order": "new"},
            previous_metadata_json=build_version_identity_metadata_json(
                model_name="orders",
                config_values={},
                local_function_hashes={"is_large_order": "old"},
            ),
            expected_change_kind=ChangeKind.FUNCTION_CHANGED,
            expected_metadata_fragments=('"local_function_hashes":{"is_large_order":"new"}',),
        ),
        DetectModelMetadataTestCase(
            description="detects config change when execution signature changes",
            config_values={"contract": "enforced"},
            schema_columns=(("order_id", "INTEGER", False),),
            deps=(),
            function_local_hashes={},
            previous_metadata_json=build_version_identity_metadata_json(
                model_name="orders",
                config_values={"contract": "enforced"},
                execution_signature={},
            ),
            expected_change_kind=ChangeKind.CONFIG_CHANGED,
            expected_metadata_fragments=(
                '"execution_signature":{"contract":{"columns":[{"name":"order_id",'
                '"nullable":false,"type":"INTEGER"}],"enforced":true}}',
            ),
        ),
        DetectModelMetadataTestCase(
            description="detects config change when typed hook execution signature changes",
            config_values={
                "post_hooks": [SqlHookEntry(statement="INSERT INTO audit_log SELECT 'new'")]
            },
            schema_columns=(),
            deps=(),
            function_local_hashes={},
            previous_metadata_json=build_version_identity_metadata_json(
                model_name="orders",
                config_values={
                    "post_hooks": [SqlHookEntry(statement="INSERT INTO audit_log SELECT 'old'")]
                },
                execution_signature={
                    "post_hooks": [SqlHookEntry(statement="INSERT INTO audit_log SELECT 'old'")]
                },
            ),
            expected_change_kind=ChangeKind.CONFIG_CHANGED,
            expected_metadata_fragments=(
                '"post_hooks":[{"statement":"INSERT INTO audit_log SELECT \'new\'","type":"sql"}]',
            ),
        ),
        DetectModelMetadataTestCase(
            description="ignores runtime audit gate proof when version identity matches",
            config_values={"materialized": "table"},
            schema_columns=(),
            deps=(),
            function_local_hashes={},
            previous_metadata_json=build_metadata_json_with_audit_gate(
                build_version_identity_metadata_json(
                    model_name="orders",
                    config_values={"materialized": "table"},
                    execution_signature={},
                )
            ),
            expected_change_kind=ChangeKind.NO_CHANGE,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_model_identity_metadata_when_detecting_changes_then_uses_aligned_metadata(
    test_case: DetectModelMetadataTestCase,
) -> None:
    model: CompiledModelView = build_model_from_metadata_test_case(test_case)
    snapshot: WarehouseSnapshot = build_snapshot_for_metadata_test_case(test_case)

    result: ChangeDetectionResult = detect_model_changes(
        model=model,
        snapshot=snapshot,
        sql_analysis_enabled=False,
        query_change_tracking=True,
        full_refresh=False,
        function_local_hashes=test_case.function_local_hashes,
    )

    assert result.change_kind == test_case.expected_change_kind
    fragment: str
    for fragment in test_case.expected_metadata_fragments:
        assert fragment in (result.fingerprint_metadata_json or "")


@pytest.mark.parametrize(
    "test_case",
    [
        DetectFunctionCallerChangeTestCase(
            description="changed function continues forward by default",
            metadata_case=build_function_caller_metadata_case(
                config_values={},
                function_local_hashes={"is_large_order": "new"},
                previous_function_hashes={"is_large_order": "old"},
                previous_config_values={},
            ),
            query_change_tracking=True,
            expected_changed_functions=("is_large_order",),
            expected_config_changed=False,
            expected_backfill=BackfillResult(action=BackfillAction.FORWARD_ONLY),
        ),
        DetectFunctionCallerChangeTestCase(
            description="changed function applies the caller's own bounded replay",
            metadata_case=build_function_caller_metadata_case(
                config_values={"replay_on_change": "bounded-7d"},
                function_local_hashes={"is_large_order": "new", "normalize_status": "same"},
                previous_function_hashes={"is_large_order": "old", "normalize_status": "same"},
                previous_config_values={"replay_on_change": "bounded-7d"},
            ),
            query_change_tracking=True,
            expected_changed_functions=("is_large_order",),
            expected_config_changed=False,
            expected_backfill=BackfillResult(action=BackfillAction.BOUNDED, duration="7d"),
        ),
        DetectFunctionCallerChangeTestCase(
            description="function change alongside a config change reports both",
            metadata_case=build_function_caller_metadata_case(
                config_values={"replay_on_change": "full", "lookback": "1d"},
                function_local_hashes={"is_large_order": "new"},
                previous_function_hashes={"is_large_order": "old"},
                previous_config_values={"replay_on_change": "full"},
            ),
            query_change_tracking=True,
            expected_changed_functions=("is_large_order",),
            expected_config_changed=True,
            expected_backfill=BackfillResult(action=BackfillAction.FULL),
        ),
        DetectFunctionCallerChangeTestCase(
            description="newly called function is not a function change",
            metadata_case=build_function_caller_metadata_case(
                config_values={"replay_on_change": "full"},
                function_local_hashes={"is_large_order": "new"},
                previous_function_hashes={},
                previous_config_values={"replay_on_change": "full"},
            ),
            query_change_tracking=True,
            expected_changed_functions=(),
            expected_config_changed=True,
            expected_backfill=BackfillResult(action=BackfillAction.FORWARD_ONLY),
        ),
        DetectFunctionCallerChangeTestCase(
            description="disabled query change tracking keeps a function change as config only",
            metadata_case=build_function_caller_metadata_case(
                config_values={"replay_on_change": "full"},
                function_local_hashes={"is_large_order": "new"},
                previous_function_hashes={"is_large_order": "old"},
                previous_config_values={"replay_on_change": "full"},
            ),
            query_change_tracking=False,
            expected_changed_functions=(),
            expected_config_changed=True,
            expected_backfill=BackfillResult(action=BackfillAction.FORWARD_ONLY),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_called_function_change_when_detecting_caller_changes_then_caller_policy_decides(
    test_case: DetectFunctionCallerChangeTestCase,
) -> None:
    result: ChangeDetectionResult = detect_model_changes(
        model=build_model_from_metadata_test_case(test_case.metadata_case),
        snapshot=build_snapshot_for_metadata_test_case(test_case.metadata_case),
        sql_analysis_enabled=False,
        query_change_tracking=test_case.query_change_tracking,
        full_refresh=False,
        function_local_hashes=test_case.metadata_case.function_local_hashes,
    )

    assert (result.changed_functions, result.config_changed, result.backfill) == (
        test_case.expected_changed_functions,
        test_case.expected_config_changed,
        test_case.expected_backfill,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        DetectModelMetadataTestCase(
            description="direct project function hash change marks dependent model changed",
            config_values={},
            schema_columns=(),
            deps=("is_large_order",),
            function_local_hashes={"is_large_order": "old"},
            previous_metadata_json="{}",
            expected_change_kind=ChangeKind.FUNCTION_CHANGED,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_direct_project_function_hash_change_when_detecting_changes_then_model_is_changed(
    test_case: DetectModelMetadataTestCase,
) -> None:
    scope: PlannerScope = build_scope_for_function_metadata_detection()
    snapshot: WarehouseSnapshot = WarehouseSnapshot(
        existing_relations=build_snapshot_for_metadata_test_case(
            DetectModelMetadataTestCase(
                description="snapshot relation",
                config_values={},
                schema_columns=(),
                deps=(),
                function_local_hashes={},
                previous_metadata_json="{}",
                expected_change_kind=ChangeKind.NO_CHANGE,
            )
        ).existing_relations,
        existing_columns={},
        fingerprints=WarehouseFingerprints(
            models={
                "orders": Fingerprint(
                    node_type="model",
                    node_name="orders",
                    target_database=None,
                    target_schema=None,
                    target_name="orders",
                    run_id="run_001",
                    definition_hash=compute_query_hash(
                        query_sql="SELECT is_large_order(amount) AS large_order FROM orders",
                        dialect=None,
                    ),
                    schema_fingerprint="schema_a",
                    definition="SELECT is_large_order(amount) AS large_order FROM orders",
                    metadata_json=build_version_identity_metadata_json(
                        model_name="orders",
                        config_values={},
                        local_function_hashes=test_case.function_local_hashes,
                    ),
                    ts=datetime(2026, 1, 15, 12, 0, 0),
                )
            }
        ),
    )

    result: PlannerChangeResults = detect_changes(
        project=build_project_for_function_metadata_detection(),
        scope=scope,
        snapshot=snapshot,
        full_refresh=False,
    )

    assert result.models["orders"].change_kind == test_case.expected_change_kind


@pytest.mark.parametrize(
    "test_case",
    [
        DetectRenamedModelTestCase(
            description="rename with the handed-over definition continues forward",
            previous_definition="SELECT 1 AS order_id",
            config_values={"replay_on_change": "full"},
            expected_query_changed=False,
            expected_backfill=BackfillResult(action=BackfillAction.FORWARD_ONLY),
        ),
        DetectRenamedModelTestCase(
            description="rename with changed SQL follows the model's own replay policy",
            previous_definition="SELECT 2 AS order_id",
            config_values={"replay_on_change": "bounded-3d"},
            expected_query_changed=True,
            expected_backfill=BackfillResult(action=BackfillAction.BOUNDED, duration="3d"),
        ),
        DetectRenamedModelTestCase(
            description="rename with changed SQL defaults to forward only",
            previous_definition="SELECT 2 AS order_id",
            config_values={},
            expected_query_changed=True,
            expected_backfill=BackfillResult(action=BackfillAction.FORWARD_ONLY),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_renamed_model_when_detecting_changes_then_reports_rename_and_own_query_change(
    test_case: DetectRenamedModelTestCase,
) -> None:
    metadata_case: DetectModelMetadataTestCase = DetectModelMetadataTestCase(
        description="renamed model",
        config_values=test_case.config_values,
        schema_columns=(),
        deps=(),
        function_local_hashes={},
        previous_metadata_json="{}",
        expected_change_kind=ChangeKind.RENAMED,
    )
    snapshot: WarehouseSnapshot = build_snapshot_for_metadata_test_case(metadata_case)
    handover: Fingerprint = replace(
        snapshot.fingerprints.models["orders"],
        definition=test_case.previous_definition,
        definition_hash=compute_query_hash(query_sql=test_case.previous_definition, dialect=None),
    )

    result: ChangeDetectionResult = detect_model_changes(
        model=build_model_from_metadata_test_case(metadata_case),
        snapshot=replace(
            snapshot,
            existing_relations={},
            fingerprints=WarehouseFingerprints(models={"orders": handover}),
            renamed_models=frozenset({"orders"}),
        ),
        sql_analysis_enabled=False,
        query_change_tracking=True,
        full_refresh=False,
    )

    assert (result.change_kind, result.query_changed, result.backfill) == (
        ChangeKind.RENAMED,
        test_case.expected_query_changed,
        test_case.expected_backfill,
    )
