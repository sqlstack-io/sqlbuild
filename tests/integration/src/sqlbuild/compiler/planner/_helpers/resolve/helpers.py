"""Helpers for resolve integration tests."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any, cast

from sqlbuild._native import (
    BackfillAction,
    BackfillResult,
    CollectionRendering,
    CompileAdapterContext,
    CompiledModel,
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    CompileModelConfig,
    CompileSqlReference,
    CursorOverridePair,
    ModelCursorSnapshot,
    ModelPlanContext,
    SourceEntry,
    SqlReferenceKind,
    WarehouseSnapshot,
    assemble_project,
    build_compile_inputs,
    discover_project_inputs,
    resolve_model_sql,
)
from sqlbuild.adapter.contract.models import ColumnInfo, RelationInfo
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import (
    CompiledModelView,
    CompiledProjectView,
)


class OrchestrationOverrideAdapter(DuckDbAdapter):
    """Record orchestration callbacks and render executable custom relations and functions."""

    def __init__(self) -> None:
        self.calls: list[str] = []

    def render_qualified_name(self, *, database: str | None, schema: str | None, name: str) -> str:
        self.calls.append(f"relation:{schema}.{name}")
        return "staging.actual_orders"

    def render_table_function_call(self, *, target: str, call_suffix_sql: str) -> str:
        self.calls.append(f"function:{target}{call_suffix_sql}")
        return "(SELECT 9 AS order_id)"


class CursorLiteralOverrideAdapter(DuckDbAdapter):
    """Keep custom cursor rendering observable through complete planner execution."""

    def __init__(self) -> None:
        self.cursor_literals: list[tuple[str, str | None]] = []

    def render_cursor_bound_literal(self, *, value: str, cursor_type: str | None) -> str:
        self.cursor_literals.append((value, cursor_type))
        literal: str = super().render_cursor_bound_literal(value=value, cursor_type=cursor_type)
        return f"{literal} /* custom cursor literal */"


def compile_resolution_project(project_dir: Path) -> CompiledProjectView:
    """Compile a real model, dependent table function and SQL hook project."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": (
            'name = "resolution"\nadapter = "duckdb"\n'
            '[connection]\ndatabase = ":memory:"\n[defaults]\nschema = "staging"\n'
        ),
        "models/orders.sql": (
            'MODEL (description "Orders", alias "orders_alias"); SELECT 9 AS order_id'
        ),
        "functions/sql/order_lookup.sql": (
            'FUNCTION (description "Order lookup", arguments (id INTEGER), '
            "returns table (order_id INTEGER)); "
            'SELECT order_id FROM __ref("orders") WHERE order_id = id'
        ),
        "models/report.sql": (
            'MODEL (description "Report", pre_hooks [inline_sql(\'SELECT order_id '
            'FROM __ref("orders")\')]); SELECT o.order_id FROM __ref("orders") o '
            'JOIN __table_fn("order_lookup")(9) f ON o.order_id = f.order_id'
        ),
    }
    for path, text in files.items():
        destination: Path = project_dir / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(text)
    return assemble_project(
        inputs=build_compile_inputs(
            discovered_inputs=discover_project_inputs(project_dir=project_dir),
            adapter_context=CompileAdapterContext(
                value_renderer=DuckDbAdapter(),
                collection_rendering=CollectionRendering.VALUE_LIST,
                python_functions_inherit_default_namespace=True,
                sql_lexical_syntax=DuckDbAdapter.sql_lexical_syntax,
                sql_analysis_dialect=DuckDbAdapter.sql_analysis_dialect_name,
            ),
            run_id="resolution",
        ),
        skip_column_inference=True,
    )


class ResolutionOverrideAdapter(DuckDbAdapter):
    """Render an expression source using adapter-owned SQL at every boundary."""

    def render_source_expression_relation(self, *, expression: str) -> str:
        return '(SELECT 7 AS "OrderId")'

    def render_identifier(self, name: str) -> str:
        return '"' + name.replace('"', '""') + '"'

    def render_source_expression_cast(
        self, *, expression: str, target_type: str, alias: str
    ) -> str:
        return f"CAST({expression} AS {target_type}) + 1 AS {alias}"

    def render_source_expression_cast_subquery(
        self, *, source_relation: str, projections: tuple[str, ...]
    ) -> str:
        return f"(SELECT {', '.join(projections)} FROM {source_relation})"


