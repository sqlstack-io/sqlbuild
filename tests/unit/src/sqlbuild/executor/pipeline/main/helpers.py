from __future__ import annotations

from pathlib import Path
from typing import ClassVar

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    FunctionPlanEntry,
    MaterializationType,
    ModelPlanEntry,
    PlanAction,
    PlanOutput,
    PlanReason,
    SeedPlanEntry,
    SourceEntry,
    SourceLoadPlanEntry,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.models import ColumnInfo
from sqlbuild.compiler.compile.types import FunctionLanguage
from sqlbuild.runtime.contracts.types import ExecutionResourceKind
from sqlbuild.spec.contracts.constants import DEFAULT_SEED_CSV_SETTINGS


class BuildSchemaPreflightAdapter(BaseAdapter):
    adapter_name: ClassVar[str] = "schema-preflight-test"

    def __init__(self) -> None:
        self.connections: list[object] = []
        self.closed_connections: list[object] = []
        self.prepared_schemas: list[tuple[str | None, str | None]] = []

    def connect(self, config: dict[str, object]) -> object:
        del config
        connection: object = object()
        self.connections.append(connection)
        return connection

    def _execute(self, connection: object, sql: str) -> object:
        del connection, sql
        return object()

    def close(self, connection: object) -> None:
        self.closed_connections.append(connection)

    def ensure_schema(
        self,
        connection: object,
        *,
        database: str | None,
        schema: str | None,
        statement_recorder: StatementRecorder,
    ) -> None:
        del connection, statement_recorder
        self.prepared_schemas.append((database, schema))


def build_schema_preflight_plan() -> PlanOutput:
    return PlanOutput(
        model_entries=(
            ModelPlanEntry(
                key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders"),
                name="orders",
                relative_path=Path("sql/models/orders.sql"),
                materialization_type=MaterializationType.TABLE,
                action=PlanAction.CREATE_TABLE,
                reason=PlanReason.FIRST_RUN,
                destination=_location(schema="dev", name="orders"),
                fingerprint_query_sql="SELECT 1 AS id",
                resolved_sql="SELECT 1 AS id",
                logical_ddl="",
            ),
        ),
        seed_entries=(
            SeedPlanEntry(
                key=CompiledObjectKey(resource_type=CompiledResourceType.SEED, name="statuses"),
                name="statuses",
                destination=_location(schema="dev", name="statuses"),
                file_path=Path("sql/seeds/statuses.csv"),
                columns=(ColumnInfo(name="id", type="INTEGER"),),
                csv_settings=DEFAULT_SEED_CSV_SETTINGS,
            ),
        ),
        source_load_entries=(
            SourceLoadPlanEntry(
                key=CompiledObjectKey(
                    resource_type=CompiledResourceType.SOURCE,
                    name="raw_orders",
                ),
                name="raw_orders",
                loader="load_orders",
                destination="raw.raw_orders",
                resource_kind=ExecutionResourceKind.SOURCE,
            ),
        ),
        function_entries=(
            FunctionPlanEntry(
                key=CompiledObjectKey(
                    resource_type=CompiledResourceType.UDF,
                    name="is_large_order",
                ),
                name="is_large_order",
                relative_path=Path("sql/functions/sql/is_large_order.sql"),
                destination=_location(schema="analytics", name="is_large_order"),
                arguments=(),
                returns="BOOLEAN",
                body_sql="amount > 100",
                fingerprint_query_sql="amount > 100",
                fingerprint_destination=_location(schema="state", name="is_large_order"),
                language=FunctionLanguage.SQL,
            ),
        ),
        source_map={
            "raw_orders": SourceEntry(name="raw_orders", schema="raw", table="raw_orders"),
        },
    )


def _location(*, schema: str, name: str) -> CompiledRelationLocation:
    return CompiledRelationLocation(
        database=None,
        schema=schema,
        name=name,
        qualified_name=f"{schema}.{name}",
    )
