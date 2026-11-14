"""Source loader execution models."""

from __future__ import annotations

import logging
from collections.abc import Callable, Mapping
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Any

from sqlbuild._native import DiscoveredLoaderFunction, SourceEntry
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.models import ColumnInfo, LifeCycleEvent
from sqlbuild.adapter.contract.types import LoaderLogicalType
from sqlbuild.adapter.relations.main.resolve_qualified_name_parts import (
    resolve_qualified_name_parts,
)
from sqlbuild.compiler.python_nodes.types import SkipMode
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.load.constants import LOADER_RELATION_QUALIFIER_SEPARATOR
from sqlbuild.executor.load.types import LoadProgressCallback
from sqlbuild.executor.node_results.classes.dependency_result_reader import DependencyResultReader
from sqlbuild.executor.python_nodes.classes.runtime_relation_guard import RuntimeRelationGuard
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.provider.main.runtime import ProviderContainer, _empty_provider_container
from sqlbuild.python_nodes.models import SqlResourceRef
from sqlbuild.python_nodes.types import SqlResourceRefKind
from sqlbuild.runtime.contracts.types import ConnectionElapsedCallback, ExecutionResourceKind


@dataclass(frozen=True)
class DecimalShape:
    """The digits a decimal column keeps before and after its point."""

    integer_digits: int
    scale: int

    @property
    def precision(self) -> int:
        """The total digits, at least one."""

        return max(self.integer_digits + self.scale, 1)

    def widened(self, other: DecimalShape) -> DecimalShape:
        """The narrowest shape holding the values of both."""

        return DecimalShape(
            integer_digits=max(self.integer_digits, other.integer_digits),
            scale=max(self.scale, other.scale),
        )

    def __str__(self) -> str:
        return f"DECIMAL({self.precision}, {self.scale})"


@dataclass(frozen=True)
class LoaderRowsSchema:
    """Tracked schema state for one or more loader row batches."""

    column_names: tuple[str, ...]
    inferred_types: dict[str, LoaderLogicalType]
    added_columns: tuple[ColumnInfo, ...]
    decimal_shapes: dict[str, DecimalShape]


@dataclass(frozen=True)
class LoaderRelationRef:
    """A loader-visible relation reference with cursor helpers."""

    name: str
    destination: str
    database: str | None
    schema: str | None
    table_name: str
    cursor_column: str | None
    adapter: BaseAdapter
    connection: Any
    statement_recorder: StatementRecorder

    @property
    def current_cursor_value(self) -> object | None:
        if self.cursor_column is None:
            return None
        return self.max(self.cursor_column)

    def max(self, column: str) -> object | None:
        if not self.adapter.relation_exists(
            connection=self.connection,
            database=self.database,
            schema=self.schema,
            name=self.table_name,
        ):
            return None
        sql: str = f"SELECT MAX({column}) FROM {self.destination}"
        self.statement_recorder.record(sql)
        cursor: Any = self.adapter.execute(connection=self.connection, sql=sql)
        row: object | None = cursor.fetchone()
        if row is None:
            return None
        return row[0]


@dataclass(frozen=True)
class LoaderSkipResult:
    """User-facing skip signal returned by a source loader."""

    reason: str
    mode: SkipMode = SkipMode.SOFT


@dataclass(frozen=True)
class LoaderResult:
    """User-facing successful result returned by a self-managed source loader."""

    payload: object | None = None
    metadata: dict[str, object] = field(default_factory=dict)
    materialized: bool | None = None


@dataclass(frozen=True)
class LoaderContext(DependencyResultReader):
    """Runtime context passed to a source loader function."""

    adapter: BaseAdapter
    connection_config: dict[str, object]
    connection: Any
    destination: str
    destination_database: str | None
    destination_schema: str | None
    destination_name: str
    run_id: str
    target: str | None
    vars: dict[str, object]
    is_reload: bool
    use_color: bool
    current_cursor_value: object | None
    logger: logging.Logger
    statement_recorder: StatementRecorder
    start_cursor_ts: datetime | None = None
    end_cursor_ts: datetime | None = None
    start_cursor_int: int | None = None
    end_cursor_int: int | None = None
    loader_refs: Mapping[Callable[..., object], LoaderRelationRef] = field(default_factory=dict)
    source_refs: Mapping[str, LoaderRelationRef] = field(default_factory=dict)
    providers: ProviderContainer = field(default_factory=_empty_provider_container)
    result_store: Any | None = None
    runtime_dir: Path = Path("target")
    on_progress: Callable[[str], None] | None = None
    relation_guard: RuntimeRelationGuard | None = field(default=None, repr=False)

    def execute_sql(self, sql: str) -> Any:
        if self.relation_guard is not None:
            self.relation_guard.check(sql)
        self.statement_recorder.record(sql)
        return self.adapter.execute(connection=self.connection, sql=sql)

    def query(self, sql: str) -> Any:
        if self.relation_guard is not None:
            self.relation_guard.check(sql)
        self.statement_recorder.record(sql)
        return self.adapter.execute(connection=self.connection, sql=sql)

    def log(self, message: str) -> None:
        self.statement_recorder.log(message)

    def progress(self, message: str) -> None:
        if self.on_progress is not None:
            self.on_progress(message)

    def skip(self, *, reason: str, mode: SkipMode = SkipMode.SOFT) -> LoaderSkipResult:
        """Return a skip signal for the current source loader."""

        return LoaderSkipResult(reason=reason, mode=mode)

    def result(
        self,
        *,
        payload: object | None = None,
        metadata: dict[str, object] | None = None,
        materialized: bool | None = None,
    ) -> LoaderResult:
        """Return a successful result for a self-managed source loader."""

        return LoaderResult(
            payload=payload,
            metadata={} if metadata is None else metadata,
            materialized=materialized,
        )

    def qualify_name(
        self,
        *,
        name: str,
        database: str | None = None,
        schema: str | None = None,
    ) -> str:
        """Return a fully-qualified relation name, preserving already-qualified input."""

        if LOADER_RELATION_QUALIFIER_SEPARATOR in name:
            return name
        return resolve_qualified_name_parts(
            adapter=self.adapter,
            database=self.destination_database if database is None else database,
            schema=self.destination_schema if schema is None else schema,
            name=name,
        )

    def qualify_in_destination_schema(self, name: str) -> str:
        """Return a relation name qualified into the destination database/schema."""

        return self.qualify_name(name=name)

    def loader(self, loader_fn: Callable[..., object]) -> LoaderRelationRef:
        """Return a relation reference for an upstream loader function."""

        relation_ref: LoaderRelationRef | None = self.loader_refs.get(loader_fn)
        if relation_ref is None:
            raise ExecutorInputError("Unknown loader reference passed to ctx.loader(...)")
        self._record_resolved(relation_ref)
        return relation_ref

    def source(self, source_name: str) -> LoaderRelationRef:
        """Return a relation reference for a project source by YAML source name."""

        relation_ref: LoaderRelationRef | None = self.source_refs.get(source_name)
        if relation_ref is None:
            raise ExecutorInputError(
                f"Unknown source reference passed to ctx.source(...): {source_name}"
            )
        self._record_resolved(relation_ref)
        return relation_ref

    def _record_resolved(self, relation_ref: LoaderRelationRef) -> None:
        if self.relation_guard is not None:
            self.relation_guard.record_resolved(
                SqlResourceRef(kind=SqlResourceRefKind.SOURCE, name=relation_ref.name)
            )


