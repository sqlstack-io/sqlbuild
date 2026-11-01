from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pytest

from sqlbuild._native import (
    FINGERPRINT_TABLE_NAME,
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    Fingerprint,
    FingerprintSet,
    MaterializationType,
    ModelPlanEntry,
    PlanAction,
    PlanReason,
    SeedCsvSettings,
    SeedPlanEntry,
)
from sqlbuild.adapter.contract.models import RelationInfo
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter


class CloneFingerprintAdapter(DuckDbAdapter):
    adapter_name: str = "test"

    def execute(self, connection: object, sql: str) -> object:
        del connection, sql
        return object()

    def relation_exists(self, *args: object, **kwargs: object) -> bool:
        del args, kwargs
        return True

    def list_relations(
        self,
        connection: object,
        *,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[RelationInfo, ...]:
        del connection, names
        return tuple(
            RelationInfo(
                database=database,
                schema=schema,
                name=FINGERPRINT_TABLE_NAME,
                relation_type="base table",
            )
            for schema in (schemas or ())
        )

    def render_qualified_name(self, **kwargs: object) -> str:
        return ".".join(str(value) for value in kwargs.values()).replace("None.", "")

    def render_read_latest_fingerprints_sql(self, **kwargs: object) -> str:
        del kwargs
        return "select * from fingerprints"

    def render_framework_type(self, type_name: object) -> str:
        return str(type_name)

    def render_create_fingerprint_table_sql(self, **kwargs: object) -> str:
        del kwargs
        return "create table fingerprints"

    def render_create_fingerprint_index_sqls(self, **kwargs: object) -> tuple[str, ...]:
        del kwargs
        return ()


def build_model_entry(
    name: str, *, schema: str, materialization: MaterializationType
) -> ModelPlanEntry:
    return ModelPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=name),
        name=name,
        relative_path=Path(f"models/{name}.sql"),
        materialization_type=materialization,
        action=PlanAction.CREATE_TABLE,
        reason=PlanReason.NO_CHANGE,
        destination=build_location(name=name, schema=schema),
        fingerprint_query_sql="select 1",
        resolved_sql="select 1",
        logical_ddl="",
    )


def build_seed_entry(name: str, *, schema: str) -> SeedPlanEntry:
    return SeedPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.SEED, name=name),
        name=name,
        destination=build_location(name=name, schema=schema),
        file_path=Path(f"seeds/{name}.csv"),
        columns=(),
        csv_settings=SeedCsvSettings(),
    )


def build_location(*, name: str, schema: str) -> CompiledRelationLocation:
    return CompiledRelationLocation(
        database=None,
        schema=schema,
        name=name,
        qualified_name=f"{schema}.{name}",
    )


def patch_fingerprint_io(
    monkeypatch: pytest.MonkeyPatch,
    *,
    read_latest: Callable[..., FingerprintSet],
    write: Callable[..., None],
) -> None:
    from sqlbuild.executor.clone._helpers import fingerprinting as fingerprinting_module

    monkeypatch.setattr(fingerprinting_module, "read_latest_fingerprints", read_latest)
    monkeypatch.setattr(fingerprinting_module, "write_fingerprints", write)


def build_fingerprint(node_type: str, node_name: str) -> Fingerprint:
    from datetime import UTC, datetime

    return Fingerprint(
        node_type=node_type,
        node_name=node_name,
        target_database=None,
        target_schema="prod",
        target_name=node_name,
        run_id="source-run",
        definition_hash=f"definition-{node_name}",
        version_hash=f"version-{node_name}",
        schema_fingerprint="schema",
        definition="definition",
        metadata_json="{}",
        ts=datetime.now(tz=UTC),
    )
