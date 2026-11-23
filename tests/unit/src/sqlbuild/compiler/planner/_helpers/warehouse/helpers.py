"""Helpers for warehouse snapshot planner tests."""

from __future__ import annotations

from pathlib import Path
from types import SimpleNamespace
from typing import cast

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledProject,
    CompiledResourceType,
    CompiledSource,
    Dialect,
    DiscoveredSourceFile,
    PlannerRelationsContext,
    PlannerScope,
    SourceEntry,
    check_selected_source_tables_exist,
    live_reads,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.models import ColumnInfo
from sqlbuild.compiler.compile.types import CompiledSourceView


def column_names(columns: tuple[ColumnInfo, ...]) -> tuple[str, ...]:
    """Return inspected column names in order."""

    return tuple(column.name for column in columns)


def check_one_source_table(
    *,
    adapter: BaseAdapter,
    connection: object,
    source: SourceEntry,
    listed_source_names: frozenset[str],
) -> None:
    """Run the S405 check for one model that reads one source."""

    model_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.MODEL, name="orders"
    )
    source_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.SOURCE, name=source.name
    )
    compiled_source: CompiledSourceView = CompiledSource(
        key=source_key,
        deps=(),
        name=source.name,
        source_entry=source,
        source_file=DiscoveredSourceFile(
            file_path=Path("/project/sources/raw.yml"),
            relative_path=Path("sql/sources/raw.yml"),
            contents=f"sources:\n  - name: {source.name}\n",
            source_entries=(source,),
        ),
    )
    check_selected_source_tables_exist(
        project=cast(CompiledProject, SimpleNamespace(sources=(compiled_source,))),
        dialect=Dialect(adapter),
        reads=live_reads(adapter=adapter, connection=connection),
        scope=PlannerScope(
            upstream_deps={model_key: (source_key,)},
            downstream_deps={source_key: (model_key,)},
            all_keys={"orders": model_key, source.name: source_key},
            models_by_name={},
            selected_keys=frozenset({model_key, source_key}),
            execution_order=(source_key, model_key),
        ),
        relations=PlannerRelationsContext(
            model_locations={},
            seed_locations={},
            function_locations={},
            source_map={source.name: source},
            source_read_map={source.name: source},
            python_source_read_map={},
            source_warehouse_columns={},
            star_exclude_keyword="EXCLUDE",
            listed_source_names=listed_source_names,
        ),
    )