@dataclass(frozen=True)
class LoadExecutionResult:
    """Execution result for one source loader."""

    source_name: str
    loader_name: str
    status: ExecutionStatus
    target: str
    resource_kind: ExecutionResourceKind = ExecutionResourceKind.SOURCE
    staging_relation: str | None = None
    rows_loaded: int = 0
    duration_ms: int | None = None
    lifecycle_events: tuple[LifeCycleEvent, ...] = field(default_factory=tuple)
    skip_mode: SkipMode | None = None
    skip_reason: str | None = None
    error_message: str | None = None
    result_payload: object | None = None
    result_metadata: dict[str, object] = field(default_factory=dict)
    result_materialized: bool | None = None
    warning_messages: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class LoadExecutionIndexes:
    """Resolved loader/source indexes used during load execution."""

    loader_by_name: dict[str, DiscoveredLoaderFunction]
    source_by_name: dict[str, SourceEntry]
    source_by_loader_name: dict[str, SourceEntry]
    loader_ref_entries: dict[Callable[..., object], SourceEntry]
    loader_name_by_function: dict[Callable[..., object], str]
    has_loader_dependencies: bool


@dataclass(frozen=True)
class LoadRuntimeParams:
    """Execution-invariant parameters shared by all loader runs in one pipeline."""

    run_id: str
    target: str | None
    vars: dict[str, object]
    is_reload: bool
    runtime_dir: Path = Path("target")
    start_cursor_ts: datetime | None = None
    end_cursor_ts: datetime | None = None
    start_cursor_int: int | None = None
    end_cursor_int: int | None = None
    use_color: bool = False
    providers: ProviderContainer | None = None
    result_store: Any | None = None
    schema_prepared: bool = False
    project_relations: Mapping[SqlResourceRef, str] | None = None


@dataclass(frozen=True)
class LoadDispatchInputs:
    """Per-node dispatch inputs shared by DAG source execution helpers."""

    source_by_name: dict[str, SourceEntry]
    indexes: LoadExecutionIndexes
    failed_or_hard_skipped: set[str]
    results_by_name: dict[str, LoadExecutionResult]


@dataclass(frozen=True)
class LoaderDestination:
    """Resolved destination relation and bare name for one loader run."""

    relation: str
    name: str


@dataclass(frozen=True)
class LoaderRefBindings:
    """Loader and source relation bindings, and the hard-coded name guard, for one loader run."""

    loader_ref_entries: Mapping[Callable[..., object], SourceEntry] | None = None
    source_ref_entries: Mapping[str, SourceEntry] | None = None
    relation_guard: RuntimeRelationGuard | None = None


@dataclass(frozen=True)
class LoadCallbacks:
    """Progress callbacks for one load pipeline run."""

    on_load_start: Callable[[SourceEntry], None] | None = None
    on_load_progress: LoadProgressCallback | None = None
    on_load_complete: Callable[[LoadExecutionResult], None] | None = None
    on_connection_start: Callable[[int], None] | None = None
    on_connection_complete: ConnectionElapsedCallback | None = None
    on_connection_error: ConnectionElapsedCallback | None = None


@dataclass(frozen=True)
class ExternalLoadPhaseResult:
    """Outcome of running external-connection loaders before warehouse connect."""

    preloaded_results: tuple[LoadExecutionResult, ...]
    failed_or_hard_skipped: frozenset[str]
    sqlbuild_sources: tuple[SourceEntry, ...]


from sqlbuild.executor.load.classes.dag_state import LoadDagState  # noqa: E402,F401
