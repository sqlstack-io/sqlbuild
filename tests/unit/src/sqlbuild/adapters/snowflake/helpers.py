"""Helpers for Snowflake adapter unit tests."""

from __future__ import annotations

import sys
from collections.abc import Callable
from contextlib import nullcontext
from dataclasses import dataclass
from io import StringIO
from itertools import cycle, islice
from types import ModuleType, SimpleNamespace
from typing import Any

from sqlbuild._native import NativeProgressProjector
from sqlbuild.adapter.contract.models import (
    ColumnInfo,
    RelationInfo,
    RelationUsageReport,
    RetentionRequest,
)
from sqlbuild.adapter.contract.types import RetentionScope
from sqlbuild.adapter.relations.main.relation_metadata_cache_scope import (
    relation_metadata_cache_scope,
)
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_connection import _SnowflakeConnection
from sqlbuild.observability import (
    EventDispatcher,
    LifecycleEvent,
    OperationAttributes,
    OperationLifecycle,
    ResourceAttemptLifecycle,
    dispatcher_scope,
    invocation_scope,
    run_scope,
)
from tests.unit.src.sqlbuild.adapters.snowflake.inspection.helpers import (
    FakeRelation,
    OfflineSnowflakeAdapter,
    RecordingSnowflakeWarehouse,
)


class FakeSnowflakeDescribeCursor:
    """Cursor double exposing Snowflake-style description metadata."""

    def __init__(self, description: tuple[tuple[object, ...], ...]) -> None:
        self.description: tuple[tuple[object, ...], ...] = description
        self.executed_sql: str | None = None
        self.executemany_sql: str | None = None
        self.executemany_rows: list[tuple[object, ...]] = []
        self.closed: bool = False

    def execute(self, sql: str) -> None:
        self.executed_sql = sql

    def executemany(self, sql: str, rows: list[tuple[object, ...]]) -> None:
        self.executemany_sql = sql
        self.executemany_rows = rows

    def close(self) -> None:
        self.closed = True


class FakeSnowflakeQueryCursor(FakeSnowflakeDescribeCursor):
    """Cursor double returning rows under Snowflake result metadata."""

    def __init__(
        self, description: tuple[tuple[object, ...], ...], rows: list[tuple[object, ...]]
    ) -> None:
        super().__init__(description=description)
        self._rows: list[tuple[object, ...]] = rows

    def fetchall(self) -> list[tuple[object, ...]]:
        return self._rows

    def fetchmany(self, size: int) -> list[tuple[object, ...]]:
        return self._rows[:size]


class FakeSnowflakeMissingObjectCursor(FakeSnowflakeQueryCursor):
    """Cursor double whose statement fails as Snowflake reports a missing object."""

    def execute(self, sql: str) -> None:
        super().execute(sql)
        raise RuntimeError("Database 'GONE' does not exist or not authorized.")


class FakeSnowflakeDescribeConnection:
    """Connection double returning a preconfigured describe cursor."""

    def __init__(self, cursor: FakeSnowflakeDescribeCursor) -> None:
        self._cursor: FakeSnowflakeDescribeCursor = cursor
        self.executed_sql: list[str] = []

    def execute(self, sql: str) -> Any:
        self.executed_sql.append(sql)
        return self._cursor

    def cursor(self) -> Any:
        return self._cursor


class FakeSnowflakeMetadataCursor:
    """Cursor double exposing Snowflake-style metadata rows."""

    def __init__(
        self,
        row: tuple[object, ...] | None = None,
        rows: list[tuple[object, ...]] | None = None,
    ) -> None:
        self.row: tuple[object, ...] | None = row
        self.rows: list[tuple[object, ...]] = rows or []
        self.executed_sql: str | None = None
        self.executed_params: tuple[object, ...] | None = None
        self.closed: bool = False

    def execute(self, sql: str, params: tuple[object, ...] | None = None) -> None:
        self.executed_sql = sql
        self.executed_params = params

    def fetchone(self) -> tuple[object, ...] | None:
        return self.row

    def fetchall(self) -> list[tuple[object, ...]]:
        return self.rows

    def close(self) -> None:
        self.closed = True


