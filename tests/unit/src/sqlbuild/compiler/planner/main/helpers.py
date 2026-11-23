from __future__ import annotations

from dataclasses import fields
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from sqlbuild._native import (
    CompiledModel,
    CompiledObjectKey,
    CompiledProject,
    CompiledRelationLocation,
    CompiledResourceType,
    CompileModelConfig,
    DeferralInputs,
    Dialect,
    Fingerprint,
    PlannerOverrides,
    PlannerPolicies,
    PlannerSelection,
    PlanOutput,
    SchemaColumn,
    SchemaModelEntry,
    SelectionStalenessGraph,
    SelectionStalenessNodeKey,
    build_execution_plan,
    build_version_identity_metadata_json,
    compute_query_hash,
    live_reads,
    write_fingerprint,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import (
    CompiledModelView,
    CompiledProjectView,
)


def model_definition_hash(project: CompiledProjectView, name: str) -> str:
    models_by_name: dict[str, CompiledModelView] = {model.name: model for model in project.models}
    model: CompiledModelView = models_by_name[name]
    return compute_query_hash(query_sql=model.query_sql, dialect=project.sql_analysis_dialect)


def build_execution_plan_from_kwargs(**kwargs: Any) -> PlanOutput:
    """Adapt flat planner kwargs to the grouped build_execution_plan inputs."""

    def grouped(model: type) -> dict[str, Any]:
        names: frozenset[str] = frozenset(field.name for field in fields(model))
        return {name: kwargs.pop(name) for name in names & kwargs.keys()}

    selection: PlannerSelection = PlannerSelection(**grouped(PlannerSelection))
    overrides: PlannerOverrides = PlannerOverrides(**grouped(PlannerOverrides))
    deferral: DeferralInputs = DeferralInputs(**grouped(DeferralInputs))
    policies: PlannerPolicies = PlannerPolicies(**grouped(PlannerPolicies))
    adapter: Any = kwargs.pop("adapter")
    connection: Any = kwargs.pop("connection")
    return build_execution_plan(
        dialect=Dialect(adapter),
        reads=live_reads(adapter=adapter, connection=connection),
        selection=selection,
        overrides=overrides,
        deferral=deferral,
        policies=policies,
        **kwargs,
    )


def build_protected_schema_replay_project() -> CompiledProjectView:
    """Build a full_refresh false incremental whose declared column forces a full replay."""

    model: CompiledModelView = CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="order_history"),
        deps=(),
        name="order_history",
        relative_path=Path("sql/models/order_history.sql"),
        query_sql="SELECT 1 AS id, 2 AS amount",
        config=CompileModelConfig(
            values={
                "materialized": "incremental",
                "incremental_strategy": "append",
                "replay_on_change": "full",
                "full_refresh": False,
            }
        ),
        destination=CompiledRelationLocation(
            database=None, schema="main", name="order_history", qualified_name="main.order_history"
        ),
        schema_entry=SchemaModelEntry(
            name="order_history",
            columns=(
                SchemaColumn(name="id", type="INTEGER"),
                SchemaColumn(name="amount", type="INTEGER"),
            ),
        ),
    )
    return CompiledProject(
        run_id="protected-rebuild-run",
        effective_target_name=None,
        effective_connection={},
        effective_vars={},
        models=(model,),
    )


def create_protected_model_state(*, adapter: DuckDbAdapter, connection: Any) -> None:
    """Create the protected model's table and a fingerprint from an older header."""

    adapter.execute(connection=connection, sql="CREATE TABLE main.order_history (id INTEGER)")
    query_sql: str = build_protected_schema_replay_project().models[0].query_sql
    _ = write_fingerprint(
        connection=connection,
        execute=adapter.execute,
        database=None,
        schema="main",
        fingerprint=Fingerprint(
            node_type="model",
            node_name="order_history",
            target_database=None,
            target_schema="main",
            target_name="order_history",
            run_id="previous-run",
            definition_hash=compute_query_hash(query_sql=query_sql, dialect=None),
            schema_fingerprint="",
            definition=query_sql,
            metadata_json=build_version_identity_metadata_json(
                model_name="order_history",
                config_values={
                    "materialized": "incremental",
                    "incremental_strategy": "append",
                    "full_refresh": False,
                    "on_schema_change": "fail",
                },
            ),
            ts=datetime(2026, 9, 1, tzinfo=UTC),
        ),
        render_qualified_name=adapter.render_qualified_name,
        render_framework_type=adapter.render_framework_type,
    )


def build_chain_staleness_graph(*, depth: int) -> SelectionStalenessGraph:
    """Build a model chain whose most upstream model changed and only the head runs."""

    keys: list[SelectionStalenessNodeKey] = [
        SelectionStalenessNodeKey(resource_type="model", name=f"m{index}") for index in range(depth)
    ]
    head: str = keys[-1].name
    return SelectionStalenessGraph(
        upstream_deps={key: (upstream,) for key, upstream in zip(keys[1:], keys[:-1], strict=True)},
        selected_model_names=frozenset({head}),
        run_model_names=frozenset({head}),
        run_seed_names=frozenset(),
        run_source_names=frozenset(),
        changed_model_names=frozenset({"m0"}),
        changed_seed_names=frozenset(),
        changed_source_names=frozenset(),
    )
