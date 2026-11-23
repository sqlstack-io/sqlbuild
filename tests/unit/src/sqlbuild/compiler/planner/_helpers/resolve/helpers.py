"""Test helpers for resolve helper tests."""

from __future__ import annotations

from pathlib import Path

from sqlbuild._native import (
    CompiledModel,
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    CompileModelConfig,
    ModelPlanContext,
    SourceEntry,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledModelView


def build_cursor_intrinsic_model(*, config_values: dict[str, object]) -> CompiledModelView:
    """Build a cursor incremental model that projects both cursor intrinsics."""

    return CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="events"),
        deps=(),
        name="events",
        relative_path=Path("sql/models/events.sql"),
        query_sql="SELECT __cursor_start() AS batch_start, __cursor_end() AS batch_end",
        config=CompileModelConfig(values=config_values),
        destination=CompiledRelationLocation(
            database=None,
            schema="main",
            name="events",
            qualified_name="main.events",
        ),
    )


def build_empty_model_plan_context(
    *,
    model_locations: dict[str, CompiledRelationLocation] | None = None,
    seed_locations: dict[str, CompiledRelationLocation] | None = None,
    source_map: dict[str, SourceEntry] | None = None,
) -> ModelPlanContext:
    """Build a model-resolution context with only the given relations."""

    return ModelPlanContext(
        model_locations=model_locations or {},
        models_by_name={},
        functions_by_name={},
        seed_locations=seed_locations or {},
        function_locations={},
        source_map=source_map or {},
        source_warehouse_columns={},
        star_exclude_keyword="EXCLUDE",
    )


def build_target(qualified: str | None, name: str) -> CompiledRelationLocation:
    """Build a minimal target for deferred tests."""

    return CompiledRelationLocation(database=None, schema=None, name=name, qualified_name=qualified)


class BracketUdfCallAdapter(DuckDbAdapter):
    def render_udf_call(self, *, target: str, call_suffix_sql: str) -> str:
        arguments_sql: str = call_suffix_sql.removeprefix("(").removesuffix(")")
        return f"{target}[{arguments_sql}]"


class BracketTableFunctionCallAdapter(DuckDbAdapter):
    def render_table_function_call(self, *, target: str, call_suffix_sql: str) -> str:
        arguments_sql: str = call_suffix_sql.removeprefix("(").removesuffix(")")
        return f"TABLE({target}[{arguments_sql}])"


def build_cursor_models(
    *, model_cursor_types: tuple[tuple[str, str | None], ...]
) -> tuple[CompiledModelView, ...]:
    """Build models named by `model_cursor_types`; a None type builds a non-cursor table."""

    return tuple(
        CompiledModel(
            key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=name),
            deps=(),
            name=name,
            relative_path=Path(f"sql/models/{name}.sql"),
            query_sql="SELECT 1",
            config=CompileModelConfig(values=_cursor_model_config_values(cursor_type=cursor_type)),
            destination=CompiledRelationLocation(
                database=None, schema="staging", name=name, qualified_name=f"staging.{name}"
            ),
        )
        for name, cursor_type in model_cursor_types
    )


def _cursor_model_config_values(*, cursor_type: str | None) -> dict[str, object]:
    table: dict[str, object] = {"materialized": "table"}
    incremental: dict[str, object] = {
        "materialized": "incremental",
        "cursor": "updated_at",
        "cursor_type": cursor_type,
    }
    return (table, incremental)[cursor_type is not None]