class FakeSnowflakeMetadataConnection:
    """Connection double returning a preconfigured metadata cursor."""

    def __init__(self, cursor: FakeSnowflakeMetadataCursor) -> None:
        self._cursor: FakeSnowflakeMetadataCursor = cursor

    def cursor(self) -> FakeSnowflakeMetadataCursor:
        return self._cursor


class FakeSnowflakeMetadataSequenceConnection:
    """Connection double returning one metadata cursor per exact relation query."""

    def __init__(self, cursors: tuple[FakeSnowflakeMetadataCursor, ...]) -> None:
        self._cursors: list[FakeSnowflakeMetadataCursor] = list(cursors)
        self.returned_cursors: list[FakeSnowflakeMetadataCursor] = []

    def cursor(self) -> FakeSnowflakeMetadataCursor:
        cursor: FakeSnowflakeMetadataCursor = self._cursors.pop(0)
        self.returned_cursors.append(cursor)
        return cursor


def build_qualified_column_relations(*, count: int) -> tuple[RelationInfo, ...]:
    """Build mixed table/view relations with a duplicate name across two schemas."""

    names: tuple[str, ...] = (
        "SHARED",
        "SHARED",
        *(f"RELATION_{index}" for index in range(2, count)),
    )
    schemas: tuple[str, ...] = tuple(islice(cycle(("SCHEMA_A", "SCHEMA_B")), count))
    relation_types: tuple[str, ...] = tuple(islice(cycle(("BASE TABLE", "VIEW")), count))
    return tuple(
        RelationInfo(
            database="ANALYTICS",
            schema=schema,
            name=name,
            relation_type=relation_type,
        )
        for name, schema, relation_type in zip(names, schemas, relation_types, strict=True)
    )


def build_show_columns_rows(*, relation: RelationInfo) -> list[tuple[object, ...]]:
    """Build documented SHOW COLUMNS rows for numeric and text columns."""

    return [
        (
            relation.name,
            relation.schema,
            "ID",
            '{"type":"FIXED","precision":20,"scale":4,"nullable":false}',
            False,
            None,
            "COLUMN",
            None,
            None,
            relation.database,
        ),
        (
            relation.name,
            relation.schema,
            "LABEL",
            '{"type":"TEXT","length":128,"byteLength":512,"nullable":true}',
            True,
            None,
            "COLUMN",
            None,
            None,
            relation.database,
        ),
    ]


def build_bulk_columns_rows(*, relations: tuple[RelationInfo, ...]) -> list[tuple[object, ...]]:
    """Build INFORMATION_SCHEMA rows equivalent to the exact SHOW fixtures."""

    rows: list[tuple[object, ...]] = []
    relation: RelationInfo
    for relation in relations:
        rows.extend(
            (
                (relation.database, relation.schema, relation.name, "ID", "NUMBER", 20, 4, None),
                (
                    relation.database,
                    relation.schema,
                    relation.name,
                    "LABEL",
                    "TEXT",
                    None,
                    None,
                    128,
                ),
            )
        )
    return rows


def expected_qualified_columns(
    *, relations: tuple[RelationInfo, ...]
) -> dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]]:
    """Build normalized expected columns keyed by physical identity."""

    return {
        relation.identity: (
            ColumnInfo(name="id", type="NUMBER(20,4)"),
            ColumnInfo(name="label", type="VARCHAR(128)"),
        )
        for relation in relations
    }


