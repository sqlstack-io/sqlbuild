from __future__ import annotations

from datetime import UTC, datetime
from pathlib import Path

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    CostContext,
    MaterializationType,
    ModelPlanEntry,
    PlanAction,
    PlanReason,
    SourceFreshnessIdentity,
    SourceFreshnessRecord,
    build_source_freshness_create_table_sql,
)
from sqlbuild.adapter.contract.models import ColumnInfo
from sqlbuild.adapter.contract.types import FrameworkType
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.cost.models import CostResourceContext


class RecordingAdapter:
    def __init__(self) -> None:
        self.insert_count: int = 0
        self.executed_sql: list[str] = []
        self.cost_contexts: list[CostResourceContext | None] = []

    def connect(self, _config: dict[str, object]) -> object:
        return object()

    def close(self, _connection: object) -> None:
        return None

    def execute(self, *, connection: object, sql: str) -> None:
        del connection
        self.executed_sql.append(sql)
        self.cost_contexts.append(CostContext.current())
        self.insert_count += int(sql.strip().upper().startswith("INSERT"))

    def render_qualified_name(
        self, *, database: str | None, schema: str | None, name: str
    ) -> str | None:
        rendered_names: dict[tuple[bool, bool], str] = {
            (False, False): name,
            (False, True): f"{schema}.{name}",
            (True, False): f"{database}.{name}",
            (True, True): f"{database}.{schema}.{name}",
        }
        return rendered_names[(database is not None, schema is not None)]

    def render_framework_type(self, framework_type: FrameworkType) -> str:
        return framework_type.value

    def get_columns(self, **_scope: object) -> tuple[ColumnInfo, ...]:
        return (ColumnInfo(name="event_id", type="VARCHAR"),)

    def render_add_columns(
        self, *, destination: str, columns: tuple[ColumnInfo, ...]
    ) -> tuple[str, ...]:
        return DuckDbAdapter().render_add_columns(destination=destination, columns=columns)

    def render_create_source_freshness_table_sql(self, *, database: str | None, schema: str) -> str:
        return build_source_freshness_create_table_sql(
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
            render_framework_type=self.render_framework_type,
        )

    def render_create_source_freshness_index_sqls(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        del database, schema
        return ()

    def render_insert_source_freshness_records_sql(
        self,
        *,
        database: str | None,
        schema: str,
        records: tuple[SourceFreshnessRecord, ...],
    ) -> str:
        return DuckDbAdapter().render_insert_source_freshness_records_sql(
            database=database,
            schema=schema,
            records=records,
        )


def model_entry(name: str) -> ModelPlanEntry:
    return ModelPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=name),
        name=name,
        relative_path=Path(f"models/{name}.sql"),
        materialization_type=MaterializationType.TABLE,
        action=PlanAction.CREATE_TABLE,
        reason=PlanReason.NO_CHANGE,
        destination=CompiledRelationLocation(
            database=None,
            schema="main",
            name=name,
            qualified_name=f"main.{name}",
        ),
        fingerprint_query_sql="SELECT 1",
        resolved_sql="SELECT 1",
        logical_ddl="",
    )


def source_freshness_identity() -> SourceFreshnessIdentity:
    return SourceFreshnessIdentity(
        source_name="raw_orders",
        target_database=None,
        target_schema=None,
        target_name=None,
    )


def source_freshness_record(
    *,
    run_id: str = "planning",
    data_version: str | None = "1",
    data_version_hash: str = "hash",
) -> SourceFreshnessRecord:
    return SourceFreshnessRecord(
        source_name="raw_orders",
        target_database=None,
        target_schema=None,
        target_name=None,
        run_id=run_id,
        strategy="sql",
        value_kind="integer",
        data_version=data_version,
        data_version_hash=data_version_hash,
        observed_at=datetime(2026, 1, 1, 0, 0, 0, tzinfo=UTC),
    )
