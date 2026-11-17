"""Test helpers for warehouse snapshot integration tests."""

from __future__ import annotations

import threading
import time
from collections.abc import Callable
from dataclasses import dataclass, field, replace
from pathlib import Path
from typing import Any, ClassVar

from sqlbuild._native import (
    CompiledModel,
    CompiledObjectKey,
    CompiledProject,
    CompiledRelationLocation,
    CompiledResourceType,
    CompiledSeed,
    CompiledSource,
    CompileModelConfig,
    CompileSqlReference,
    Dialect,
    DiscoveredSchemaFile,
    DiscoveredSeedFile,
    DiscoveredSourceFile,
    SchemaColumn,
    SchemaModelEntry,
    SchemaSeedEntry,
    SourceEntry,
    SqlReferenceKind,
    WarehouseSnapshot,
    compiled_model_record,
    gather_warehouse_snapshot,
    live_reads,
    project_with,
)
from sqlbuild.adapter.contract.models import ColumnInfo
from sqlbuild.adapter.relations.main.open_inspection_catalog import open_inspection_catalog
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import (
    CompiledModelView,
    CompiledProjectView,
    CompiledSeedView,
    CompiledSourceView,
)


class RecordingDuckDbAdapter(DuckDbAdapter):
    """DuckDB adapter that records get_all_columns name filters for assertions."""

    def __init__(self) -> None:
        self.get_all_columns_names: list[tuple[str, ...] | None] = []

    def get_all_columns(
        self,
        connection: Any,
        *,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> dict[str, tuple[ColumnInfo, ...]]:
        self.get_all_columns_names.append(names)
        return super().get_all_columns(
            connection=connection,
            database=database,
            schemas=schemas,
            names=names,
        )


@dataclass(frozen=True)
class _IncrementalModelSpec:
    """Spec for building a test incremental model with cursor config."""

    name: str
    schema: str
    cursor: str
    ref_names: tuple[str, ...]
    extra_config: dict[str, object] = field(default_factory=dict)


class _FailEligibleCursorAdapter(DuckDbAdapter):
    def render_max_cursor_at_or_before(
        self,
        *,
        relation: str,
        cursor_column: str,
        maximum_allowed: str,
        cursor_type: str | None,
        is_date: bool,
    ) -> str:
        del relation, cursor_column, maximum_allowed, cursor_type, is_date
        return "SELECT missing_eligible_cursor_function()"


def build_project_with_targets(
    *,
    model_locations: dict[str, str | None] | None = None,
    model_deps: dict[str, tuple[str, ...]] | None = None,
    seed_locations: dict[str, str | None] | None = None,
    incremental_models: tuple[_IncrementalModelSpec, ...] = (),
    source_names: tuple[tuple[str, str, str], ...] = (),
) -> CompiledProjectView:
    """Build a minimal CompiledProject with explicit target schemas."""

    models: list[CompiledModelView] = []
    model_name: str
    target_schema: str | None
    for model_name, target_schema in (model_locations or {}).items():
        deps: tuple[CompiledObjectKey, ...] = tuple(
            CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=dep_name)
            for dep_name in (model_deps or {}).get(model_name, ())
        )
        references: tuple[CompileSqlReference, ...] = tuple(
            CompileSqlReference(ref_kind=SqlReferenceKind.REF, ref_name=dep_name)
            for dep_name in (model_deps or {}).get(model_name, ())
        )
        models.append(
            CompiledModel(
                key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=model_name),
                deps=deps,
                name=model_name,
                relative_path=Path(f"models/{model_name}.sql"),
                query_sql=f"SELECT * FROM {model_name}",
                config=CompileModelConfig(),
                destination=CompiledRelationLocation(
                    database=None,
                    schema=target_schema,
                    name=model_name,
                    qualified_name=(None, f"{target_schema}.{model_name}")[bool(target_schema)],
                ),
                references=references,
            )
        )

    spec: _IncrementalModelSpec
    for spec in incremental_models:
        references: tuple[CompileSqlReference, ...] = tuple(
            CompileSqlReference(ref_kind=SqlReferenceKind.REF, ref_name=ref_name)
            for ref_name in spec.ref_names
        )
        models.append(
            CompiledModel(
                key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=spec.name),
                deps=(),
                name=spec.name,
                relative_path=Path(f"models/{spec.name}.sql"),
                query_sql=f"SELECT * FROM {spec.name}",
                config=CompileModelConfig(
                    values={
                        "materialized": "incremental",
                        "cursor": spec.cursor,
                        "incremental_strategy": "delete_insert",
                    }
                    | spec.extra_config
                ),
                destination=CompiledRelationLocation(
                    database=None,
                    schema=spec.schema,
                    name=spec.name,
                    qualified_name=f"{spec.schema}.{spec.name}",
                ),
                references=references,
            )
        )

    seeds: list[CompiledSeedView] = []
    seed_name: str
    for seed_name, target_schema in (seed_locations or {}).items():
        seeds.append(
            CompiledSeed(
                key=CompiledObjectKey(resource_type=CompiledResourceType.SEED, name=seed_name),
                deps=(),
                name=seed_name,
                seed_file=DiscoveredSeedFile(
                    file_path=Path(f"seeds/{seed_name}.csv"),
                    relative_path=Path(f"seeds/{seed_name}.csv"),
                ),
                schema_entry=SchemaSeedEntry(name=seed_name, columns=()),
                schema_file=DiscoveredSchemaFile(
                    file_path=Path("seeds/schema.yml"),
                    relative_path=Path("seeds/schema.yml"),
                    contents="",
                    model_entries=(),
                    seed_entries=(),
                ),
                destination=CompiledRelationLocation(
                    database=None,
                    schema=target_schema,
                    name=seed_name,
                    qualified_name=(None, f"{target_schema}.{seed_name}")[bool(target_schema)],
                ),
            )
        )

    sources: list[CompiledSourceView] = []
    source_spec: tuple[str, str, str]
    for source_spec in source_names:
        sources.append(
            CompiledSource(
                key=CompiledObjectKey(
                    resource_type=CompiledResourceType.SOURCE, name=source_spec[0]
                ),
                deps=(),
                name=source_spec[0],
                source_entry=SourceEntry(
                    name=source_spec[0], schema=source_spec[1], table=source_spec[2]
                ),
                source_file=DiscoveredSourceFile(
                    file_path=Path("sources/sources.yml"),
                    relative_path=Path("sources/sources.yml"),
                    contents="",
                    source_entries=(),
                ),
            )
        )

    return CompiledProject(
        run_id="test_run",
        effective_target_name=None,
        effective_connection={},
        effective_vars={},
        models=tuple(models),
        sources=tuple(sources),
        seeds=tuple(seeds),
    )