def build_bulk_sequence_connection(
    *, relations: tuple[RelationInfo, ...], chunk_size: int
) -> FakeSnowflakeMetadataSequenceConnection:
    """Build one bulk metadata cursor per expected relation chunk."""

    cursors: list[FakeSnowflakeMetadataCursor] = []
    chunk_start: int
    for chunk_start in range(0, len(relations), chunk_size):
        chunk: tuple[RelationInfo, ...] = relations[chunk_start : chunk_start + chunk_size]
        cursors.append(FakeSnowflakeMetadataCursor(rows=build_bulk_columns_rows(relations=chunk)))
    return FakeSnowflakeMetadataSequenceConnection(tuple(cursors))


def describe_equivalent_numeric_relation(
    connection: object, relation: str
) -> tuple[ColumnInfo, ...]:
    del connection
    columns_by_relation: dict[str, tuple[ColumnInfo, ...]] = {
        "left_relation": (ColumnInfo(name="id", type="NUMBER(38,0)"),),
        "right_relation": (ColumnInfo(name="id", type="DECIMAL(38,0)"),),
    }
    return columns_by_relation[relation]


class FakeSnowflakeRawConnection:
    """Raw connector connection double for connect() tests."""

    def __init__(self) -> None:
        self.executed_sql: list[str] = []

    def cursor(self) -> Any:
        return self

    def execute(self, sql: str, **kwargs: object) -> Any:
        del kwargs
        self.executed_sql.append(sql)
        return self

    def close(self) -> None:
        pass


def install_fake_snowflake_connector(
    monkeypatch: Any,
) -> tuple[dict[str, object], FakeSnowflakeRawConnection]:
    """Install a fake optional snowflake.connector module and capture connect kwargs."""

    captured_kwargs: dict[str, object] = {}
    raw_connection: FakeSnowflakeRawConnection = FakeSnowflakeRawConnection()
    snowflake_module: ModuleType = ModuleType("snowflake")
    connector_module: ModuleType = ModuleType("snowflake.connector")

    def connect(**kwargs: object) -> FakeSnowflakeRawConnection:
        captured_kwargs.update(kwargs)
        return raw_connection

    connector_module.__dict__["connect"] = connect
    snowflake_module.__dict__["connector"] = connector_module
    monkeypatch.setitem(sys.modules, "snowflake", snowflake_module)
    monkeypatch.setitem(sys.modules, "snowflake.connector", connector_module)
    return captured_kwargs, raw_connection


def _raise_rejection(error: Exception) -> None:
    raise error


def _accept_statement(error: Exception) -> None:
    del error


_REJECTION_OUTCOMES: dict[bool, Callable[[Exception], None]] = {
    True: _raise_rejection,
    False: _accept_statement,
}


class FakeSnowflakeRejectingConnection:
    """Connection double that raises one driver error for statements containing a fragment."""

    def __init__(self, *, rejected_fragments: tuple[str, ...], error: Exception) -> None:
        self._rejected_fragments: tuple[str, ...] = rejected_fragments
        self._error: Exception = error
        self.executed_sql: list[str] = []

    def execute(self, sql: str) -> FakeSnowflakeDescribeCursor:
        self.executed_sql.append(sql)
        _REJECTION_OUTCOMES[all(fragment in sql for fragment in self._rejected_fragments)](
            self._error
        )
        return FakeSnowflakeDescribeCursor(description=(("status",),))


class FakeSnowflakeRejectingRawCursor:
    """Raw driver cursor that submits every statement and rejects those matching all fragments."""

    def __init__(self, *, rejections: tuple[tuple[tuple[str, ...], Exception], ...]) -> None:
        self._rejections: tuple[tuple[tuple[str, ...], Exception], ...] = rejections
        self.sfqid: str | None = None
        self.rowcount: int = 0
        self.executed_sql: list[str] = []

    def execute(self, sql: str, **kwargs: object) -> FakeSnowflakeRejectingRawCursor:
        del kwargs
        self.executed_sql.append(sql)
        self.sfqid = f"01c-query-{len(self.executed_sql)}"
        fragments: tuple[str, ...]
        error: Exception
        for fragments, error in self._rejections:
            _REJECTION_OUTCOMES[all(fragment in sql for fragment in fragments)](error)
        return self

    def close(self) -> None:
        return None


