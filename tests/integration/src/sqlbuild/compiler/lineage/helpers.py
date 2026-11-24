from __future__ import annotations

import random
from collections import Counter
from collections.abc import Callable
from dataclasses import replace
from pathlib import Path
from typing import cast

import pytest

import sqlbuild._native as native_module
from sqlbuild._native import (
    ColumnLineageMode,
    ProjectColumnLineage,
    build_project_graph,
    compiled_model_record,
    discover_project_inputs,
    project_with,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView

_PROJECT_TOML: str = (
    'name = "orders_lineage"\nadapter = "duckdb"\n\n[connection]\ndatabase = "orders.duckdb"\n'
)
_SOURCES: str = """sources:
  - name: raw_orders
    description: Orders feed.
    expression: >-
      (SELECT 1 AS order_id, 10 AS customer_id, CAST(5 AS DOUBLE) AS amount, 'placed' AS status)
    columns:
      - name: order_id
        type: INTEGER
      - name: customer_id
        type: INTEGER
      - name: amount
        type: DOUBLE
      - name: status
        type: VARCHAR
  - name: raw_customers
    description: Customers feed.
    expression: >-
      (SELECT 10 AS customer_id, 'Ada' AS Customer_Name, 'east' AS region)
    columns:
      - name: customer_id
        type: INTEGER
      - name: Customer_Name
        type: VARCHAR
      - name: region
        type: VARCHAR
"""
_SEEDS: str = """seeds:
  - name: regions
    description: Region labels.
    columns:
      - name: region
        type: VARCHAR
      - name: region_label
        type: VARCHAR
"""
_INITIAL_RELATIONS: dict[str, tuple[str, ...]] = {
    '__source("raw_orders")': ("order_id", "customer_id", "amount", "status"),
    '__source("raw_customers")': ("customer_id", "Customer_Name", "region"),
    '__seed("regions")': ("region", "region_label"),
}
_OPT_OUT_SHARE: float = 0.25
_OPT_OUTS: tuple[str, str] = ("", ", sql_analysis false")

type _Template = Callable[
    [random.Random, list[tuple[str, tuple[str, ...]]]], tuple[str, tuple[str, ...]]
]


def _star(
    rng: random.Random, inputs: list[tuple[str, tuple[str, ...]]]
) -> tuple[str, tuple[str, ...]]:
    relation, columns = inputs[0]
    return f"SELECT * FROM {relation}", columns


def _qualified_star(
    rng: random.Random, inputs: list[tuple[str, tuple[str, ...]]]
) -> tuple[str, tuple[str, ...]]:
    (left, left_columns), (right, right_columns) = inputs[0], inputs[-1]
    extra: str = rng.choice(right_columns)
    sql: str = (
        f"SELECT a.*, b.{extra} AS joined_{extra.lower()}, 1 AS flag\n"
        f"FROM {left} a\nJOIN {right} b ON TRUE"
    )
    return sql, (*left_columns, f"joined_{extra.lower()}", "flag")


def _nested_ctes(
    rng: random.Random, inputs: list[tuple[str, tuple[str, ...]]]
) -> tuple[str, tuple[str, ...]]:
    relation, columns = inputs[0]
    picked: list[str] = rng.sample(list(columns), k=min(2, len(columns)))
    sql: str = (
        f"WITH base AS (\n  SELECT {', '.join(picked)} FROM {relation}\n),\n"
        "layered AS (\n  SELECT * FROM base\n)\n"
        f"SELECT {picked[0]} AS key_value, COUNT(*) AS row_count\nFROM layered\nGROUP BY 1"
    )
    return sql, ("key_value", "row_count")


def _union(
    rng: random.Random, inputs: list[tuple[str, tuple[str, ...]]]
) -> tuple[str, tuple[str, ...]]:
    (left, left_columns), (right, right_columns) = inputs[0], inputs[-1]
    operator: str = rng.choice(("UNION ALL", "UNION", "EXCEPT", "INTERSECT"))
    sql: str = (
        f"SELECT {left_columns[0]} AS shared_key, 'left' AS side FROM {left}\n"
        f"{operator}\nSELECT {right_columns[0]}, 'right' FROM {right}"
    )
    return sql, ("shared_key", "side")


def _union_stars(
    rng: random.Random, inputs: list[tuple[str, tuple[str, ...]]]
) -> tuple[str, tuple[str, ...]]:
    relation, columns = inputs[0]
    sql: str = f"SELECT * FROM {relation}\nUNION ALL\nSELECT t.* FROM {relation} t"
    return sql, columns


def _quoted(
    rng: random.Random, inputs: list[tuple[str, tuple[str, ...]]]
) -> tuple[str, tuple[str, ...]]:
    relation, columns = inputs[0]
    column: str = rng.choice(columns)
    sql: str = (
        f'SELECT "{column}" AS "Mixed_Case", UPPER(CAST(Src.{column.upper()} AS VARCHAR)) AS Up\n'
        f"FROM {relation} AS Src"
    )
    return sql, ("Mixed_Case", "Up")


def _expressions(
    rng: random.Random, inputs: list[tuple[str, tuple[str, ...]]]
) -> tuple[str, tuple[str, ...]]:
    relation, columns = inputs[0]
    column: str = rng.choice(columns)
    sql: str = (
        f"SELECT CAST({column} AS VARCHAR) AS as_text, {column} IS NULL AS is_missing,\n"
        f"  'fixed' AS literal_value, COUNT({column}) OVER () AS total_rows,\n"
        f"  (SELECT 1) AS scalar_one\nFROM {relation}"
    )
    return sql, ("as_text", "is_missing", "literal_value", "total_rows", "scalar_one")


def _branching(
    rng: random.Random, inputs: list[tuple[str, tuple[str, ...]]]
) -> tuple[str, tuple[str, ...]]:
    relation, columns = inputs[0]
    first, last = columns[0], columns[-1]
    sql: str = (
        f"SELECT CASE WHEN {first} IS NULL THEN 'none' ELSE 'some' END AS presence,\n"
        f"  CASE {first} WHEN {last} THEN {first} END AS matched,\n"
        f"  COALESCE({first}, {last}) AS filled, {first} IN ({last}, {first}) AS listed,\n"
        f"  ROW_NUMBER() OVER (PARTITION BY {last} ORDER BY {first}) AS position\n"
        f"FROM {relation}"
    )
    return sql, ("presence", "matched", "filled", "listed", "position")


def _subquery(
    rng: random.Random, inputs: list[tuple[str, tuple[str, ...]]]
) -> tuple[str, tuple[str, ...]]:
    relation, columns = inputs[0]
    column: str = rng.choice(columns)
    sql: str = f"SELECT s.{column}, s.* FROM (SELECT * FROM {relation}) s"
    return sql, columns


_TEMPLATES: tuple[_Template, ...] = (
    _star,
    _qualified_star,
    _nested_ctes,
    _union,
    _union_stars,
    _quoted,
    _expressions,
    _branching,
    _subquery,
)


def generated_lineage_files(*, rng: random.Random, model_count: int) -> dict[str, str]:
    """A project of sources, a seed and models drawing on every lineage template."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": _PROJECT_TOML,
        "sql/sources/raw.yml": _SOURCES,
        "sql/seeds/regions.yml": _SEEDS,
        "sql/seeds/regions.csv": "region,region_label\neast,East\n",
    }
    relations: list[tuple[str, tuple[str, ...]]] = list(_INITIAL_RELATIONS.items())
    for index in range(model_count):
        template: _Template = _TEMPLATES[index % len(_TEMPLATES)]
        inputs: list[tuple[str, tuple[str, ...]]] = rng.sample(relations, k=2)
        sql, columns = template(rng, inputs)
        name: str = f"orders_model_{index}"
        opt_out: str = _OPT_OUTS[rng.random() < _OPT_OUT_SHARE]
        files[f"sql/models/layer_{index % 3}/{name}.sql"] = (
            f'MODEL (description "Generated orders model {index}"{opt_out});\n\n{sql}\n'
        )
        relations.append((f'__ref("{name}")', columns))
    return files


def compiled_project(*, project_dir: Path, files: dict[str, str]) -> CompiledProjectView:
    """Write and compile a project, keeping any diagnostics."""

    for relative_path, contents in files.items():
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")
    return build_project_graph(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter=DuckDbAdapter(),
    ).project


def without_compact_facts(project: CompiledProjectView) -> CompiledProjectView:
    """The project with every model sent through the parse fallback."""

    return project_with(
        project=project,
        models=tuple(
            replace(compiled_model_record(model), fast_lineage_columns=None)
            for model in project.models
        ),
    )


def lineage_views(
    *,
    project: CompiledProjectView,
    dialect: str | None,
    model_names: frozenset[str] | None,
) -> tuple[list[object], list[object]]:
    """Each public view of the native fast lineage: its name and value."""

    lineage: ProjectColumnLineage = cast(
        ProjectColumnLineage,
        fast_column_lineage(project=project, dialect=dialect, model_names=model_names),
    )
    return _public_views(project=project, lineage=lineage)


def fast_column_lineage(
    *,
    project: CompiledProjectView,
    dialect: str | None,
    model_names: frozenset[str] | None,
) -> ProjectColumnLineage | None:
    """The native fast lineage graph, or None without SQL analysis."""

    return native_module.build_project_column_lineage(
        project, dialect, ColumnLineageMode.FAST, model_names
    )


def _public_views(
    *, project: CompiledProjectView, lineage: ProjectColumnLineage
) -> tuple[list[object], list[object]]:
    names: list[object] = []
    values: list[object] = []
    for model in project.models:
        name: str = model.name
        views: tuple[tuple[str, object], ...] = (
            ("has_model", lineage.has_model(name)),
            ("has_star", lineage.model_has_star(name)),
            ("edge_count", lineage.edge_count_targeting(name)),
            ("targeting", lineage.edges_targeting(name)),
            ("sourced", lineage.edges_sourced_from(name)),
        )
        for view, value in views:
            names.append(f"{name}.{view}")
            values.append(value)
    names.extend(("models", "edges"))
    values.extend((list(lineage.models.items()), lineage.edges))
    return names, values


def lineage_log_records(caplog: pytest.LogCaptureFixture) -> list[tuple[str, str, object]]:
    """Captured records as logger, message and the logged parse error."""

    return [
        (record.name, record.getMessage(), record.__dict__.get("sqlbuild_error"))
        for record in caplog.records
    ]


def fast_lineage_work(
    *, project: CompiledProjectView, model_names: frozenset[str] | None
) -> Counter[str]:
    """Count the selected models fast lineage parses and those whose root star it expands."""

    names: frozenset[str] = model_names or frozenset(model.name for model in project.models)
    return Counter(
        parse=sum(
            model.name in names and model.fast_lineage_columns is None for model in project.models
        ),
        star=sum(
            model.name in names
            and model.fast_lineage_columns is not None
            and model.fast_lineage_has_star
            and not model.fast_lineage_star_resolved
            for model in project.models
        ),
    )
