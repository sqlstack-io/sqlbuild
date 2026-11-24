from __future__ import annotations

from pathlib import Path
from typing import Any, ClassVar

from sqlbuild._native import (
    CompiledModel,
    CompiledObjectKey,
    CompiledProject,
    CompiledRelationLocation,
    CompiledResourceType,
    CompileModelConfig,
    FunctionPlanEntry,
    MaterializationType,
    ModelPlanEntry,
    PlanAction,
    PlanReason,
    ResolvedTimeTravelRetention,
    TimeTravelRetentionSource,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.models import RelationInfo
from sqlbuild.compiler.compile.types import (
    CompiledModelView,
    CompiledProjectView,
)


class FakeCloneAdapter(BaseAdapter):
    adapter_name: ClassVar[str] = "clone-test"

    def __init__(
        self,
        *,
        supports_zero_copy: bool,
        origin_is_transient: bool = False,
        origin_names: tuple[str, ...] = ("fact_orders",),
    ) -> None:
        self._supports_zero_copy = supports_zero_copy
        self._origin_is_transient = origin_is_transient
        self._origin_names = origin_names
        self.executed_statements: list[str] = []
        self.used_connections: list[Any] = []

    def supports_zero_copy_clone(self) -> bool:
        return self._supports_zero_copy

    def connect(self, config: dict[str, Any]) -> object:
        del config
        return object()

    def close(self, connection: Any) -> None:
        del connection

    def _execute(self, connection: Any, sql: str) -> None:
        self.used_connections.append(connection)
        self.executed_statements.append(sql)

    def relation_exists(
        self,
        connection: Any,
        *,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> bool:
        del connection, database, schema, name
        return True

    def list_relations(
        self,
        connection: Any,
        *,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[RelationInfo, ...]:
        self.used_connections.append(connection)
        del database, names
        relations: list[RelationInfo] = []
        for schema in schemas or ():
            for origin_name in self._origin_names:
                relations.append(
                    RelationInfo(
                        database=None,
                        schema=schema,
                        name=origin_name,
                        relation_type="base table",
                        is_transient=self._origin_is_transient,
                    )
                )
        return tuple(relations)

    def render_drop(self, *, destination: str, if_exists: bool = True) -> tuple[str, ...]:
        exists_clause: str = {True: " IF EXISTS", False: ""}[if_exists]
        return (f"DROP TABLE{exists_clause} {destination}",)

    def render_clone(
        self,
        *,
        origin: str,
        destination: str,
        hard_copy: bool = False,
        origin_is_transient: bool = False,
    ) -> tuple[str, ...]:
        table_kind: str = {True: "TRANSIENT TABLE", False: "TABLE"}[origin_is_transient]
        statements: dict[bool, str] = {
            True: f"CREATE OR REPLACE TABLE {destination} AS SELECT * FROM {origin}",
            False: f"CREATE {table_kind} {destination} CLONE {origin}",
        }
        return (statements[hard_copy],)

    def render_create_function(self, **kwargs: Any) -> tuple[str, ...]:
        return (f"CREATE FUNCTION {kwargs['destination']}",)


def build_clone_model_entry(*, schema: str, name: str) -> ModelPlanEntry:
    return ModelPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=name),
        name=name,
        relative_path=Path("models") / f"{name}.sql",
        materialization_type=MaterializationType.TABLE,
        action=PlanAction.CREATE_TABLE,
        reason=PlanReason.FIRST_RUN,
        destination=CompiledRelationLocation(
            database=None,
            schema=schema,
            name=name,
            qualified_name=f"{schema}.{name}",
        ),
        fingerprint_query_sql="SELECT 1",
        resolved_sql="SELECT 1",
        logical_ddl="MODEL (description 'Test model.', materialized table);",
    )


def build_clone_function_entry(
    *, schema: str, name: str, fingerprint_schema: str | None = None
) -> FunctionPlanEntry:
    destination: CompiledRelationLocation = CompiledRelationLocation(
        database=None,
        schema=schema,
        name=name,
        qualified_name=f"{schema}.{name}",
    )
    return FunctionPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.UDF, name=name),
        name=name,
        relative_path=Path("sql/functions/sql") / f"{name}.sql",
        destination=destination,
        arguments=(),
        returns="INTEGER",
        body_sql="1",
        fingerprint_query_sql="body=1",
        fingerprint_destination=CompiledRelationLocation(
            database=None,
            schema=fingerprint_schema or schema,
            name=name,
            qualified_name=f"{fingerprint_schema or schema}.{name}",
        ),
    )


def build_clone_retention_project() -> CompiledProjectView:
    return CompiledProject(
        run_id="clone-retention-run",
        effective_target_name="test",
        effective_connection={},
        effective_vars={},
        models=(
            _build_retention_model(name="orders", schema="analytics"),
            _build_retention_model(name="customers", schema="customer_mart"),
        ),
    )


def _build_retention_model(*, name: str, schema: str) -> CompiledModelView:
    key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.MODEL,
        name=name,
    )
    return CompiledModel(
        key=key,
        deps=(),
        name=name,
        relative_path=Path(f"sql/models/{name}.sql"),
        query_sql=f"SELECT 1 AS {name}_id",
        config=CompileModelConfig(
            time_travel_retention=ResolvedTimeTravelRetention(
                desired_days=7,
                unmanaged=False,
                source=TimeTravelRetentionSource.MODEL,
            )
        ),
        destination=CompiledRelationLocation(
            database="warehouse",
            schema=schema,
            name=name,
            qualified_name=f"warehouse.{schema}.{name}",
        ),
    )