class FakeSnowflakeRejectingRawConnection:
    """Raw driver connection handing out one shared cursor."""

    def __init__(self, cursor: FakeSnowflakeRejectingRawCursor) -> None:
        self._cursor: FakeSnowflakeRejectingRawCursor = cursor

    def cursor(self) -> FakeSnowflakeRejectingRawCursor:
        return self._cursor


@dataclass(frozen=True)
class StatementProgressCapture:
    """Lifecycle statement events and CLI progress text published around one statement."""

    statement_events: tuple[str, ...]
    statement_fail_lines: tuple[str, ...]
    output: str
    error: Exception | None


def execute_with_statement_progress(
    *, adapter: SnowflakeAdapter, raw_cursor: FakeSnowflakeRejectingRawCursor, sql: str
) -> StatementProgressCapture:
    """Execute one statement through the real Snowflake cursor under a resource operation."""

    stream: StringIO = StringIO()
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)
    dispatcher.subscribe_lifecycle(
        subscriber=NativeProgressProjector(stream=stream, use_color=False, debug=True).consume,
        accepts_opaque=False,
    )
    connection: _SnowflakeConnection = _SnowflakeConnection(
        FakeSnowflakeRejectingRawConnection(raw_cursor)
    )
    error: Exception | None = None
    try:
        with (
            invocation_scope("inv-state-table-retention"),
            run_scope("run-state-table-retention"),
            dispatcher_scope(dispatcher),
            ResourceAttemptLifecycle(
                resource_id="model:orders", resource_kind="table", resource_name="orders"
            ),
            OperationLifecycle(
                operation_kind="warehouse",
                operation_name="staging_creation",
                attributes=OperationAttributes(phase="create", adapter="snowflake"),
            ),
        ):
            _ = adapter.execute(connection=connection, sql=sql)
    except Exception as caught:
        error = caught
    return StatementProgressCapture(
        statement_events=tuple(
            filter(
                lambda event_type: event_type.startswith("statement_"),
                (event.event_type for event in events),
            )
        ),
        statement_fail_lines=tuple(
            filter(
                lambda line: line.lstrip().startswith("statement") and "FAIL" in line,
                stream.getvalue().splitlines(),
            )
        ),
        output=stream.getvalue(),
        error=error,
    )


class CustomInitSnowflakeAdapter(SnowflakeAdapter):
    """Custom adapter whose own initializer does not call the base initializer."""

    def __init__(self) -> None:
        self.custom_label: str = "orders"


class FakeAccessHistoryError(Exception):
    """A Snowflake driver error with an error number."""

    def __init__(self, message: str, errno: int | None) -> None:
        super().__init__(message)
        self.errno: int | None = errno


class FakeUsageWarehouse:
    """Answers CURRENT_DATABASE() and access-history reads, recording each access-history read."""

    def __init__(
        self,
        *,
        current_database: str | None = None,
        access_rows: tuple[tuple[object, ...], ...] = (),
        dependency_rows: tuple[tuple[str, str], ...] = (),
        errors: tuple[Exception, ...] = (),
        dependency_errors: tuple[Exception, ...] = (),
    ) -> None:
        self.current_database: str | None = current_database
        self.access_rows: tuple[tuple[object, ...], ...] = access_rows
        self.dependency_rows: tuple[tuple[str, str], ...] = dependency_rows
        self.errors: tuple[Exception, ...] = errors
        self.dependency_errors: tuple[Exception, ...] = dependency_errors
        self.queries: list[tuple[str, tuple[object, ...]]] = []
        self.dependency_queries: list[tuple[str, tuple[object, ...]]] = []

    def current_database_rows(self) -> list[tuple[object, ...]]:
        return [(self.current_database,)]

    def access_history_rows(self, sql: str, params: tuple[object, ...]) -> list[tuple[object, ...]]:
        self.queries.append((sql, params))
        error: Exception
        for error in self.errors:
            raise error
        names: frozenset[object] = frozenset(params)
        return list(filter(lambda row: row[0] in names, self.access_rows))

    def object_dependency_rows(
        self, sql: str, params: tuple[object, ...]
    ) -> list[tuple[object, ...]]:
        self.dependency_queries.append((sql, params))
        error: Exception
        for error in self.dependency_errors:
            raise error
        databases: frozenset[object] = frozenset(params)
        return list(filter(lambda row: row[0].split(".")[0] in databases, self.dependency_rows))