def build_model(
    *,
    name: str,
    query_sql: str,
    config: dict[str, object],
    ref_names: tuple[str, ...],
) -> CompiledModelView:
    """Build a minimal CompiledModel for resolve integration tests."""

    raw_schema: object | None = config.get("schema")
    schema: str = cast(str, ("staging", raw_schema)[isinstance(raw_schema, str)])
    references: tuple[CompileSqlReference, ...] = tuple(
        CompileSqlReference(ref_kind=SqlReferenceKind.REF, ref_name=ref_name)
        for ref_name in ref_names
    )
    return CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=name),
        deps=(),
        name=name,
        relative_path=Path(f"models/{name}.sql"),
        query_sql=query_sql,
        config=CompileModelConfig(values=config),
        destination=CompiledRelationLocation(
            database=None,
            schema=schema,
            name=name,
            qualified_name=f"{schema}.{name}",
        ),
        references=references,
    )


@dataclass(frozen=True)
class _ResolveResult:
    """Result from resolve_and_execute."""

    resolved_sql: str
    rows: list[Any]
    column_types: dict[str, str]


def resolve_and_execute(
    *,
    model: CompiledModelView,
    snapshot: WarehouseSnapshot,
    model_locations: dict[str, CompiledRelationLocation],
    source_map: dict[str, SourceEntry],
    source_warehouse_columns: dict[str, tuple[ColumnInfo, ...]],
    connection: Any,
    full_refresh: bool = False,
    start_cursor_override: str | None = None,
    end_cursor_override: str | None = None,
    replaces_relation: bool = False,
) -> _ResolveResult:
    """Resolve model SQL and execute it against a real connection."""

    resolved_sql: str = resolve_model_sql(
        adapter=DuckDbAdapter(),
        model=model,
        snapshot=snapshot,
        context=ModelPlanContext(
            model_locations=model_locations,
            models_by_name={},
            functions_by_name={},
            seed_locations={},
            function_locations={},
            source_map=source_map,
            source_warehouse_columns=source_warehouse_columns,
            star_exclude_keyword="EXCLUDE",
        ),
        backfill=BackfillResult(action=BackfillAction.FORWARD_ONLY),
        full_refresh=full_refresh,
        cursor_overrides=CursorOverridePair(
            start_cursor_override=start_cursor_override,
            end_cursor_override=end_cursor_override,
        ),
        replaces_relation=replaces_relation,
    )

    result: Any = connection.execute(resolved_sql)
    rows: list[Any] = result.fetchall()
    column_types: dict[str, str] = {desc[0]: str(desc[1]) for desc in result.description}
    return _ResolveResult(resolved_sql=resolved_sql, rows=rows, column_types=column_types)


def build_empty_input_model(*, cursor_start: str | None) -> CompiledModelView:
    """Build a delete_insert cursor model reading the raw_events source."""

    config: dict[str, object] = {
        "materialized": "incremental",
        "incremental_strategy": "delete_insert",
        "cursor": "event_time",
        "cursor_type": "timestamp",
        "cursor_inputs": {"raw_events": "event_time"},
        "cursor_start": cursor_start,
    }
    return build_model(
        name="fact_events",
        query_sql='SELECT event_id, event_time FROM __source("raw_events")',
        config=config,
        ref_names=(),
    )


def build_empty_input_snapshot(
    *,
    target_max: str | None,
    target_relation: str | None,
    existing_relations: dict[str, RelationInfo],
) -> WarehouseSnapshot:
    """Build a planning snapshot in which the raw_events cursor input has no rows."""

    return WarehouseSnapshot(
        existing_relations=existing_relations,
        cursor_snapshots={
            "fact_events": ModelCursorSnapshot(
                target_max=target_max,
                upstream_mins=(),
                upstream_maxes=(),
                target_relation=target_relation,
                expected_watermark_count=1,
                unavailable_watermark_tags=("fact_events__raw_events__max",),
                empty_input_names=("raw_events.event_time",),
            )
        },
    )


def raw_events_source_map() -> dict[str, SourceEntry]:
    """Return the raw_events source declaration."""

    return {"raw_events": SourceEntry(name="raw_events", schema="staging", table="raw_events")}


def fact_events_relation(*, schema: str) -> dict[str, RelationInfo]:
    """Return an existing fact_events table in one schema, keyed as the planner keys it."""

    return {
        "fact_events": RelationInfo(
            database=None, schema=schema, name="fact_events", relation_type="table"
        )
    }