def with_leading_enforced_model_contracts(
    project: CompiledProjectView, *, columns_by_model: dict[str, tuple[str, ...]]
) -> CompiledProjectView:
    """Attach enforced contracts to leading models in a compiled test project."""

    models: list[CompiledModelView] = list(project.models)
    for index, (model_name, column_names) in enumerate(columns_by_model.items()):
        model: CompiledModelView = models[index]
        assert model.name == model_name
        models[index] = replace(
            compiled_model_record(model),
            config=replace(model.config, values=model.config.values | {"contract": "enforced"}),
            schema_entry=SchemaModelEntry(
                name=model.name,
                columns=tuple(SchemaColumn(name=name) for name in column_names),
            ),
        )
    return project_with(project=project, models=tuple(models))


def build_deferred_locations_from_map(
    targets: dict[str, str],
) -> dict[str, CompiledRelationLocation]:
    """Build deferred locations from a name -> qualified_name mapping."""

    return {
        name: CompiledRelationLocation(
            database=None,
            schema=(None, qualified.rsplit(".", 1)[0])["." in qualified],
            name=name,
            qualified_name=qualified,
        )
        for name, qualified in targets.items()
    }


class ConcurrentDuckDbAdapter(DuckDbAdapter):
    """DuckDB adapter allowing four concurrent inspection reads."""

    metadata_inspection_concurrency: ClassVar[int] = 4


class _FailingFetch:
    def fetchall(self) -> list[Any]:
        raise RuntimeError("fetch failed")


class _Rows:
    def __init__(self, rows: list[Any]) -> None:
        self._rows: list[Any] = rows

    def fetchall(self) -> list[Any]:
        return self._rows


def _read_rows(connection: Any, sql: str, lock: threading.Lock) -> Any:
    with lock:
        return _Rows(connection.execute(sql).fetchall())


