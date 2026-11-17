"""Test helpers for build output formatter tests."""

from __future__ import annotations

from pathlib import Path
from typing import Any
from unittest.mock import Mock

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    DiscoveredLoaderFunction,
    MaterializationType,
    ModelPlanEntry,
    PlanAction,
    PlanOutput,
    PlanReason,
    SeedCsvSettings,
    SeedPlanEntry,
    SourceColumnEntry,
    SourceEntry,
    SourceLoadPlanEntry,
    SourceWriteStrategy,
)
from sqlbuild.adapter.contract.classes.retention_adapter import RetentionAdapterMixin
from sqlbuild.compiler.auditing.types import (
    AuditAttachmentKind,
    AuditOutcome,
    AuditRunScope,
    AuditSeverity,
)
from sqlbuild.executor.auditing.models import AuditExecutionResult


def build_audit_result(
    *,
    name: str,
    outcome: AuditOutcome,
    row_count: int = 0,
    column_name: str | None = None,
    run_scope_phase: AuditRunScope = AuditRunScope.FINAL,
) -> AuditExecutionResult:
    return AuditExecutionResult(
        audit_name=name,
        audit_definition_name="test_audit",
        attachment_kind=AuditAttachmentKind.MODEL,
        severity={
            AuditOutcome.PASS: AuditSeverity.ERROR,
            AuditOutcome.WARN: AuditSeverity.WARN,
            AuditOutcome.ERROR: AuditSeverity.ERROR,
        }[outcome],
        outcome=outcome,
        row_count=row_count,
        executed_sql="SELECT 1",
        run_scope_phase=run_scope_phase,
        attached_target_name="test_model",
        attached_column_name=column_name,
    )


def build_model_plan_entry(
    *,
    name: str,
    materialization_type: MaterializationType = MaterializationType.TABLE,
    action: PlanAction = PlanAction.CREATE_TABLE,
    reason: PlanReason = PlanReason.FIRST_RUN,
    resolved_sql: str = "SELECT 1",
    incremental_strategy: str | None = None,
    snapshot_strategy: str | None = None,
    observed_at_column: str | None = None,
    historical_input: str | None = None,
) -> ModelPlanEntry:
    return ModelPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=name),
        name=name,
        relative_path=Path(f"models/{name}.sql"),
        materialization_type=materialization_type,
        action=action,
        reason=reason,
        destination=CompiledRelationLocation(
            database=None, schema="main", name=name, qualified_name=f"main.{name}"
        ),
        fingerprint_query_sql="SELECT 1",
        resolved_sql=resolved_sql,
        logical_ddl=f"CREATE TABLE main.{name} AS SELECT 1",
        incremental_strategy=incremental_strategy,
        snapshot_strategy=snapshot_strategy,
        observed_at_column=observed_at_column,
        historical_input=historical_input,
    )


def build_seed_plan_entry(*, name: str) -> SeedPlanEntry:
    return SeedPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.SEED, name=name),
        name=name,
        destination=CompiledRelationLocation(
            database=None, schema="main", name=name, qualified_name=f"main.{name}"
        ),
        file_path=Path(f"seeds/{name}.csv"),
        columns=(),
        csv_settings=SeedCsvSettings(),
    )


def build_source_load_plan_output(*, source_name: str, loader_name: str) -> PlanOutput:
    source_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.SOURCE,
        name=source_name,
    )
    return PlanOutput(
        source_load_entries=(
            SourceLoadPlanEntry(
                key=source_key,
                name=source_name,
                loader=loader_name,
                destination=source_name,
            ),
        ),
        source_map={
            source_name: SourceEntry(
                name=source_name,
                loader=loader_name,
                write_strategy=SourceWriteStrategy.TABLE,
                columns=(
                    SourceColumnEntry(name="id", type="INTEGER"),
                    SourceColumnEntry(name="status", type="VARCHAR"),
                ),
            )
        },
    )


def source_node_test_loader(_ctx: object) -> list[dict[str, object]]:
    return [{"id": 1, "status": "loaded"}]


def failing_source_node_test_loader(_ctx: object) -> list[dict[str, object]]:
    raise RuntimeError("loader failed intentionally")


def build_discovered_source_loader(*, loader_name: str) -> DiscoveredLoaderFunction:
    return DiscoveredLoaderFunction(
        file_path=Path("python/loaders/raw.py"),
        relative_path=Path("python/loaders/raw.py"),
        name=loader_name,
        function=source_node_test_loader,
    )


def build_failing_discovered_source_loader(*, loader_name: str) -> DiscoveredLoaderFunction:
    return DiscoveredLoaderFunction(
        file_path=Path("python/loaders/raw.py"),
        relative_path=Path("python/loaders/raw.py"),
        name=loader_name,
        function=failing_source_node_test_loader,
    )


def fetch_rows_or_empty(connection: Any, sql: str) -> tuple[tuple[object, ...], ...]:
    try:
        return tuple(connection.execute(sql).fetchall())
    except Exception:
        return ()


def build_plan_output(
    *,
    model_entries: tuple[ModelPlanEntry, ...] = (),
    seed_entries: tuple[SeedPlanEntry, ...] = (),
) -> PlanOutput:
    return PlanOutput(
        model_entries=model_entries,
        seed_entries=seed_entries,
    )


def retention_mock_adapter(**attributes: Any) -> Mock:
    """Return a Mock adapter whose batched retention hooks use the real default implementations."""

    adapter: Mock = Mock(**attributes)
    adapter.inspect_retentions.side_effect = lambda *, connection, requests: (
        RetentionAdapterMixin.inspect_retentions(adapter, connection=connection, requests=requests)
    )
    adapter.retention_state_from_relation.side_effect = lambda *, request, relation: (
        RetentionAdapterMixin.retention_state_from_relation(
            adapter, request=request, relation=relation
        )
    )
    return adapter


def batched_retention_request_ids(*, adapter: Mock) -> tuple[tuple[str, ...], ...]:
    """Return the request IDs passed to each batched retention inspection call."""

    batches: list[tuple[str, ...]] = []
    for batch in adapter.inspect_retentions.call_args_list:
        batches.append(tuple(request.request_id for request in batch.kwargs["requests"]))
    return tuple(batches)


def model_key(name: str) -> CompiledObjectKey:
    return CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=name)


def build_model_key_edges(
    *, edges: dict[str, tuple[str, ...]]
) -> dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]:
    key_edges: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]] = {}
    name: str
    children: tuple[str, ...]
    for name, children in edges.items():
        key_edges[model_key(name)] = tuple(model_key(child) for child in children)
    return key_edges
