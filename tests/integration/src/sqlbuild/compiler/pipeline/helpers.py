from __future__ import annotations

import json
from collections.abc import Callable
from pathlib import Path
from typing import TYPE_CHECKING, Any

import sqlbuild._native as native_module
from sqlbuild._native import (
    CollectionRendering,
    CompileAdapterContext,
    CompiledLineageColumnFact,
    CompilePipelineOptions,
    CompilePipelineResult,
    DiscoveredProjectInputs,
    build_compile_inputs,
    build_compiled_project,
    build_manifest,
    discover_project_inputs,
    run_compile_pipeline,
    semantic_shapes,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.postgres.classes.postgres_adapter import PostgresAdapter
from sqlbuild.compiler.compile.types import CompiledModelView, CompiledProjectView
from sqlbuild.runtime.contracts.models import ConnectionHooks

if TYPE_CHECKING:
    from sqlbuild.compiler.compile.types import AuditInput

_REPOSITORY_ROOT: Path = Path(__file__).resolve().parents[6]
_ANALYSIS_CACHE_METRICS: tuple[str, ...] = (
    "analysis_batch_cache_hits",
    "analysis_entry_cache_hits",
    "analysis_cache_misses",
    "analysis_cache_bypasses",
)
_SCHEMA_FIXTURE_PATH: Path = (
    Path(__file__).resolve().parents[5] / "fixtures" / "dbt_manifest_v12_schema.json"
)
_RESHAPED_STAR_PROJECT_TOML: str = 'name = "orders"\nadapter = "duckdb"\n[rules]\nselect = []\n'
RESHAPED_STAR_UPSTREAM_MODELS: dict[str, str] = {
    "staged_orders": "SELECT 1 AS customer_id, 'books' AS category, 10 AS amount",
    "wide_orders": "SELECT 1 AS customer_id, 2 AS books, 3 AS games",
}
_ANALYSIS_THREAD_NAME: str = "sqlbuild-analys"
_MODEL_ANALYSIS_THREAD_PREFIX: str = "sqlbuild-model-analysis-"
_HOLD_SECONDS: float = 0.3
_REACQUIRE_DELAY_SECONDS: float = 0.05
_INTERRUPT_TIMEOUT_SECONDS: float = 30.0
_INTERRUPT_DELAY_SECONDS: float = 0.15
SEMANTIC_BINDING_PROJECT_TOML: str = 'name = "semantic_binding"\nadapter = "duckdb"\n'
_AUDIT_FACTORY_ADAPTER_CONTEXT: CompileAdapterContext = CompileAdapterContext(
    value_renderer=DuckDbAdapter(),
    collection_rendering=CollectionRendering.VALUE_LIST,
    python_functions_inherit_default_namespace=True,
    sql_lexical_syntax=DuckDbAdapter.sql_lexical_syntax,
)


def compile_audits_for_project(*, project_dir: Path) -> tuple[AuditInput, ...]:
    """Compile one project's audit inputs with the real DuckDB compiler path."""

    return build_compile_inputs(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter_context=_AUDIT_FACTORY_ADAPTER_CONTEXT,
        run_id="integration_run",
    ).audit_inputs


def audit_input_projection(*, audits: tuple[AuditInput, ...]) -> tuple[tuple[object, ...], ...]:
    """Project attachment-relevant fields for direct/generated equivalence."""

    return tuple(
        (
            audit.name,
            audit.audit_file.relative_path,
            audit.sql_body,
            audit.attached_target_kind,
            audit.attached_target_name,
            audit.attached_column_name,
            audit.severity,
            audit.run_scope,
            audit.always_run,
        )
        for audit in audits
    )


def run_compile_pipeline_for_project(
    *,
    project_dir: Path,
    adapter: BaseAdapter,
    defer_to: str | None = None,
    select: tuple[str, ...] = (),
    exclude: tuple[str, ...] = (),
    on_progress: Callable[[str], None] | None = None,
    resolve_python_run_selectors: bool = False,
    no_sql_validation: bool = True,
) -> CompilePipelineResult:
    """Discover a project and run the real compile pipeline."""

    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=project_dir)
    return run_compile_pipeline(
        discovered_inputs=discovered_inputs,
        adapter=adapter,
        options=CompilePipelineOptions(
            no_sql_validation=no_sql_validation,
            defer_to=defer_to,
            select=select,
            exclude=exclude,
            resolve_python_run_selectors=resolve_python_run_selectors,
        ),
        hooks=ConnectionHooks(on_progress=on_progress),
    )