def _raise_unreadable(connection: Any, sql: str, lock: threading.Lock) -> Any:
    del connection, lock
    raise RuntimeError(f"cannot read {sql.split(' FROM ', 1)[1]}")


def _failing_fetch(connection: Any, sql: str, lock: threading.Lock) -> Any:
    del connection, sql, lock
    return _FailingFetch()


_CURSOR_ANSWERS: dict[tuple[bool, bool], Callable[[Any, str, threading.Lock], Any]] = {
    (False, False): _read_rows,
    (False, True): _read_rows,
    (True, False): _raise_unreadable,
    (True, True): _failing_fetch,
}


class TrackingCursorExecute:
    """Execute double that tracks overlapping cursor reads and fails chosen relations."""

    def __init__(
        self,
        *,
        failing_relations: frozenset[str] = frozenset(),
        failing_eligible_relations: frozenset[str] = frozenset(),
        fail_on_fetch: bool = False,
    ) -> None:
        self._failing: tuple[frozenset[str], frozenset[str]] = (
            failing_relations,
            failing_eligible_relations,
        )
        self._fail_on_fetch: bool = fail_on_fetch
        self._lock: threading.Lock = threading.Lock()
        self._active: int = 0
        self.max_active: int = 0
        self.cursor_sql: list[str] = []

    def __call__(self, *, connection: Any, sql: str) -> Any:
        eligible: bool = "<=" in sql
        tracked: bool = eligible or "AS _max" in sql
        return (self._untracked, self._tracked)[tracked](connection, sql, eligible)

    def _untracked(self, connection: Any, sql: str, eligible: bool) -> Any:
        del eligible
        return _read_rows(connection, sql, self._lock)

    def _tracked(self, connection: Any, sql: str, eligible: bool) -> Any:
        with self._lock:
            self._active += 1
            self.max_active = max(self.max_active, self._active)
            self.cursor_sql.append(sql)
        try:
            time.sleep(0.01)
            relations: set[str] = {
                part.split(None, 1)[0].rstrip(")") for part in sql.split(" FROM ")[1:]
            }
            failing: bool = not relations.isdisjoint(self._failing[eligible])
            return _CURSOR_ANSWERS[(failing, self._fail_on_fetch)](connection, sql, self._lock)
        finally:
            with self._lock:
                self._active -= 1


def create_events(connection: Any, count: int) -> None:
    """Create one small events table per fanned model."""

    index: int
    for index in range(count):
        connection.execute(f"CREATE TABLE staging.events_{index} (event_time TIMESTAMP)")
        connection.execute(
            f"INSERT INTO staging.events_{index} VALUES ('2024-01-01'), "
            f"('2024-03-0{index % 9 + 1}')"
        )


def gather_with_catalog(
    *,
    project: CompiledProjectView,
    adapter: DuckDbAdapter,
    connection: Any,
    execute: TrackingCursorExecute,
    progress: list[str],
) -> WarehouseSnapshot:
    """Gather a snapshot inside an inspection catalog of the adapter."""

    with open_inspection_catalog(adapter=adapter, connection=connection):
        return gather_warehouse_snapshot(
            project=project,
            dialect=Dialect(adapter),
            reads=live_reads(adapter=adapter, connection=connection, execute=execute),
            on_progress=progress.append,
        )


def unreadable_inputs(snapshot: WarehouseSnapshot) -> dict[str, tuple[str, ...]]:
    """Return the unreadable watermark inputs of every model that has some."""

    return dict(
        filter(
            lambda item: bool(item[1]),
            (
                (name, cursor.unreadable_input_names)
                for name, cursor in snapshot.cursor_snapshots.items()
            ),
        )
    )


def cursor_progress(progress: list[str]) -> list[str]:
    """Return the cursor-bound progress messages."""

    return list(filter(lambda message: "cursor bounds" in message, progress))


def build_fanned_cursor_project(
    count: int, *, extra_config: dict[str, object] | None = None
) -> CompiledProjectView:
    """Build one incremental model per upstream events table."""

    return build_project_with_targets(
        model_locations={f"events_{index}": "staging" for index in range(count)},
        incremental_models=tuple(
            _IncrementalModelSpec(
                name=f"daily_{index}",
                schema="marts",
                cursor="event_time",
                ref_names=(f"events_{index}",),
                extra_config=extra_config or {},
            )
            for index in range(count)
        ),
    )
