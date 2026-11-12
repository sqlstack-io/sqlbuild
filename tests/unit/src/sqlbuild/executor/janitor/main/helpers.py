"""Test helpers for janitor executor tests."""

from __future__ import annotations

import base64
from collections.abc import Callable
from dataclasses import replace
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, ClassVar, cast

from sqlbuild._native import (
    FINGERPRINT_TABLE_NAME,
    JANITOR_EVENTS_TABLE_NAME,
    SOURCE_FRESHNESS_TABLE_NAME,
    CompiledModel,
    CompiledObjectKey,
    CompiledProject,
    CompiledRelationLocation,
    CompiledResourceType,
    CompiledSeed,
    CompiledSource,
    CompileModelConfig,
    SchemaSeedEntry,
    SeedCsvSettings,
    SourceEntry,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.models import (
    ColumnInfo,
    CursorValue,
    RelationInfo,
    RowDiffResult,
    RowDiffTolerances,
    SchemaDiffResult,
)
from sqlbuild.adapter.contract.types import TablePromotionMode
from sqlbuild.compiler.compile.types import (
    CompiledProjectView,
    CompiledSourceView,
)
from sqlbuild.executor.janitor.models import JanitorActionDone, JanitorProgressItem
from sqlbuild.executor.janitor.types import JanitorAction
from sqlbuild.executor.janitor_events.constants import (
    JANITOR_EVENT_SCHEMA_VERSION,
)
from sqlbuild.executor.janitor_events.models import JanitorEventRecord
from sqlbuild.executor.janitor_events.types import JanitorEventType
from sqlbuild.spec.contracts.constants import DEFAULT_SEED_CSV_SETTINGS

UNQUOTED_ADDRESSING_REASON: str = (
    "relation name is not a plain lowercase identifier and may require quoting; "
    "janitor does not act on it"
)
CASE_COLLISION_REASON: str = (
    "relation name collides case-insensitively with another relation; janitor does not act on it"
)
MALFORMED_ARCHIVE_REASON: str = (
    "name resembles a janitor archive but does not match the strict archive grammar"
)


class FakeJanitorAdapter(BaseAdapter):
    adapter_name: ClassVar[str] = "janitor-test"

    def __init__(
        self,
        *,
        relation_infos: tuple[RelationInfo, ...],
        supports_age_metadata: bool = True,
        tracked_relations: tuple[tuple[str | None, str | None, str], ...] = (),
        identifier_limit: int = 255,
    ) -> None:
        self.relation_infos: tuple[RelationInfo, ...] = relation_infos
        self.age_metadata_supported: bool = supports_age_metadata
        self.identifier_limit: int = identifier_limit
        self.dropped_targets: list[str] = []
        self.dropped_view_targets: list[str] = []
        self.renamed_targets: list[tuple[str, str]] = []
        self.executed_sql: list[str] = []
        self.tracked_relations: tuple[tuple[str | None, str | None, str], ...] = tracked_relations
        self._tracked_rows: tuple[tuple[Any, ...], ...] = tuple(
            (
                "model",
                tracked_relation[2],
                tracked_relation[0],
                tracked_relation[1],
                tracked_relation[2],
                "run_001",
                "definition_hash",
                "version_hash",
                "schema_hash",
                base64.b64encode(b"SELECT 1").decode("ascii"),
                base64.b64encode(b"{}").decode("ascii"),
                "2026-01-15T12:00:00",
            )
            for tracked_relation in tracked_relations
        )
        tracked_locations: set[tuple[str | None, str | None]] = {
            (tracked[0], tracked[1]) for tracked in tracked_relations
        }
        self._available_relations: tuple[RelationInfo, ...] = (
            *relation_infos,
            *tuple(
                RelationInfo(
                    database=database,
                    schema=schema,
                    name=FINGERPRINT_TABLE_NAME,
                    relation_type="base table",
                )
                for database, schema in tracked_locations
            ),
        )
        relation_keys: set[tuple[str | None, str | None, str]] = {
            (relation.database, relation.schema, relation.name)
            for relation in self._available_relations
        }
        fingerprint_relation_keys: set[tuple[str | None, str | None, str]] = {
            (relation.database, relation.schema, FINGERPRINT_TABLE_NAME)
            for relation in self._available_relations
        }
        freshness_relation_keys: set[tuple[str | None, str | None, str]] = {
            (relation.database, relation.schema, SOURCE_FRESHNESS_TABLE_NAME)
            for relation in self._available_relations
        }
        state_relation_keys: set[tuple[str | None, str | None, str]] = (
            fingerprint_relation_keys | freshness_relation_keys
        )
        self._existing_relation_keys = relation_keys & state_relation_keys

    def supports_relation_age_metadata(self) -> bool:
        return self.age_metadata_supported

    def maximum_identifier_length(self) -> int:
        return self.identifier_limit

    def default_schema(self) -> str | None:
        return "analytics"

    def default_database(self) -> str | None:
        return None

    def star_exclude_keyword(self) -> str:
        return "EXCLUDE"

    def default_table_promotion_mode(self) -> TablePromotionMode:
        return TablePromotionMode.STAGED

    def connect(self, config: dict[str, Any]) -> object:
        return object()

    def close(self, connection: Any) -> None:
        return None

    def _execute(self, connection: Any, sql: str) -> Any:
        del connection
        self.executed_sql.append(sql)
        rows_by_query_kind: dict[bool, tuple[tuple[Any, ...], ...]] = {
            True: (),
            False: self._tracked_rows,
        }
        return _FakeResult(
            rows=rows_by_query_kind["LIMIT 0" in sql or JANITOR_EVENTS_TABLE_NAME in sql]
        )

    def list_relations(
        self,
        connection: Any,
        *,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[RelationInfo, ...]:
        selected: list[RelationInfo] = []
        available_schemas: tuple[str, ...] = schemas or tuple(
            relation.schema or "" for relation in self._available_relations
        )
        available_names: tuple[str, ...] = names or tuple(
            relation.name for relation in self._available_relations
        )
        for relation in self._available_relations:
            _APPEND_RELATION_BY_SELECTED[
                relation.database == database
                and (relation.schema or "") in available_schemas
                and relation.name in available_names
            ](selected, relation)
        return tuple(selected)

    def get_columns(
        self,
        connection: Any,
        *,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> tuple[ColumnInfo, ...]:
        return ()

    def get_all_columns(
        self,
        connection: Any,
        *,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> dict[str, tuple[ColumnInfo, ...]]:
        return {}

    def relation_exists(
        self,
        connection: Any,
        *,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> bool:
        del connection
        return (database, schema, name) in self._existing_relation_keys

    def render_prune_fingerprint_history_sql(
        self,
        *,
        database: str | None,
        schema: str,
        retain_versions: int,
    ) -> str:
        del database, schema
        return f"PRUNE {FINGERPRINT_TABLE_NAME} KEEP {retain_versions}"

    def render_prune_source_freshness_history_sql(
        self,
        *,
        database: str | None,
        schema: str,
        retain_versions: int,
    ) -> str:
        del database, schema
        return f"PRUNE {SOURCE_FRESHNESS_TABLE_NAME} KEEP {retain_versions}"

    def drop(
        self,
        connection: Any,
        *,
        destination: str,
        if_exists: bool = True,
        statement_recorder: StatementRecorder,
    ) -> None:
        del connection, if_exists, statement_recorder
        self.dropped_targets.append(destination)

    def drop_view(
        self,
        connection: Any,
        *,
        destination: str,
        if_exists: bool = True,
        statement_recorder: StatementRecorder,
    ) -> None:
        del connection, if_exists, statement_recorder
        self.dropped_view_targets.append(destination)

    def create_table_as(
        self,
        connection: Any,
        *,
        destination: str,
        sql: str,
        config: dict[str, Any] | None = None,
        statement_recorder: StatementRecorder,
    ) -> None:
        raise NotImplementedError

    def create_view_as(
        self,
        connection: Any,
        *,
        destination: str,
        sql: str,
        statement_recorder: StatementRecorder,
    ) -> None:
        raise NotImplementedError

    def append(
        self,
        connection: Any,
        *,
        destination: str,
        sql: str,
        columns: tuple[str, ...] | None = None,
        statement_recorder: StatementRecorder,
    ) -> None:
        raise NotImplementedError

    def delete_insert(
        self,
        connection: Any,
        *,
        destination: str,
        sql: str,
        unique_key: str | tuple[str, ...],
        columns: tuple[str, ...] | None = None,
        statement_recorder: StatementRecorder,
    ) -> None:
        raise NotImplementedError

    def delete_insert_cursor(
        self,
        connection: Any,
        *,
        destination: str,
        sql: str,
        cursor_column: str,
        cursor_start: str,
        cursor_end: str,
        columns: tuple[str, ...] | None = None,
        statement_recorder: StatementRecorder,
        cursor_type: str | None = None,
    ) -> None:
        raise NotImplementedError

    def merge(
        self,
        connection: Any,
        *,
        destination: str,
        sql: str,
        unique_key: str | tuple[str, ...],
        statement_recorder: StatementRecorder,
        exclude_columns: tuple[str, ...] = (),
    ) -> None:
        raise NotImplementedError

    def load_seed(
        self,
        connection: Any,
        *,
        destination: str,
        file_path: Path,
        columns: tuple[ColumnInfo, ...],
        csv_settings: SeedCsvSettings = DEFAULT_SEED_CSV_SETTINGS,
        replace: bool = True,
        infer_types: bool = False,
        statement_recorder: StatementRecorder,
    ) -> None:
        raise NotImplementedError

    def add_columns(
        self,
        connection: Any,
        *,
        destination: str,
        columns: tuple[ColumnInfo, ...],
        statement_recorder: StatementRecorder,
    ) -> None:
        raise NotImplementedError

    def drop_columns(
        self,
        connection: Any,
        *,
        destination: str,
        column_names: tuple[str, ...],
        statement_recorder: StatementRecorder,
    ) -> None:
        raise NotImplementedError

    def alter_column_types(
        self,
        connection: Any,
        *,
        destination: str,
        columns: tuple[ColumnInfo, ...],
        statement_recorder: StatementRecorder,
    ) -> None:
        raise NotImplementedError

    def rename(
        self,
        connection: Any,
        *,
        origin: str,
        destination: str,
        statement_recorder: StatementRecorder,
    ) -> None:
        del connection, statement_recorder
        self.renamed_targets.append((origin, destination))

    def swap(
        self,
        connection: Any,
        *,
        left: str,
        right: str,
        statement_recorder: StatementRecorder,
    ) -> None:
        raise NotImplementedError

    def clone(
        self,
        connection: Any,
        *,
        origin: str,
        destination: str,
        hard_copy: bool = False,
        origin_is_transient: bool = False,
        statement_recorder: StatementRecorder,
    ) -> None:
        raise NotImplementedError

    def diff_schema(
        self,
        connection: Any,
        *,
        left: str,
        right: str,
    ) -> SchemaDiffResult:
        raise NotImplementedError

    def diff_rows(
        self,
        connection: Any,
        *,
        left: str,
        right: str,
        unique_key: str | tuple[str, ...],
        excluded_columns: tuple[str, ...] = (),
        tolerances: RowDiffTolerances | None = None,
        cursor_column: str | None = None,
        start_cursor: CursorValue | None = None,
        end_cursor: CursorValue | None = None,
    ) -> RowDiffResult:
        raise NotImplementedError

    def relation_names_match(self, left: str, right: str) -> bool:
        return left == right

    def render_seed_select_before_cursor(
        self,
        *,
        origin: str,
        cursor_column: str,
        cursor_end_exclusive: str,
        cursor_type: object,
    ) -> str:
        raise NotImplementedError


class FoldingJanitorAdapter(FakeJanitorAdapter):
    """Warehouse double that filters case-insensitively and lists lowercase identifiers."""

    def list_relations(
        self,
        connection: Any,
        *,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[RelationInfo, ...]:
        requested_schemas: frozenset[str] = frozenset(schema.lower() for schema in schemas or ())
        requested_names: frozenset[str] = frozenset(name.lower() for name in names or ())
        matching: tuple[RelationInfo, ...] = tuple(
            filter(
                lambda relation: (
                    (relation.database or "").lower() == (database or "").lower()
                    and (
                        not requested_schemas
                        or (relation.schema or "").lower() in requested_schemas
                    )
                    and (not requested_names or relation.name.lower() in requested_names)
                ),
                self._available_relations,
            )
        )
        return tuple(
            replace(
                relation,
                database=database,
                schema=(relation.schema or "").lower(),
                name=relation.name.lower(),
            )
            for relation in matching
        )


class SeparateAgeMetadataJanitorAdapter(FakeJanitorAdapter):
    """Warehouse double that lists without timestamps and supplies ages separately."""

    def __init__(
        self,
        *,
        relation_infos: tuple[RelationInfo, ...],
        relation_ages: dict[str, datetime],
        tracked_relations: tuple[tuple[str | None, str | None, str], ...] = (),
    ) -> None:
        super().__init__(relation_infos=relation_infos, tracked_relations=tracked_relations)
        self.relation_ages: dict[str, datetime] = relation_ages
        self.age_metadata_requests: list[tuple[str, ...]] = []

    def with_relation_age_metadata(
        self,
        *,
        connection: Any,
        relations: tuple[RelationInfo, ...],
    ) -> tuple[RelationInfo, ...]:
        self.age_metadata_requests.append(tuple(relation.name for relation in relations))
        return tuple(
            replace(relation, last_altered_at=self.relation_ages.get(relation.name))
            for relation in relations
        )


class FailingJanitorEventAdapter(FakeJanitorAdapter):
    def _execute(self, connection: Any, sql: str) -> Any:
        is_event_insert: bool = sql.startswith("INSERT INTO") and JANITOR_EVENTS_TABLE_NAME in sql
        _EVENT_WRITE_GUARDS[is_event_insert](sql)
        return super()._execute(connection=connection, sql=sql)


def _allow_statement(sql: str) -> None:
    del sql


def _fail_event_write(sql: str) -> None:
    del sql
    raise RuntimeError("simulated janitor event write failure")


_EVENT_WRITE_GUARDS: dict[bool, Callable[[str], None]] = {
    False: _allow_statement,
    True: _fail_event_write,
}


class _FakeResult:
    def __init__(self, *, rows: tuple[tuple[Any, ...], ...]) -> None:
        self.rows: tuple[tuple[Any, ...], ...] = rows

    def fetchall(self) -> list[tuple[Any, ...]]:
        return list(self.rows)


def build_project(
    *,
    source_schema: str | None = None,
    source_database: str | None = None,
    destination_database: str | None = None,
    destination_schema: str = "analytics",
    migrate_from: str | None = None,
) -> CompiledProjectView:
    source: CompiledSourceView = CompiledSource(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.SOURCE,
            name="raw_orders",
        ),
        deps=(),
        name="raw_orders",
        source_entry=SourceEntry(
            name="raw_orders",
            database=source_database,
            schema=source_schema or "",
            table="orders",
        ),
        source_file=cast(Any, object()),
    )
    sources: tuple[CompiledSourceView, ...] = {True: (), False: (source,)}[source_schema is None]
    return CompiledProject(
        run_id="run-1",
        effective_target_name="dev",
        effective_connection={},
        effective_vars={},
        models=(
            CompiledModel(
                key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders"),
                deps=(),
                name="orders",
                relative_path=Path("models/orders.sql"),
                query_sql="select 1",
                config=CompileModelConfig(values={"migrate_from": migrate_from}),
                destination=CompiledRelationLocation(
                    database=destination_database,
                    schema=destination_schema,
                    name="orders",
                    qualified_name=f"{destination_schema}.orders",
                ),
            ),
        ),
        seeds=(
            CompiledSeed(
                key=CompiledObjectKey(resource_type=CompiledResourceType.SEED, name="countries"),
                deps=(),
                name="countries",
                seed_file=cast(Any, object()),
                schema_entry=SchemaSeedEntry(name="countries"),
                schema_file=cast(Any, object()),
                destination=CompiledRelationLocation(
                    database=destination_database,
                    schema=destination_schema,
                    name="countries",
                    qualified_name=f"{destination_schema}.countries",
                ),
            ),
        ),
        sources=sources,
    )


def _append_relation(selected: list[RelationInfo], relation: RelationInfo) -> None:
    selected.append(relation)


def _ignore_relation(selected: list[RelationInfo], relation: RelationInfo) -> None:
    del selected, relation


_APPEND_RELATION_BY_SELECTED: dict[bool, Callable[[list[RelationInfo], RelationInfo], None]] = {
    True: _append_relation,
    False: _ignore_relation,
}


PHASE_EVENT_TIME: datetime = datetime(2026, 9, 24, 10, 15, tzinfo=UTC)


def phase_event(index: int) -> JanitorEventRecord:
    """The synthetic audit event of the archive action at `index`."""

    archive_name: str = f"_sqb_archive__20260924t101500z__orders_{index:04d}"
    return JanitorEventRecord(
        event_id=f"event_{index:04d}",
        schema_version=JANITOR_EVENT_SCHEMA_VERSION,
        event_type=JanitorEventType.ARCHIVE,
        occurred_at=PHASE_EVENT_TIME,
        run_id="run_orders_cleanup",
        relation_database=None,
        relation_schema="analytics",
        relation_type="BASE TABLE",
        original_name=f"orders_{index:04d}",
        original_qualified_name=f"analytics.orders_{index:04d}",
        archive_name=archive_name,
        archive_qualified_name=f"analytics.{archive_name}",
        archived_at=PHASE_EVENT_TIME,
    )


class PhaseActions:
    """Archive actions that each return their event and remember that they completed."""

    def __init__(self) -> None:
        self.completed: list[int] = []
        self.flushed: list[str] = []

    def apply(self, *, item: int, connection: Any) -> JanitorActionDone:
        del connection
        self.completed.append(item)
        return JanitorActionDone(event=phase_event(item))

    def flush(self, finished: tuple[JanitorActionDone, ...]) -> None:
        self.flushed.extend(cast(JanitorEventRecord, done.event).event_id for done in finished)

    def completed_event_ids(self) -> list[str]:
        return sorted(phase_event(item).event_id for item in self.completed)


class FailingReportProgress:
    """Progress whose `failing_report`th finished-action report raises `error`."""

    def __init__(self, *, failing_report: int, error: type[BaseException], size: int) -> None:
        def quiet() -> None:
            return None

        def fail() -> None:
            raise error("progress stream closed")

        self._reactions: list[Callable[[], None]] = [
            *([quiet] * (failing_report - 1)),
            fail,
            *([quiet] * size),
        ]
        self._calls: int = 0
        self.closed_failed: list[bool] = []
        self.unwritten: list[tuple[int, tuple[str, ...], str]] = []

    def phase_started(self, *, phase: str, total: int) -> None:
        del phase, total

    def item_finished(self, *, phase: str, item: object) -> None:
        del phase, item
        self._calls += 1
        self._reactions[self._calls - 1]()

    def phase_finished(self, *, phase: str, failed: bool) -> None:
        del phase
        self.closed_failed.append(failed)

    def events_unwritten(self, *, count: int, schemas: tuple[str, ...], error: str) -> None:
        self.unwritten.append((count, schemas, error))


def interrupting_wait(real_wait: Callable[..., Any]) -> Callable[..., Any]:
    """`concurrent.futures.wait` that raises KeyboardInterrupt on its first call only."""

    def interrupt(*args: object, **kwargs: object) -> Any:
        del args, kwargs
        raise KeyboardInterrupt

    calls: list[Callable[..., Any]] = [interrupt]

    def wait(*args: Any, **kwargs: Any) -> Any:
        reaction: Callable[..., Any] = [*calls, real_wait][0]
        calls.clear()
        return reaction(*args, **kwargs)

    return wait


def phase_label(item: int) -> JanitorProgressItem:
    """The progress label of the archive action at `item`."""

    return JanitorProgressItem(action=JanitorAction.ARCHIVED, name=f"analytics.orders_{item:04d}")


def _raiser(error: type[BaseException], message: str) -> Callable[[], None]:
    def fail() -> None:
        raise error(message)

    return fail


def _quiet() -> None:
    return None


class FailingPhaseActions(PhaseActions):
    """Archive actions where the `failing_item`th raises `error`, and whose event flush raises."""

    def __init__(
        self,
        *,
        failing_item: int,
        error: type[BaseException],
        flush_error: type[BaseException],
        size: int,
    ) -> None:
        super().__init__()
        self._reactions: list[Callable[[], None]] = [
            *([_quiet] * failing_item),
            _raiser(error, f"rename failed for analytics.orders_{failing_item:04d}"),
            *([_quiet] * size),
        ]
        self._flush_error: type[BaseException] = flush_error

    def apply(self, *, item: int, connection: Any) -> JanitorActionDone:
        self._reactions[item]()
        return super().apply(item=item, connection=connection)

    def flush(self, finished: tuple[JanitorActionDone, ...]) -> None:
        del finished
        raise self._flush_error("events table is locked")


def interrupting_submit(
    real_submit: Callable[..., Any], *, failing_call: int
) -> Callable[..., Any]:
    """`submit_janitor_action` that raises KeyboardInterrupt on its `failing_call`th call."""

    reactions: list[Callable[[], None]] = [
        *([_quiet] * (failing_call - 1)),
        _raiser(KeyboardInterrupt, "interrupted while submitting"),
    ]
    calls: list[int] = []

    def submit(**kwargs: Any) -> Any:
        calls.append(len(calls))
        [*reactions, *([_quiet] * len(calls))][len(calls) - 1]()
        return real_submit(**kwargs)

    return submit
