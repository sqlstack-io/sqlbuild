"""Test helpers for plan formatter tests."""

from __future__ import annotations

from pathlib import Path

from sqlbuild._native import (
    BackfillAction,
    BackfillResult,
    ColumnRenameHint,
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    CursorBounds,
    CursorInputRelation,
    DiscoveredProviderUsage,
    FunctionPlanEntry,
    MaterializationType,
    MicrobatchLimitAction,
    ModelPlanEntry,
    PlanAction,
    PlanOutput,
    PlanProviderUsage,
    PlanReason,
    PlanWarning,
    SchemaChangeKind,
    SchemaColumnSource,
    SchemaFinding,
    SeedCsvSettings,
    SeedPlanEntry,
    SourceLoadPlanEntry,
    SourceWriteStrategy,
    TableTypePlanEntry,
    WarningSeverity,
)
from sqlbuild.compiler.compile.types import FunctionLanguage


def build_model_entry(
    *,
    name: str,
    action: PlanAction = PlanAction.CREATE_TABLE,
    reason: PlanReason = PlanReason.FIRST_RUN,
    materialization_type: MaterializationType = MaterializationType.TABLE,
    backfill_action: BackfillAction = BackfillAction.FULL,
    backfill_duration: str | None = None,
    previous_query_sql: str | None = None,
    fingerprint_metadata_json: str | None = None,
    previous_metadata_json: str | None = None,
    fingerprint_version_hash: str | None = None,
    previous_version_hash: str | None = None,
    cursor_column: str | None = None,
    cursor_type: str | None = None,
    cursor_grain: str | None = None,
    cursor_bounds: CursorBounds | None = None,
    cursor_input_relations: tuple[CursorInputRelation, ...] = (),
    batch_size: str | None = None,
    microbatch_range: CursorBounds | None = None,
    start_cursor_override: str | None = None,
    end_cursor_override: str | None = None,
    incremental_strategy: str | None = None,
    incremental_mode: str | None = None,
    snapshot_strategy: str | None = None,
    observed_at_column: str | None = None,
    historical_input: str | None = None,
    schema_findings: tuple[SchemaFinding, ...] = (),
    changed_functions: tuple[str, ...] = (),
    custom_materialization_name: str | None = None,
    query_changed: bool = False,
    config_changed: bool = False,
    microbatch_limit: int | None = None,
    microbatch_limit_count: int | None = None,
    microbatch_limit_action: MicrobatchLimitAction | None = None,
    microbatch_limit_warning: str | None = None,
    column_rename_hints: tuple[ColumnRenameHint, ...] = (),
) -> ModelPlanEntry:
    """Build a minimal ModelPlanEntry for formatter tests."""

    return ModelPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=name),
        name=name,
        relative_path=Path(f"sql/models/{name}.sql"),
        materialization_type=materialization_type,
        action=action,
        reason=reason,
        destination=CompiledRelationLocation(
            database=None, schema="main", name=name, qualified_name=f"main.{name}"
        ),
        fingerprint_query_sql=f"SELECT * FROM {name}",
        resolved_sql=f"SELECT * FROM {name}",
        logical_ddl=f"CREATE TABLE main.{name} AS (SELECT * FROM {name})",
        incremental_strategy=incremental_strategy,
        incremental_mode=incremental_mode,
        snapshot_strategy=snapshot_strategy,
        observed_at_column=observed_at_column,
        historical_input=historical_input,
        cursor_column=cursor_column,
        cursor_type=cursor_type,
        cursor_grain=cursor_grain,
        cursor_bounds=cursor_bounds,
        cursor_input_relations=cursor_input_relations,
        batch_size=batch_size,
        microbatch_range=microbatch_range,
        start_cursor_override=start_cursor_override,
        end_cursor_override=end_cursor_override,
        previous_query_sql=previous_query_sql,
        fingerprint_metadata_json=fingerprint_metadata_json,
        previous_metadata_json=previous_metadata_json,
        fingerprint_version_hash=fingerprint_version_hash,
        previous_version_hash=previous_version_hash,
        schema_findings=schema_findings,
        backfill=BackfillResult(action=backfill_action, duration=backfill_duration),
        changed_functions=changed_functions,
        custom_materialization_name=custom_materialization_name,
        query_changed=query_changed,
        config_changed=config_changed,
        microbatch_limit=microbatch_limit,
        microbatch_limit_count=microbatch_limit_count,
        microbatch_limit_action=microbatch_limit_action,
        microbatch_limit_warning=microbatch_limit_warning,
        column_rename_hints=column_rename_hints,
    )