class FakeUsageCursor:
    """Cursor double routing each statement to the fake usage warehouse."""

    def __init__(self, warehouse: FakeUsageWarehouse) -> None:
        self.warehouse: FakeUsageWarehouse = warehouse
        self.rows: list[tuple[object, ...]] = []

    def execute(self, sql: str, params: tuple[object, ...] = ()) -> None:
        reads: dict[tuple[bool, bool], Callable[[], list[tuple[object, ...]]]] = {
            (True, False): self.warehouse.current_database_rows,
            (False, True): lambda: self.warehouse.object_dependency_rows(sql, params),
            (False, False): lambda: self.warehouse.access_history_rows(sql, params),
        }
        self.rows = reads[(sql == "SELECT CURRENT_DATABASE()", "OBJECT_DEPENDENCIES" in sql)]()

    def fetchone(self) -> tuple[object, ...] | None:
        return next(iter(self.rows), None)

    def fetchall(self) -> list[tuple[object, ...]]:
        return self.rows

    def close(self) -> None:
        return None


class FakeUsageConnection:
    """Connection double handing out cursors over one fake usage warehouse."""

    def __init__(self, warehouse: FakeUsageWarehouse) -> None:
        self.warehouse: FakeUsageWarehouse = warehouse

    def cursor(self) -> FakeUsageCursor:
        return FakeUsageCursor(self.warehouse)


def usage_relation(
    database: str | None, schema: str, name: str, relation_type: str = "base table"
) -> RelationInfo:
    """A relation for usage reads, a base table unless given another type."""

    return RelationInfo(database=database, schema=schema, name=name, relation_type=relation_type)


def direct_read(
    object_name: str, user: str, query_count: int, read_at: object
) -> tuple[object, ...]:
    """An access-history result row for direct reads."""

    return (object_name, user, query_count, read_at, True, None)


def read_snowflake_usage(
    *, warehouse: FakeUsageWarehouse, relations: tuple[RelationInfo, ...], window_seconds: int
) -> RelationUsageReport:
    """Snowflake's usage report for the relations from the fake warehouse."""

    return SnowflakeAdapter().relation_usage(
        connection=FakeUsageConnection(warehouse),
        relations=relations,
        window_seconds=window_seconds,
    )


def retention_relation(
    schema: str,
    name: str,
    *,
    retention: int,
    transient: bool = False,
    table_type: str = "BASE TABLE",
) -> FakeRelation:
    """One synthetic relation in the ANALYTICS database with its retention."""

    return FakeRelation(
        database="ANALYTICS",
        schema=schema,
        name=name,
        table_type=table_type,
        is_transient=transient,
        retention_time=retention,
    )


def retention_request(*, schema: str, name: str, desired_days: int = 7) -> RetentionRequest:
    """A relation retention request in the analytics database, identified by its name."""

    return RetentionRequest(
        request_id=name,
        scope=RetentionScope.RELATION,
        database="analytics",
        schema=schema,
        name=name,
        desired_days=desired_days,
    )


def show_statement(sql: str) -> str:
    """A SHOW statement without its schema scope after LIKE, or its row limit after IN SCHEMA."""

    return (sql.split(" LIMIT ")[0], sql.split(" IN SCHEMA ")[0])[" LIKE " in sql]


