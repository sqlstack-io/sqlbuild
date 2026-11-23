"""Compile small DuckDB projects for Rule-fix verification tests."""

from __future__ import annotations

from dataclasses import replace
from pathlib import Path

from sqlbuild._native import (
    build_compiled_project,
    compiled_model_record,
    discover_project_inputs,
    project_with,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView

_PROJECT_TOML: str = 'name = "orders"\nadapter = "duckdb"\n[rules]\nselect = []\n'


def compile_models(
    *, project_dir: Path, models: tuple[tuple[str, str], ...]
) -> CompiledProjectView:
    """Write and compile the models."""

    (project_dir / "sqlbuild_project.toml").write_text(_PROJECT_TOML, encoding="utf-8")
    (project_dir / "sql" / "models").mkdir(exist_ok=True)
    for name, sql in models:
        (project_dir / "sql" / "models" / name).write_text(sql, encoding="utf-8")
    return build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter=DuckDbAdapter(),
    )


def without_lineage(project: CompiledProjectView) -> CompiledProjectView:
    """The same compilation with column lineage unavailable for every model."""

    return project_with(
        project=project,
        models=tuple(
            replace(compiled_model_record(model), fast_lineage_columns=None)
            for model in project.models
        ),
    )
