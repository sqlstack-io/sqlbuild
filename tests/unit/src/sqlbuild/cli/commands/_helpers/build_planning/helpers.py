"""Test helpers for build CLI helper tests."""

from pathlib import Path

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    MaterializationType,
    ModelPlanEntry,
    PlanAction,
    PlanReason,
    PlanSafetyGates,
    RetentionDirection,
    RetentionPlanEntry,
    RetentionPlanPhase,
    TableTypePlanEntry,
)
from sqlbuild.adapter.contract.models import RetentionRequest
from sqlbuild.adapter.contract.types import RetentionScope


def build_snapshot_full_refresh_entry(
    *,
    name: str = "customer_snapshot",
    observed_at_column: str | None = None,
    snapshot_full_refresh: str | None = None,
) -> ModelPlanEntry:
    return ModelPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=name),
        name=name,
        relative_path=Path(f"sql/models/{name}.sql"),
        materialization_type=MaterializationType.SNAPSHOT,
        action=PlanAction.SNAPSHOT,
        reason=PlanReason.FULL_REFRESH,
        destination=CompiledRelationLocation(
            database=None,
            schema="main",
            name=name,
            qualified_name=f"main.{name}",
        ),
        fingerprint_query_sql="SELECT 1 AS id",
        resolved_sql="SELECT 1 AS id",
        logical_ddl=f"CREATE TABLE main.{name} AS SELECT 1 AS id",
        observed_at_column=observed_at_column,
        snapshot_full_refresh=snapshot_full_refresh,
    )


def build_table_type_entry(
    *,
    name: str = "orders",
    downgrade: bool = True,
    policy: str = "require_confirmation",
    desired_type: str = "transient",
    actual_type: str = "permanent",
) -> TableTypePlanEntry:
    return TableTypePlanEntry(
        model_name=name,
        destination=CompiledRelationLocation(
            database=None,
            schema="main",
            name=name,
            qualified_name=f"main.{name}",
        ),
        copy_name=f"__sqb_type_swap__{name}",
        desired_type=desired_type,
        actual_type=actual_type,
        source="model",
        downgrade=downgrade,
        downgrade_policy=policy,
    )


def build_retention_entry(
    *,
    name: str = "orders",
    direction: RetentionDirection = RetentionDirection.DECREASE,
    policy: str = "deny",
) -> RetentionPlanEntry:
    return RetentionPlanEntry(
        request=RetentionRequest(
            request_id=name,
            scope=RetentionScope.RELATION,
            database=None,
            schema="main",
            name=name,
            desired_days=7,
        ),
        model_names=(name,),
        actual_days=90,
        effective_days=90,
        source="target",
        direction=direction,
        phase=RetentionPlanPhase.POST,
        statements=("SELECT 1",),
        decrease_policy=policy,
    )


def build_policy_gates(
    *,
    allow_snapshot_full_refresh: bool = False,
    allow_table_type_downgrade: bool = False,
    allow_retention_decrease: bool = False,
) -> PlanSafetyGates:
    return PlanSafetyGates(
        allow_missing_migration_origin=False,
        allow_snapshot_full_refresh=allow_snapshot_full_refresh,
        allow_table_type_downgrade=allow_table_type_downgrade,
        allow_retention_decrease=allow_retention_decrease,
        missing_origin_help=MISSING_ORIGIN_BUILD_HELP,
        snapshot_full_refresh_help=SNAPSHOT_FULL_REFRESH_BUILD_HELP,
        table_type_downgrade_help=TABLE_TYPE_DOWNGRADE_BUILD_HELP,
        retention_decrease_help=RETENTION_DECREASE_BUILD_HELP,
        execution_limit_note=EXECUTION_LIMIT_BUILD_NOTE,
    )


EXECUTION_LIMIT_BUILD_NOTE: str = "No warehouse changes were made."
MISSING_ORIGIN_BUILD_HELP: str = (
    "Pass --allow-missing-migration-origin to confirm in non-interactive runs."
)
RETENTION_DECREASE_BUILD_HELP: str = (
    "Pass --allow-retention-decrease to confirm in non-interactive runs."
)
SNAPSHOT_FULL_REFRESH_BUILD_HELP: str = (
    "Pass --allow-snapshot-full-refresh to confirm in non-interactive runs."
)
TABLE_TYPE_DOWNGRADE_BUILD_HELP: str = (
    "Pass --allow-table-type-downgrade to confirm in non-interactive runs."
)