def _lookup_exists(
    *, adapter: OfflineSnowflakeAdapter, connection: Any, schema: str, name: str
) -> None:
    _ = adapter.relation_exists(
        connection=connection, database="analytics", schema=schema, name=name
    )


def _lookup_retention(
    *, adapter: OfflineSnowflakeAdapter, connection: Any, schema: str, name: str
) -> int | None:
    return adapter.inspect_retention(
        connection=connection, request=retention_request(schema=schema, name=name)
    ).effective_days


def _replace_table(
    *, adapter: OfflineSnowflakeAdapter, connection: Any, schema: str, name: str
) -> None:
    _ = adapter.execute(
        connection=connection, sql=f"CREATE OR REPLACE TABLE analytics.{schema}.{name} (id INT)"
    )


_LOOKUP_STEPS: dict[str, Callable[..., int | None]] = {
    "exists": _lookup_exists,
    "retention": _lookup_retention,
    "replace": _replace_table,
}


def run_metadata_steps(
    *,
    relations: tuple[FakeRelation, ...],
    steps: tuple[tuple[str, str, str], ...],
    in_build: bool,
) -> tuple[tuple[int | None, tuple[str, ...]], ...]:
    """Run each (kind, schema, name) step; return its retention days and SHOW statements."""

    warehouse: RecordingSnowflakeWarehouse = RecordingSnowflakeWarehouse(relations=relations)
    adapter: OfflineSnowflakeAdapter = OfflineSnowflakeAdapter(warehouse=warehouse)
    connection: Any = adapter.connect({})
    results: list[tuple[int | None, tuple[str, ...]]] = []
    with (nullcontext(), relation_metadata_cache_scope(adapter=adapter))[in_build]:
        kind: str
        schema: str
        name: str
        for kind, schema, name in steps:
            warehouse.reset()
            days: int | None = _LOOKUP_STEPS[kind](
                adapter=adapter, connection=connection, schema=schema, name=name
            )
            results.append(
                (
                    days,
                    tuple(show_statement(query.sql) for query in warehouse.queries),
                )
            )
    return tuple(results)


def relation_existence(
    *, relations: tuple[FakeRelation, ...], lookups: tuple[str, ...], in_build: bool
) -> tuple[tuple[bool, ...], int]:
    """Each name's existence in the MART schema, and how many whole-schema listings were read."""

    warehouse: RecordingSnowflakeWarehouse = RecordingSnowflakeWarehouse(relations=relations)
    adapter: OfflineSnowflakeAdapter = OfflineSnowflakeAdapter(warehouse=warehouse)
    connection: Any = adapter.connect({})
    with (nullcontext(), relation_metadata_cache_scope(adapter=adapter))[in_build]:
        answers: tuple[bool, ...] = tuple(
            adapter.relation_exists(
                connection=connection, database="analytics", schema="mart", name=name
            )
            for name in lookups
        )
    listings: int = len(
        tuple(
            filter(
                lambda sql: " IN SCHEMA " in sql and " LIKE " not in sql, warehouse.attempted_sql
            )
        )
    )
    return answers, listings


def prefix_listing_statements(warehouse: RecordingSnowflakeWarehouse) -> tuple[str, ...]:
    """The SHOW TERSE OBJECTS statements the warehouse received, without their schema scope."""

    return tuple(
        sql.split(" IN SCHEMA ")[0]
        for sql in filter(lambda sql: sql.startswith("SHOW TERSE OBJECTS"), warehouse.attempted_sql)
    )


class RecordingSnowflakeAdapter(SnowflakeAdapter):
    """Snowflake adapter that records each statement instead of running it."""

    def __init__(self) -> None:
        super().__init__()
        self.executed: list[str] = []

    def execute(self, *, connection: Any, sql: str, **kwargs: Any) -> Any:
        del connection, kwargs
        self.executed.append(sql)
        return SimpleNamespace(rowcount=3)