def build_plan_output(
    *,
    model_entries: tuple[ModelPlanEntry, ...] = (),
    seed_entries: tuple[SeedPlanEntry, ...] = (),
    function_entries: tuple[FunctionPlanEntry, ...] = (),
    source_load_entries: tuple[SourceLoadPlanEntry, ...] = (),
    warnings: tuple[PlanWarning, ...] = (),
    provider_usages: tuple[PlanProviderUsage, ...] = (),
    metadata: dict[str, object] | None = None,
    table_type_entries: tuple[TableTypePlanEntry, ...] = (),
) -> PlanOutput:
    """Build a minimal PlanOutput for formatter tests."""

    selected_keys: frozenset[CompiledObjectKey] = frozenset(
        e.key for e in (*model_entries, *seed_entries, *function_entries)
    )
    return PlanOutput(
        execution_order=tuple(e.key for e in (*function_entries, *model_entries, *seed_entries)),
        model_entries=model_entries,
        seed_entries=seed_entries,
        function_entries=function_entries,
        source_load_entries=source_load_entries,
        selected_keys=selected_keys,
        warnings=warnings,
        provider_usages=provider_usages,
        metadata=metadata or {},
        table_type_entries=table_type_entries,
    )


def build_table_type_entry(*, model_name: str) -> TableTypePlanEntry:
    """Build one permanent-desired Snowflake table-type entry for formatter tests."""

    return TableTypePlanEntry(
        model_name=model_name,
        destination=CompiledRelationLocation(
            database="analytics",
            schema="marts",
            name=model_name,
            qualified_name=f"analytics.marts.{model_name}",
        ),
        copy_name=f"{model_name}__sqlbuild_table_type",
        desired_type="permanent",
        actual_type="transient",
        source="target",
        downgrade=False,
        downgrade_policy="warn",
    )


def build_plan_provider_usage(
    *,
    provider_name: str = "marker_provider",
    consumer_kind: str = "loader",
    consumer_name: str = "raw_orders",
    parameter_name: str = "marker_provider",
    annotation_class_name: str | None = "MarkerProvider",
    annotation_module: str | None = "python.providers.marker",
) -> PlanProviderUsage:
    return PlanProviderUsage(
        provider_name=provider_name,
        consumer_kind=consumer_kind,
        consumer_name=consumer_name,
        parameter_name=parameter_name,
        annotation_class_name=annotation_class_name,
        annotation_module=annotation_module,
    )


def build_discovered_provider_usage(
    *,
    provider_name: str = "marker_provider",
    parameter_name: str = "marker_provider",
    annotation_class_name: str | None = "MarkerProvider",
    annotation_module: str | None = "python.providers.marker",
) -> DiscoveredProviderUsage:
    return DiscoveredProviderUsage(
        provider_name=provider_name,
        parameter_name=parameter_name,
        annotation_class_name=annotation_class_name,
        annotation_module=annotation_module,
    )


def build_source_load_entry(
    *,
    name: str,
    write_strategy: SourceWriteStrategy | None = SourceWriteStrategy.TABLE,
    cursor_column: str | None = None,
    unique_key: tuple[str, ...] = (),
    is_reload: bool = False,
) -> SourceLoadPlanEntry:
    """Build a minimal SourceLoadPlanEntry for formatter tests."""

    return SourceLoadPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.SOURCE, name=name),
        name=name,
        loader=f"{name}_loader",
        destination=name,
        write_strategy=write_strategy,
        cursor_column=cursor_column,
        unique_key=unique_key,
        is_reload=is_reload,
    )


def build_seed_entry(*, name: str, reason: PlanReason = PlanReason.FIRST_RUN) -> SeedPlanEntry:
    """Build a minimal SeedPlanEntry for formatter tests."""

    return SeedPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.SEED, name=name),
        name=name,
        destination=CompiledRelationLocation(
            database=None, schema="main", name=name, qualified_name=f"main.{name}"
        ),
        file_path=Path(f"sql/seeds/{name}.csv"),
        columns=(),
        csv_settings=SeedCsvSettings(),
        reason=reason,
    )


def build_function_entry(
    *,
    name: str,
    language: FunctionLanguage = FunctionLanguage.SQL,
    reason: PlanReason = PlanReason.NO_CHANGE,
    previous_query_sql: str | None = None,
) -> FunctionPlanEntry:
    """Build a minimal FunctionPlanEntry for formatter tests."""

    return FunctionPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.UDF, name=name),
        name=name,
        relative_path=Path(f"sql/functions/{language.value}/{name}.sql"),
        destination=CompiledRelationLocation(
            database=None, schema="main", name=name, qualified_name=f"main.{name}"
        ),
        fingerprint_destination=CompiledRelationLocation(
            database=None, schema="main", name=name, qualified_name=f"main.{name}"
        ),
        arguments=(),
        returns="BOOLEAN",
        body_sql="return True",
        fingerprint_query_sql="return True",
        language=language,
        previous_query_sql=previous_query_sql,
        reason=reason,
    )


def build_warning(
    *,
    model_name: str | None,
    message: str,
    severity: WarningSeverity = WarningSeverity.WARNING,
    code: str | None = None,
) -> PlanWarning:
    """Build a PlanWarning for formatter tests."""

    return PlanWarning(model_name=model_name, severity=severity, message=message, code=code)


def build_schema_finding(
    *,
    kind: SchemaChangeKind,
    column_name: str,
    expected_type: str | None = None,
    actual_type: str | None = None,
) -> SchemaFinding:
    """Build a SchemaFinding for formatter tests."""

    return SchemaFinding(
        kind=kind,
        column_name=column_name,
        source=SchemaColumnSource.YML,
        expected_type=expected_type,
        actual_type=actual_type,
    )
