"""Test helpers for pipeline helper tests."""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path

from sqlbuild._native import (
    CompiledModel,
    CompiledObjectKey,
    CompiledProject,
    CompiledRelationLocation,
    CompiledResourceType,
    CompileModelConfig,
    Fingerprint,
)
from sqlbuild.adapter.contract.models import RelationInfo
from sqlbuild.compiler.compile.types import (
    CompiledModelView,
    CompiledProjectView,
)


def build_single_model_project(
    *,
    logical_schema: str | None,
    logical_database: str | None,
    physical_schema: str | None,
    physical_database: str | None,
) -> CompiledProjectView:
    """Build a project with one model for deferred target testing."""

    qualified: str | None = (None, f"{physical_schema}.test_model")[physical_schema is not None]

    model: CompiledModelView = CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="test_model"),
        deps=(),
        name="test_model",
        relative_path=Path("models/test_model.sql"),
        query_sql="SELECT 1",
        config=CompileModelConfig(),
        destination=CompiledRelationLocation(
            database=physical_database,
            schema=physical_schema,
            name="test_model",
            qualified_name=qualified,
            logical_schema=logical_schema,
            logical_database=logical_database,
        ),
    )
    return CompiledProject(
        run_id="test",
        effective_target_name="dev",
        effective_connection={},
        effective_vars={},
        models=(model,),
    )


def build_previous_python_identity_map(
    *, previous_version_hash: str | None, current_version_hash: str
) -> dict[tuple[str, str], Fingerprint]:
    resolved_version_hash: str | None = {
        "current": current_version_hash,
    }.get(str(previous_version_hash), previous_version_hash)
    identity_map: dict[tuple[str, str], Fingerprint] = {
        ("task", "prepare_orders"): Fingerprint(
            node_type="task",
            node_name="prepare_orders",
            target_database=None,
            target_schema="analytics",
            target_name=None,
            run_id="run_001",
            definition_hash="definition",
            version_hash=resolved_version_hash or "",
            schema_fingerprint="schema",
            definition='{"source_text": "def prepare_orders(ctx):\\n    return old_helper()\\n"}',
            metadata_json=(
                '{"dependencies": [{"module": "tasks.helpers", '
                '"qualname": "old_helper", "source_path": "python/tasks/helpers.py", '
                '"source_text": "def old_helper():\\n    return 1\\n"}]}'
            ),
            ts=datetime(2026, 1, 15, 12, 0, 0),
        )
    }
    return ({}, identity_map)[previous_version_hash is not None]


@dataclass
class ListingAdapter:
    """An adapter double that lists its relations per database and records each listing."""

    relations: tuple[RelationInfo, ...]
    listed: list[tuple[str | None, tuple[str, ...]]] = field(default_factory=list)

    def list_relations(
        self, *, connection: object, database: str | None, schemas: tuple[str, ...]
    ) -> tuple[RelationInfo, ...]:
        """The relations of `schemas` in `database`."""

        self.listed.append((database, schemas))
        return tuple(
            filter(
                lambda relation: relation.database == database and relation.schema in schemas,
                self.relations,
            )
        )


def deferred_location(*, database: str | None, schema: str, name: str) -> CompiledRelationLocation:
    """One deferred relation location."""

    return CompiledRelationLocation(
        database=database,
        schema=schema,
        name=name,
        qualified_name=f"{database}.{schema}.{name}",
        logical_schema=schema,
        logical_database=database,
    )
