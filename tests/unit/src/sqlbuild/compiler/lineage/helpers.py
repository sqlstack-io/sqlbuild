from __future__ import annotations

from pathlib import Path

import sqlbuild._native as native
from sqlbuild._native import (
    ColumnLineageMode,
    CompiledModel,
    CompiledObjectKey,
    CompiledProject,
    CompiledRelationLocation,
    CompiledResourceType,
    CompiledSeed,
    CompileModelConfig,
    DiscoveredSchemaFile,
    DiscoveredSeedFile,
    InferredColumn,
    ProjectColumnLineage,
    SchemaColumn,
    SchemaSeedEntry,
    SettingsConfig,
)
from sqlbuild.compiler.compile.types import (
    CompiledModelView,
    CompiledProjectView,
    CompiledSeedView,
)


def make_compiled_project(
    *,
    models: tuple[CompiledModelView, ...],
    seeds: tuple[CompiledSeedView, ...] = (),
    sql_analysis_enabled: bool = True,
) -> CompiledProjectView:
    return CompiledProject(
        run_id="test-run",
        effective_target_name=None,
        effective_connection={},
        effective_vars={},
        settings=SettingsConfig(sql_analysis=sql_analysis_enabled),
        models=models,
        seeds=seeds,
    )


def make_compiled_model(
    *,
    name: str,
    query_sql: str,
    inferred_columns: tuple[str, ...] | None,
) -> CompiledModelView:
    return CompiledModel(
        key=CompiledObjectKey(CompiledResourceType.MODEL, name),
        deps=(),
        name=name,
        relative_path=Path(f"sql/models/{name}.sql"),
        query_sql=query_sql,
        config=CompileModelConfig(),
        destination=CompiledRelationLocation(
            database=None,
            schema=None,
            name=name,
            qualified_name=name,
        ),
        inferred_columns=(
            None,
            tuple(InferredColumn(column_name) for column_name in (inferred_columns or ())),
        )[inferred_columns is not None],
    )


def make_compiled_seed(*, name: str, columns: tuple[str, ...]) -> CompiledSeedView:
    return CompiledSeed(
        key=CompiledObjectKey(CompiledResourceType.SEED, name),
        deps=(),
        name=name,
        seed_file=DiscoveredSeedFile(
            file_path=Path(f"sql/seeds/{name}.csv"),
            relative_path=Path(f"sql/seeds/{name}.csv"),
        ),
        schema_entry=SchemaSeedEntry(
            name=name,
            columns=tuple(SchemaColumn(column_name) for column_name in columns),
        ),
        schema_file=DiscoveredSchemaFile(
            file_path=Path("sql/seeds/schema.yml"),
            relative_path=Path("sql/seeds/schema.yml"),
            contents="",
            model_entries=(),
            seed_entries=(),
        ),
        destination=CompiledRelationLocation(
            database=None,
            schema=None,
            name=name,
            qualified_name=name,
        ),
    )


def edge_label(source_name: str, source_column: str, target_name: str, target_column: str) -> str:
    return f"{source_name}.{source_column}->{target_name}.{target_column}"


def make_chain_models(
    *, chain_length: int, unrelated_model_count: int
) -> tuple[CompiledModelView, ...]:
    root: CompiledModelView = make_compiled_model(
        name="orders_0",
        query_sql="SELECT 1 AS order_id, 10 AS amount",
        inferred_columns=("order_id", "amount"),
    )
    chain: tuple[CompiledModelView, ...] = tuple(
        make_compiled_model(
            name=f"orders_{index}",
            query_sql=f'SELECT order_id, amount + 1 AS amount FROM __ref("orders_{index - 1}")',
            inferred_columns=("order_id", "amount"),
        )
        for index in range(1, chain_length)
    )
    unrelated: tuple[CompiledModelView, ...] = tuple(
        make_compiled_model(
            name=f"inventory_{index}",
            query_sql="SELECT 1 AS product_id, 5 AS quantity",
            inferred_columns=("product_id", "quantity"),
        )
        for index in range(unrelated_model_count)
    )
    return (root, *chain, *unrelated)


def project_column_lineage(
    *,
    project: CompiledProjectView,
    dialect: str | None = None,
    mode: ColumnLineageMode = ColumnLineageMode.RICH,
    model_names: frozenset[str] | None = None,
) -> ProjectColumnLineage | None:
    """The project's native column lineage graph, rich by default."""

    return native.build_project_column_lineage(project, dialect, mode, model_names)