def build_manifest_for_pipeline_result(
    *,
    result: CompilePipelineResult,
    project_name: str,
    adapter_type: str,
) -> dict[str, object]:
    """Build the manifest artifact from a compile pipeline result and parse its JSON."""

    return json.loads(
        build_manifest(
            project=result.project,
            plan_output=result.plan_output,
            project_name=project_name,
            adapter_type=adapter_type,
            upstream_deps=result.plan_output.upstream_deps,
            downstream_deps=result.plan_output.downstream_deps,
        ).contents()
    )


def validate_manifest_against_dbt_schema(manifest: dict[str, object]) -> None:
    """Validate a manifest dict against the dbt v12 JSON schema."""

    from jsonschema import Draft202012Validator

    schema: dict[str, Any] = json.loads(_SCHEMA_FIXTURE_PATH.read_text(encoding="utf-8"))
    validator: Any = Draft202012Validator(schema)
    errors: list[str] = [e.message for e in validator.iter_errors(manifest)]
    assert errors == [], f"Manifest schema validation errors: {errors}"


def write_semantic_binding_project(
    *,
    project_dir: Path,
    upstream_sql: str,
    downstream_sql: str,
    project_toml: str = SEMANTIC_BINDING_PROJECT_TOML,
) -> None:
    _ = (project_dir / "sqlbuild_project.toml").write_text(project_toml, encoding="utf-8")
    models_dir: Path = project_dir / "models"
    models_dir.mkdir()
    _ = (models_dir / "upstream.sql").write_text(upstream_sql, encoding="utf-8")
    _ = (models_dir / "downstream.sql").write_text(downstream_sql, encoding="utf-8")


def compile_reshaped_star_model(
    *, project_dir: Path, query_sql: str
) -> tuple[CompiledProjectView, CompiledModelView]:
    """Compile one reshaped-star model over the shared upstream order models."""

    (project_dir / "sqlbuild_project.toml").write_text(
        _RESHAPED_STAR_PROJECT_TOML, encoding="utf-8"
    )
    models: Path = project_dir / "models"
    models.mkdir()
    for name, sql in RESHAPED_STAR_UPSTREAM_MODELS.items():
        (models / f"{name}.sql").write_text(
            f"MODEL (description 'Test model.', materialized table);\n{sql}", encoding="utf-8"
        )
    (models / "reshaped_orders.sql").write_text(
        f"MODEL (description 'Test model reshaped_orders.', materialized table);\n{query_sql}",
        encoding="utf-8",
    )
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter=DuckDbAdapter(),
    )
    models_by_name: dict[str, CompiledModelView] = {model.name: model for model in project.models}
    return project, models_by_name["reshaped_orders"]


def lineage_source_pairs(column: CompiledLineageColumnFact) -> frozenset[tuple[str, str]]:
    """Return one lineage column's upstream (resource, column) pairs."""

    return frozenset(
        (source.resource_name, source.column_name) for source in column.upstream_columns
    )


_SHARED_BINDING_UPSTREAM_SQL: str = (
    "MODEL (description 'Test model.', materialized view); SELECT 1 AS id, 2.5 AS amount"
)


def apply_warehouse_setup_sql(*, project_dir: Path, statements: tuple[str, ...]) -> None:
    """Create pre-existing warehouse objects in the project's file-backed DuckDB database."""

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(project_dir / "warehouse.duckdb"))
    try:
        statement: str
        for statement in statements:
            connection.execute(statement)
    finally:
        connection.close()


def shape_batches(*, reset: bool = False) -> tuple[int, int]:
    """Native shape batches the binding catalog's shape cache missed, and expressions they held."""

    work: dict[str, int] = native_module.compile_stage_work_info(reset=reset)
    return work.get("expression_shape_batches", 0), work.get("expression_shapes_inferred", 0)


_OFFLINE_POSTGRES_CONNECTION: dict[str, object] = {
    "host": "localhost",
    "database": "warehouse",
    "schema": "analytics",
}


def postgres_semantic_shapes(
    *, project_dir: Path, files: dict[str, str]
) -> dict[str, dict[str, str]]:
    """Compile `files` offline as a Postgres project and return every published model shape."""

    for relative_path, contents in files.items():
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter=PostgresAdapter(),
        resolved_connection=_OFFLINE_POSTGRES_CONNECTION,
    )
    return semantic_shapes(project=project)
