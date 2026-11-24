"""Recording Snowflake warehouse double for inspection round-trip tests.

The double answers ``INFORMATION_SCHEMA`` reads by evaluating the issued SQL against an in-memory
DuckDB copy of synthetic metadata, so filtering, ordering, and case semantics come from the SQL the
adapter actually sends rather than from hand-written fakes.
"""

from __future__ import annotations

import json
import re
import threading
import time
from collections.abc import Callable, Iterator
from dataclasses import dataclass, field, replace
from datetime import UTC, datetime
from itertools import chain, compress
from pathlib import Path
from typing import Any, ClassVar

import duckdb
import pyarrow

from sqlbuild.adapter.contract.models import ColumnInfo, RelationInfo
from sqlbuild.adapter.relations.classes.inspection_catalog import InspectionCatalog
from sqlbuild.adapter.relations.classes.synced_catalog import SyncedCatalog
from sqlbuild.adapter.relations.models import SyncedRelation
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_connection import _SnowflakeConnection

_METADATA_RELATION_PATTERN: re.Pattern[str] = re.compile(
    r'(?:"(?P<database>[^"]*)"\.)?information_schema\.(?P<view>tables|columns)\b',
    re.IGNORECASE,
)
_OTHER_INFORMATION_SCHEMA_PATTERN: re.Pattern[str] = re.compile(
    r"information_schema\.", re.IGNORECASE
)
_SHOW_COLUMNS_PATTERN: re.Pattern[str] = re.compile(
    r"^\s*SHOW\s+COLUMNS\s+IN\s+(?:TABLE|VIEW)\s+(?P<relation>\S+)\s*$", re.IGNORECASE
)
_SHOW_SCHEMA_RELATIONS_PATTERN: re.Pattern[str] = re.compile(
    r"^SHOW (?P<terse>TERSE )?(?P<kind>TABLES|VIEWS|OBJECTS)(?: LIKE '(?P<like>(?:[^']|'')*)')? "
    r"IN SCHEMA (?P<scope>\S+)"
    r"(?: LIMIT (?P<limit>\d+)(?: FROM '(?P<after>(?:[^']|'')*)')?)?$"
)
_SHOW_SCHEMA_COLUMNS_PATTERN: re.Pattern[str] = re.compile(
    r"^SHOW COLUMNS IN SCHEMA (?P<scope>\S+)$"
)
_SHOW_PATTERN: re.Pattern[str] = re.compile(r"^\s*SHOW\s", re.IGNORECASE)
_CURSOR_BOUND_PATTERN: re.Pattern[str] = re.compile(
    r"^SELECT\s+CAST\((?:MIN|MAX)\(.*\)\s+AS\s+VARCHAR\).*\s+FROM\s+(?P<relation>\S+)\s*$",
    re.IGNORECASE | re.DOTALL,
)
_CURSOR_BATCH_PATTERN: re.Pattern[str] = re.compile(r"^SELECT \d+ AS _read, ", re.IGNORECASE)
_CURSOR_BATCH_BRANCH: re.Pattern[str] = re.compile(
    r"^SELECT (?P<read>\d+) AS _read, (?P<select>.*?)(?:\s+FROM\s+(?P<relation>[^\s()]+))?$",
    re.IGNORECASE | re.DOTALL,
)
_DESCRIBE_PATTERN: re.Pattern[str] = re.compile(r"\) AS __describe_source LIMIT 0$")
_ANY_PATTERN: re.Pattern[str] = re.compile(r"")
_CURRENT_DATABASE_PATTERN: re.Pattern[str] = re.compile(r"^SELECT CURRENT_DATABASE\(\)$")
_USE_DATABASE_PATTERN: re.Pattern[str] = re.compile(
    r'^(?s:.*?)USE DATABASE "?(?P<database>[^";]+)"?(?s:.*)$'
)
_SHOW_SCHEMAS_PATTERN: re.Pattern[str] = re.compile(
    r"^SHOW SCHEMAS LIKE '(?P<pattern>(?:[^']|'')*)' IN DATABASE \"(?P<database>[^\"]*)\"$"
)
_IN_LIST_PATTERN: re.Pattern[str] = re.compile(r"\bIN\s*\(([^()]*)\)", re.IGNORECASE)
_TABLES_PATTERN: re.Pattern[str] = re.compile(r"information_schema\.tables\b", re.IGNORECASE)
_COLUMNS_PATTERN: re.Pattern[str] = re.compile(r"information_schema\.columns\b", re.IGNORECASE)
_QUERY_KINDS: tuple[tuple[re.Pattern[str], str], ...] = (
    (_TABLES_PATTERN, "tables"),
    (_COLUMNS_PATTERN, "columns"),
    (_SHOW_COLUMNS_PATTERN, "show_columns"),
    (re.compile(r"^SHOW TERSE (?:TABLES|VIEWS) LIKE "), "show_relation_lookup"),
    (re.compile(r"^SHOW TABLES IN SCHEMA "), "show_tables"),
    (re.compile(r"^SHOW VIEWS IN SCHEMA "), "show_views"),
    (_SHOW_SCHEMA_COLUMNS_PATTERN, "show_schema_columns"),
    (_CURSOR_BOUND_PATTERN, "cursor_bounds"),
    (_CURSOR_BATCH_PATTERN, "cursor_bounds"),
    (_CURRENT_DATABASE_PATTERN, "session"),
    (_SHOW_SCHEMAS_PATTERN, "show_schemas"),
    (_OTHER_INFORMATION_SCHEMA_PATTERN, "other_metadata"),
    (_SHOW_PATTERN, "other_metadata"),
    (_ANY_PATTERN, "data"),
)
_FIXED_CREATED_AT: datetime = datetime(2026, 1, 1, tzinfo=UTC)
_SHOW_TYPE_NAMES: dict[str, str] = {"NUMBER": "FIXED", "FLOAT": "REAL"}
_STATUS_DESCRIPTION: tuple[tuple[str], ...] = (("status",),)
_SHOW_DESCRIPTION: tuple[tuple[str], ...] = (
    ("table_name",),
    ("schema_name",),
    ("column_name",),
    ("data_type",),
)
_BOUND_MARKERS: tuple[str, str] = ("MIN(", "MAX(")
_BOUND_VALUE_INDEX: dict[bool, int] = {True: 0, False: 1}
SHOW_RESULT_CAP: int = 10_000
# The relation count beyond which schema listing tests read a schema's columns in one listing.
SCHEMA_LISTING_EXACT_COLUMN_LIMIT: int = 32
_SHOW_TABLE_FIELDS: tuple[str, ...] = (
    "created_on",
    "name",
    "database_name",
    "schema_name",
    "kind",
    "comment",
    "cluster_by",
    "rows",
    "bytes",
    "owner",
    "retention_time",
    "is_external",
    "is_event",
    "is_dynamic",
)
_SHOW_VIEW_FIELDS: tuple[str, ...] = (
    "created_on",
    "name",
    "reserved",
    "database_name",
    "schema_name",
    "owner",
    "comment",
    "text",
    "is_secure",
    "is_materialized",
)
_SHOW_TERSE_FIELDS: tuple[str, ...] = (
    "created_on",
    "name",
    "kind",
    "database_name",
    "schema_name",
)
_SHOW_SCHEMA_COLUMN_FIELDS: tuple[str, ...] = (
    "table_name",
    "schema_name",
    "column_name",
    "data_type",
    "null?",
    "default",
    "kind",
    "expression",
    "comment",
    "database_name",
    "autoincrement",
)
_VIEW_TABLE_TYPES: frozenset[str] = frozenset({"VIEW", "MATERIALIZED VIEW"})
_OBJECT_KINDS: dict[str, str] = {"BASE TABLE": "TABLE", "VIEW": "VIEW"}
_TIMESTAMP_TYPES: frozenset[str] = frozenset(
    {"TIMESTAMP_NTZ", "TIMESTAMP_LTZ", "TIMESTAMP_TZ", "TIME"}
)


@dataclass(frozen=True)
class FakeColumn:
    """One synthetic Snowflake column with INFORMATION_SCHEMA type fields."""

    name: str
    data_type: str = "TEXT"
    numeric_precision: int | None = None
    numeric_scale: int | None = None
    character_maximum_length: int | None = 16_777_216


@dataclass(frozen=True)
class FakeRelation:
    """One synthetic relation with its stored (case-exact) Snowflake name."""

    database: str
    schema: str
    name: str
    table_type: str = "BASE TABLE"
    is_transient: bool = False
    columns: tuple[FakeColumn, ...] = (FakeColumn(name="ID", data_type="NUMBER"),)
    retention_time: int = 1
    last_ddl: datetime = datetime(2026, 1, 1, tzinfo=UTC)


@dataclass(frozen=True)
class RecordedQuery:
    """One statement issued against the double."""

    sql: str
    params: tuple[object, ...]
    thread_id: int
    started_at: float
    finished_at: float
    row_count: int

    @property
    def kind(self) -> str:
        """Classify the statement for round-trip budgets."""

        return next(filter(lambda rule: rule[0].search(self.sql) is not None, _QUERY_KINDS))[1]

    @property
    def largest_in_list(self) -> int:
        """Return the largest literal or placeholder IN list in the statement."""

        return max(
            (len(match.group(1).split(",")) for match in _IN_LIST_PATTERN.finditer(self.sql)),
            default=0,
        )


@dataclass
class RecordingSnowflakeWarehouse:
    """Thread-safe in-memory Snowflake metadata and cursor-bound responder."""

    relations: tuple[FakeRelation, ...]
    statement_latency_seconds: float = 0.0
    failing_relations: frozenset[str] = frozenset()
    failing_metadata_schemas: frozenset[str] = frozenset()
    current_database: str = "ANALYTICS"
    extra_schemas: frozenset[tuple[str, str]] = frozenset()
    forbidden_schemas: frozenset[str] = frozenset()
    cursor_values: tuple[str, str] = ("2026-01-01 00:00:00.000", "2026-01-31 00:00:00.000")
    queries: list[RecordedQuery] = field(default_factory=list)
    attempted_sql: list[str] = field(default_factory=list)
    _lock: threading.Lock = field(default_factory=threading.Lock)
    _active: dict[bool, int] = field(default_factory=lambda: {True: 0, False: 0})
    _peak: dict[bool, int] = field(default_factory=lambda: {True: 0, False: 0})

    def __post_init__(self) -> None:
        self._database: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
        self._database.execute(
            "CREATE TABLE fake_tables (table_catalog VARCHAR, table_schema VARCHAR, "
            "table_name VARCHAR, table_type VARCHAR, is_transient VARCHAR, "
            "created TIMESTAMPTZ, last_altered TIMESTAMPTZ, last_ddl TIMESTAMPTZ, "
            "retention_time INTEGER)"
        )
        self._database.execute(
            "CREATE TABLE fake_columns (table_catalog VARCHAR, table_schema VARCHAR, "
            "table_name VARCHAR, column_name VARCHAR, ordinal_position INTEGER, "
            "data_type VARCHAR, numeric_precision INTEGER, numeric_scale INTEGER, "
            "character_maximum_length INTEGER)"
        )
        self._load(table="fake_tables", rows=[_table_row(relation) for relation in self.relations])
        column_rows: list[dict[str, object]] = []
        relation: FakeRelation
        for relation in self.relations:
            column_rows.extend(_column_rows(relation))
        self._load(table="fake_columns", rows=column_rows)

    @property
    def max_concurrent_cursor_bounds(self) -> int:
        """Return the highest number of cursor-bound statements in flight at once."""

        return self._peak[True]

    @property
    def max_concurrent_metadata(self) -> int:
        """Return the highest number of other statements in flight at once."""

        return self._peak[False]

    def raw_connection(self) -> FakeRawSnowflakeConnection:
        """Return a raw DB-API connection double bound to this warehouse."""

        return FakeRawSnowflakeConnection(warehouse=self)

    def queries_of_kind(self, kind: str) -> tuple[RecordedQuery, ...]:
        """Return recorded statements of one classification."""

        with self._lock:
            return tuple(filter(lambda query: query.kind == kind, self.queries))

    def replace_relations(self, *, relations: tuple[FakeRelation, ...]) -> None:
        """Replace the catalog that SHOW and INFORMATION_SCHEMA reads answer from."""

        self.relations = relations
        self._database.execute("DELETE FROM fake_tables")
        self._database.execute("DELETE FROM fake_columns")
        self._load(table="fake_tables", rows=[_table_row(relation) for relation in relations])
        column_rows: list[dict[str, object]] = []
        relation: FakeRelation
        for relation in relations:
            column_rows.extend(_column_rows(relation))
        self._load(table="fake_columns", rows=column_rows)

    def reset(self) -> None:
        """Forget recorded statements and concurrency peaks."""

        with self._lock:
            self.queries.clear()
            self.attempted_sql.clear()
            self._peak.update({True: 0, False: 0})

    def run(self, *, sql: str, params: tuple[object, ...]) -> tuple[list[tuple[Any, ...]], Any]:
        """Execute one statement and record it."""

        started_at: float = time.monotonic()
        is_cursor_bound: bool = (
            _CURSOR_BOUND_PATTERN.match(sql) is not None
            or _CURSOR_BATCH_PATTERN.match(sql) is not None
        )
        with self._lock:
            self.attempted_sql.append(sql)
            self._active[is_cursor_bound] += 1
            self._peak[is_cursor_bound] = max(
                self._peak[is_cursor_bound], self._active[is_cursor_bound]
            )
        try:
            time.sleep(self.statement_latency_seconds)
            rows, description = self._answer(sql=sql, params=params)
        finally:
            with self._lock:
                self._active[is_cursor_bound] -= 1
        with self._lock:
            self.queries.append(
                RecordedQuery(
                    sql=sql,
                    params=params,
                    thread_id=threading.get_ident(),
                    started_at=started_at,
                    finished_at=time.monotonic(),
                    row_count=len(rows),
                )
            )
        return rows, description

    def _load(self, *, table: str, rows: list[dict[str, object]]) -> None:
        _ROW_LOADERS[bool(rows)](self, table, rows)

    def _insert_rows(self, table: str, rows: list[dict[str, object]]) -> None:
        staged: pyarrow.Table = pyarrow.Table.from_pylist(rows)
        self._database.register("staged_rows", staged)
        self._database.execute(f"INSERT INTO {table} BY NAME SELECT * FROM staged_rows")
        self._database.unregister("staged_rows")

    def _answer(self, *, sql: str, params: tuple[object, ...]) -> tuple[list[tuple[Any, ...]], Any]:
        routes: tuple[tuple[re.Pattern[str], Callable[..., Any]], ...] = (
            (_METADATA_RELATION_PATTERN, self._answer_information_schema),
            (_OTHER_INFORMATION_SCHEMA_PATTERN, _answer_empty_metadata),
            (_SHOW_COLUMNS_PATTERN, self._answer_show_columns),
            (_SHOW_SCHEMA_RELATIONS_PATTERN, self._answer_show_schema_relations),
            (_SHOW_SCHEMA_COLUMNS_PATTERN, self._answer_show_schema_columns),
            (_DESCRIBE_PATTERN, self._answer_describe),
            (_CURSOR_BATCH_PATTERN, self._answer_cursor_batch),
            (_CURSOR_BOUND_PATTERN, self._answer_cursor_bounds),
            (_CURRENT_DATABASE_PATTERN, self._answer_current_database),
            (_USE_DATABASE_PATTERN, self._answer_use_database),
            (_SHOW_SCHEMAS_PATTERN, self._answer_show_schemas),
            (_ANY_PATTERN, _answer_status),
        )
        route: tuple[re.Pattern[str], Callable[..., Any]] = next(
            filter(lambda candidate: candidate[0].search(sql) is not None, routes)
        )
        return route[1](sql=sql, params=params)

    def _answer_information_schema(
        self, *, sql: str, params: tuple[object, ...]
    ) -> tuple[list[tuple[Any, ...]], Any]:
        _MISSING_RELATION_ACTIONS[
            not self.failing_metadata_schemas.isdisjoint(str(param) for param in params)
        ](sql)
        duckdb_sql: str = _METADATA_RELATION_PATTERN.sub(
            self._scope_metadata_relation, sql
        ).replace("%s", "?")
        with self._lock:
            cursor: duckdb.DuckDBPyConnection = self._database.cursor()
        result: duckdb.DuckDBPyConnection = cursor.execute(duckdb_sql, list(params))
        rows: list[tuple[Any, ...]] = result.fetchall()
        return rows, result.description

    def _answer_show_columns(
        self, *, sql: str, params: tuple[object, ...]
    ) -> tuple[list[tuple[Any, ...]], Any]:
        del params
        relation: str = next(_SHOW_COLUMNS_PATTERN.finditer(sql)).group("relation")
        parts: list[str] = [part.strip('"').upper() for part in relation.split(".")]
        matches: tuple[FakeRelation, ...] = tuple(
            filter(
                lambda fake: (fake.database, fake.schema, fake.name)[-len(parts) :] == tuple(parts),
                self.relations,
            )
        )
        rows: list[tuple[Any, ...]] = []
        fake: FakeRelation
        for fake in matches:
            rows.extend(
                (fake.name, fake.schema, column.name, json.dumps(_show_columns_type(column)))
                for column in fake.columns
            )
        _MISSING_RELATION_ACTIONS[not rows](relation)
        return rows, _SHOW_DESCRIPTION

    def _scope_metadata_relation(self, match: re.Match[str]) -> str:
        table: str = f"fake_{match.group('view').lower()}"
        database: str = (match.group("database") or self.current_database).replace("'", "''")
        return f"(SELECT * FROM {table} WHERE table_catalog = '{database}') AS {table}"

    def _answer_current_database(
        self, *, sql: str, params: tuple[object, ...]
    ) -> tuple[list[tuple[Any, ...]], Any]:
        del sql, params
        return [(self.current_database,)], (("CURRENT_DATABASE()",),)

    def _answer_use_database(
        self, *, sql: str, params: tuple[object, ...]
    ) -> tuple[list[tuple[Any, ...]], Any]:
        del params
        with self._lock:
            self.current_database = next(_USE_DATABASE_PATTERN.finditer(sql)).group("database")
        return [], _STATUS_DESCRIPTION

    def _existing_schemas(self) -> frozenset[tuple[str, str]]:
        return self.extra_schemas | {(fake.database, fake.schema) for fake in self.relations}

    def _schema_relations(
        self, *, scope: str, reports_missing_database: bool
    ) -> tuple[FakeRelation, ...]:
        """Mimic Snowflake: SHOW TABLES/VIEWS report 2043 for a missing database or schema."""

        parts: tuple[str, ...] = (
            self.current_database,
            *(part.strip('"') for part in scope.split(".")),
        )[-2:]
        _FAILED_SCHEMA_ACTIONS[parts[-1] in self.failing_metadata_schemas](scope)
        existing: frozenset[tuple[str, str]] = self._existing_schemas()
        database_missing: bool = parts[0] not in {database for database, _ in existing}
        _MISSING_DATABASE_ACTIONS[database_missing and reports_missing_database](parts[0])
        _OBJECT_MISSING_ACTIONS[parts not in existing or parts[-1] in self.forbidden_schemas](scope)
        return tuple(filter(lambda fake: (fake.database, fake.schema) == parts, self.relations))

    def _answer_show_schemas(
        self, *, sql: str, params: tuple[object, ...]
    ) -> tuple[list[tuple[Any, ...]], Any]:
        del params
        match: re.Match[str] = next(_SHOW_SCHEMAS_PATTERN.finditer(sql))
        database: str = match.group("database")
        existing: frozenset[tuple[str, str]] = self._existing_schemas()
        _MISSING_DATABASE_ACTIONS[database not in {name for name, _ in existing}](database)
        pattern: re.Pattern[str] = _like_pattern(match.group("pattern"))
        names: list[str] = sorted(
            schema
            for name, schema in filter(
                lambda entry: entry[0] == database and pattern.fullmatch(entry[1]) is not None,
                existing,
            )
        )
        return [(_FIXED_CREATED_AT, name, database) for name in names], (
            ("created_on",),
            ("name",),
            ("database_name",),
        )

    def _answer_show_schema_relations(
        self, *, sql: str, params: tuple[object, ...]
    ) -> tuple[list[tuple[Any, ...]], Any]:
        del params
        match: re.Match[str] = next(_SHOW_SCHEMA_RELATIONS_PATTERN.finditer(sql))
        wants_views: bool = match.group("kind") == "VIEWS"
        wants_objects: bool = match.group("kind") == "OBJECTS"
        after: str = (match.group("after") or "").replace("''", "'")
        like: re.Pattern[str] = _like_pattern(match.group("like") or "%")
        selected: list[FakeRelation] = sorted(
            filter(
                lambda fake: (
                    (wants_objects or (fake.table_type in _VIEW_TABLE_TYPES) is wants_views)
                    and fake.name > after
                    and like.fullmatch(fake.name) is not None
                ),
                self._schema_relations(scope=match.group("scope"), reports_missing_database=False),
            ),
            key=lambda fake: fake.name,
        )
        limit: int = int(match.group("limit") or SHOW_RESULT_CAP)
        terse: bool = match.group("terse") is not None
        row_builder: Callable[[FakeRelation], tuple[Any, ...]] = (
            (
                (_show_table_row, _show_view_row)[wants_views],
                _show_terse_row,
            )[terse],
            _show_object_row,
        )[wants_objects]
        fields: tuple[str, ...] = (
            (
                (_SHOW_TABLE_FIELDS, _SHOW_VIEW_FIELDS)[wants_views],
                _SHOW_TERSE_FIELDS,
            )[terse],
            _SHOW_TERSE_FIELDS,
        )[wants_objects]
        return [row_builder(fake) for fake in selected[:limit]], tuple((f,) for f in fields)

    def _answer_show_schema_columns(
        self, *, sql: str, params: tuple[object, ...]
    ) -> tuple[list[tuple[Any, ...]], Any]:
        del params
        scope: str = next(_SHOW_SCHEMA_COLUMNS_PATTERN.finditer(sql)).group("scope")
        rows: list[tuple[Any, ...]] = []
        fake: FakeRelation
        for fake in sorted(
            self._schema_relations(scope=scope, reports_missing_database=True),
            key=lambda item: item.name,
        ):
            rows.extend(
                (
                    fake.name,
                    fake.schema,
                    column.name,
                    json.dumps(_show_columns_type(column)),
                    "true",
                    "",
                    "COLUMN",
                    "",
                    "",
                    fake.database,
                    "",
                )
                for column in fake.columns
            )
        return rows, tuple((f,) for f in _SHOW_SCHEMA_COLUMN_FIELDS)

    def _answer_cursor_bounds(
        self, *, sql: str, params: tuple[object, ...]
    ) -> tuple[list[tuple[Any, ...]], Any]:
        del params
        relation: str = next(_CURSOR_BOUND_PATTERN.finditer(sql)).group("relation")
        _MISSING_RELATION_ACTIONS[relation.lower() in self.failing_relations](relation)
        select_list: str = sql.split(" FROM ")[0]
        values: tuple[str, ...] = tuple(
            compress(self.cursor_values, (marker in select_list for marker in _BOUND_MARKERS))
        )
        return [values], tuple((f"_{index}",) for index in range(len(values)))

    def _answer_describe(
        self, *, sql: str, params: tuple[object, ...]
    ) -> tuple[list[tuple[Any, ...]], Any]:
        """A query's columns, named as Snowflake folds unquoted identifiers, and no rows."""

        del params
        with self._lock:
            description: Any = self._database.execute(sql).description
        return [], tuple((str(column[0]).upper(),) for column in description)

    def _answer_cursor_batch(
        self, *, sql: str, params: tuple[object, ...]
    ) -> tuple[list[tuple[Any, ...]], Any]:
        """One ``(_read, _min, _max)`` row per branch; a failing relation fails the statement."""

        del params
        rows: list[tuple[Any, ...]] = []
        branch: str
        for branch in sql.split("\nUNION ALL\n"):
            matched: re.Match[str] | None = _CURSOR_BATCH_BRANCH.match(branch)
            assert matched is not None, branch
            relation: str = matched.group("relation") or ""
            _MISSING_RELATION_ACTIONS[relation.lower() in self.failing_relations](relation)
            bounds: tuple[str, ...] = tuple(
                self.cursor_values[_BOUND_VALUE_INDEX["MIN(" in part]]
                for part in matched.group("select").split(" AS _")[:-1]
            )
            rows.append((int(matched.group("read")), *bounds))
        return rows, (("_read",), *((f"_{index}",) for index in range(len(rows[0]) - 1)))


def _like_pattern(pattern: str) -> re.Pattern[str]:
    return re.compile(
        re.escape(pattern.replace("''", "'")).replace("%", ".*").replace("_", "."),
        re.IGNORECASE,
    )


def _accept_relation(relation: str) -> None:
    del relation


def _raise_unavailable_relation(relation: str) -> None:
    raise RuntimeError(f"Object '{relation}' does not exist or not authorized.")


_MISSING_RELATION_ACTIONS: dict[bool, Callable[[str], None]] = {
    False: _accept_relation,
    True: _raise_unavailable_relation,
}


def _skip_rows(warehouse: RecordingSnowflakeWarehouse, table: str, rows: list[Any]) -> None:
    del warehouse, table, rows


_ROW_LOADERS: dict[bool, Callable[[RecordingSnowflakeWarehouse, str, list[Any]], None]] = {
    False: _skip_rows,
    True: RecordingSnowflakeWarehouse._insert_rows,
}


class FakeSnowflakeProgrammingError(RuntimeError):
    """Driver-style error carrying a Snowflake error number."""

    def __init__(self, message: str, *, errno: int) -> None:
        super().__init__(message)
        self.errno: int = errno


def _raise_missing_object(scope: str) -> None:
    del scope
    raise FakeSnowflakeProgrammingError(
        "002043 (02000): SQL compilation error: Object does not exist, or operation cannot be "
        "performed.",
        errno=2043,
    )


def _raise_failed_schema(scope: str) -> None:
    raise FakeSnowflakeProgrammingError(f"Metadata service unavailable for {scope}.", errno=390)


def _raise_missing_database(database: str) -> None:
    raise FakeSnowflakeProgrammingError(
        f"002003 (02000): SQL compilation error: Database '{database}' does not exist or not "
        "authorized. Your primary role must have USAGE on the database.",
        errno=2003,
    )


_MISSING_DATABASE_ACTIONS: dict[bool, Callable[[str], None]] = {
    False: _accept_relation,
    True: _raise_missing_database,
}
_OBJECT_MISSING_ACTIONS: dict[bool, Callable[[str], None]] = {
    False: _accept_relation,
    True: _raise_missing_object,
}
_FAILED_SCHEMA_ACTIONS: dict[bool, Callable[[str], None]] = {
    False: _accept_relation,
    True: _raise_failed_schema,
}


def _show_table_row(relation: FakeRelation) -> tuple[Any, ...]:
    external: bool = relation.table_type == "EXTERNAL TABLE"
    return (
        _FIXED_CREATED_AT,
        relation.name,
        relation.database,
        relation.schema,
        ("TABLE", "TRANSIENT")[relation.is_transient],
        "",
        "",
        0,
        0,
        "SYSADMIN",
        str(relation.retention_time),
        ("N", "Y")[external],
        "N",
        "N",
    )


def _show_terse_row(relation: FakeRelation) -> tuple[Any, ...]:
    return (
        _FIXED_CREATED_AT,
        relation.name,
        relation.table_type,
        relation.database,
        relation.schema,
    )


def _show_object_row(relation: FakeRelation) -> tuple[Any, ...]:
    """A SHOW TERSE OBJECTS row, whose kind names the object type as Snowflake spells it."""

    return (
        _FIXED_CREATED_AT,
        relation.name,
        _OBJECT_KINDS.get(relation.table_type, relation.table_type.replace(" ", "_")),
        relation.database,
        relation.schema,
    )


def _show_view_row(relation: FakeRelation) -> tuple[Any, ...]:
    return (
        _FIXED_CREATED_AT,
        relation.name,
        "",
        relation.database,
        relation.schema,
        "SYSADMIN",
        "",
        "",
        "false",
        ("false", "true")[relation.table_type == "MATERIALIZED VIEW"],
    )


def _answer_empty_metadata(
    *, sql: str, params: tuple[object, ...]
) -> tuple[list[tuple[Any, ...]], Any]:
    del sql, params
    return [], (("value",),)


def _answer_status(*, sql: str, params: tuple[object, ...]) -> tuple[list[tuple[Any, ...]], Any]:
    del sql, params
    return [], _STATUS_DESCRIPTION


def _table_row(relation: FakeRelation) -> dict[str, object]:
    return {
        "table_catalog": relation.database,
        "table_schema": relation.schema,
        "table_name": relation.name,
        "table_type": relation.table_type,
        "is_transient": (("NO", "YES")[relation.is_transient], None)[
            relation.table_type in _VIEW_TABLE_TYPES
        ],
        "created": _FIXED_CREATED_AT,
        "last_altered": _FIXED_CREATED_AT,
        "last_ddl": relation.last_ddl,
        "retention_time": (relation.retention_time, None)[relation.table_type in _VIEW_TABLE_TYPES],
    }


def _column_rows(relation: FakeRelation) -> list[dict[str, object]]:
    rows: list[dict[str, object]] = []
    position: int
    column: FakeColumn
    for position, column in enumerate(relation.columns, start=1):
        rows.append(
            {
                "table_catalog": relation.database,
                "table_schema": relation.schema,
                "table_name": relation.name,
                "column_name": column.name,
                "ordinal_position": position,
                "data_type": column.data_type,
                "numeric_precision": column.numeric_precision,
                "numeric_scale": column.numeric_scale,
                "character_maximum_length": column.character_maximum_length,
            }
        )
    return rows


def _show_columns_type(column: FakeColumn) -> dict[str, object]:
    candidates: tuple[tuple[str, object], ...] = (
        ("type", _SHOW_TYPE_NAMES.get(column.data_type, column.data_type)),
        ("nullable", True),
        ("precision", (column.numeric_precision, 0)[column.data_type in _TIMESTAMP_TYPES]),
        ("scale", (column.numeric_scale, 9)[column.data_type in _TIMESTAMP_TYPES]),
        ("length", (None, column.character_maximum_length)[column.data_type == "TEXT"]),
    )
    return dict(filter(lambda item: item[1] is not None, candidates))


class FakeRawSnowflakeCursor:
    """DB-API cursor double that delegates statements to the recording warehouse."""

    def __init__(self, *, warehouse: RecordingSnowflakeWarehouse) -> None:
        self._warehouse: RecordingSnowflakeWarehouse = warehouse
        self._rows: list[tuple[Any, ...]] = []
        self.description: Any = None
        self.rowcount: int = -1
        self.sfqid: str | None = None

    def execute(self, sql: str, params: Any = None, **kwargs: Any) -> FakeRawSnowflakeCursor:
        del kwargs
        rows, description = self._warehouse.run(sql=sql, params=tuple(params or ()))
        self._rows = list(rows)
        self.description = description
        self.rowcount = len(rows)
        return self

    def fetchall(self) -> list[tuple[Any, ...]]:
        rows: list[tuple[Any, ...]] = self._rows
        self._rows = []
        return rows

    def fetchone(self) -> tuple[Any, ...] | None:
        first: list[tuple[Any, ...]] = self._rows[:1]
        self._rows = self._rows[1:]
        return next(iter(first), None)

    def fetchmany(self, size: int) -> list[tuple[Any, ...]]:
        rows: list[tuple[Any, ...]] = self._rows[:size]
        self._rows = self._rows[size:]
        return rows

    def close(self) -> None:
        self._rows = []

    def __iter__(self) -> Iterator[tuple[Any, ...]]:
        return iter(self.fetchall())


class FakeRawSnowflakeConnection:
    """Raw connection double handing out cursors bound to one recording warehouse."""

    def __init__(self, *, warehouse: RecordingSnowflakeWarehouse) -> None:
        self._warehouse: RecordingSnowflakeWarehouse = warehouse

    def cursor(self) -> FakeRawSnowflakeCursor:
        return FakeRawSnowflakeCursor(warehouse=self._warehouse)

    def close(self) -> None:
        return None


class OfflineSnowflakeAdapter(SnowflakeAdapter):
    """Snowflake adapter whose connections reach the recording warehouse instead of the network."""

    def __init__(self, *, warehouse: RecordingSnowflakeWarehouse) -> None:
        super().__init__()
        self._recording_warehouse: RecordingSnowflakeWarehouse = warehouse

    def connect(self, config: dict[str, Any]) -> _SnowflakeConnection:
        del config
        return _SnowflakeConnection(self._recording_warehouse.raw_connection())


class SchemaListingSnowflakeAdapter(OfflineSnowflakeAdapter):
    """An offline adapter reading a schema's columns in one listing beyond a few relations.

    Keeps the fixtures of schema listing tests small: production reads up to a few hundred
    relations one by one.
    """

    exact_column_inspection_limit: ClassVar[int] = SCHEMA_LISTING_EXACT_COLUMN_LIMIT


@dataclass(frozen=True)
class SyntheticSnowflakeProject:
    """Synthetic multi-schema project files and the warehouse relations they inspect."""

    project_dir: Path
    relations: tuple[FakeRelation, ...]
    model_schemas: dict[str, str]
    incremental_model_names: tuple[str, ...]


_TABLE_MODEL_HEADER: str = "MODEL (description 'Test model.',\n  materialized table,\n);\n\n"
_INCREMENTAL_MODEL_HEADER: str = (
    "MODEL (description 'Test model.',\n"
    "  materialized incremental,\n"
    "  incremental_strategy delete_insert,\n"
    "  cursor ordered_at,\n"
    "  cursor_type timestamp,\n"
    "  cursor_grain day,\n"
    ");\n\n"
)
_ORDER_COLUMNS: tuple[FakeColumn, ...] = (
    FakeColumn(name="ORDER_ID", data_type="NUMBER", numeric_precision=38, numeric_scale=0),
    FakeColumn(name="ORDERED_AT", data_type="TIMESTAMP_NTZ"),
)


def write_synthetic_snowflake_project(
    *,
    project_dir: Path,
    database: str = "ANALYTICS",
    model_schemas: tuple[str, ...] = ("staging", "intermediate", "marts"),
    source_schema: str = "raw",
    models_per_schema: int = 40,
    incremental_every: int = 4,
    sources: int = 30,
    unmanaged_relations_per_schema: int = 600,
    state_tables: tuple[str, ...] = ("_sqlbuild_fingerprints", "_sqlbuild_migrations"),
) -> SyntheticSnowflakeProject:
    """Write a synthetic orders project and the matching warehouse relation set."""

    (project_dir / "sql" / "models").mkdir(parents=True, exist_ok=True)
    (project_dir / "sql" / "sources").mkdir(parents=True, exist_ok=True)
    (project_dir / "sqlbuild_project.toml").write_text(
        _project_toml(database=database, model_schemas=model_schemas), encoding="utf-8"
    )
    (project_dir / "sql" / "sources" / "raw.yml").write_text(
        _sources_yml(database=database, source_schema=source_schema, sources=sources),
        encoding="utf-8",
    )
    relations: list[FakeRelation] = [
        FakeRelation(
            database=database,
            schema=source_schema.upper(),
            name=f"ORDERS_{index}",
            columns=_ORDER_COLUMNS,
        )
        for index in range(sources)
    ]
    names_by_schema: dict[str, str] = {}
    incremental_flags: list[bool] = []
    schema_index: int
    schema: str
    for schema_index, schema in enumerate(model_schemas):
        (project_dir / "sql" / "models" / schema).mkdir(parents=True, exist_ok=True)
        model_index: int
        for model_index in range(models_per_schema):
            name: str = f"{schema}_orders_{model_index}"
            upstream: str = (
                f'__ref("{model_schemas[schema_index - 1]}_orders_{model_index}")',
                f'__source("raw_orders_{model_index % sources}")',
            )[schema_index == 0]
            incremental: bool = model_index % incremental_every == 0
            header: str = (_TABLE_MODEL_HEADER, _INCREMENTAL_MODEL_HEADER)[incremental]
            (project_dir / "sql" / "models" / schema / f"{name}.sql").write_text(
                f"{header}SELECT order_id, ordered_at FROM {upstream}\n", encoding="utf-8"
            )
            names_by_schema[name] = schema
            incremental_flags.append(incremental)
            relations.append(
                FakeRelation(
                    database=database,
                    schema=schema.upper(),
                    name=name.upper(),
                    is_transient=model_index % 2 == 1,
                    columns=_ORDER_COLUMNS,
                )
            )
    for schema in model_schemas:
        relations.extend(
            FakeRelation(
                database=database,
                schema=schema.upper(),
                name=state_table.upper(),
                is_transient=True,
            )
            for state_table in state_tables
        )
    for schema in (*model_schemas, source_schema):
        relations.extend(
            FakeRelation(
                database=database,
                schema=schema.upper(),
                name=f"ADHOC_SCRATCH_{index}",
                table_type=("BASE TABLE", "VIEW")[index % 5 == 0],
                is_transient=index % 3 == 0,
            )
            for index in range(unmanaged_relations_per_schema)
        )
    return SyntheticSnowflakeProject(
        project_dir=project_dir,
        relations=tuple(relations),
        model_schemas=names_by_schema,
        incremental_model_names=tuple(compress(names_by_schema, incremental_flags)),
    )


def build_inspection_catalog_relations() -> tuple[FakeRelation, ...]:
    """Return a metadata set with case variants, views, transient and external tables."""

    return (
        FakeRelation(database="ANALYTICS", schema="STAGING", name="ORDERS", columns=_ORDER_COLUMNS),
        FakeRelation(
            database="ANALYTICS",
            schema="STAGING",
            name="orders",
            columns=(FakeColumn(name="quoted_only", data_type="TEXT"),),
        ),
        FakeRelation(database="ANALYTICS", schema="STAGING", name="OrderLines"),
        FakeRelation(
            database="ANALYTICS",
            schema="STAGING",
            name="CUSTOMERS_V",
            table_type="VIEW",
            columns=(FakeColumn(name="CUSTOMER_ID", data_type="NUMBER", numeric_precision=38),),
        ),
        FakeRelation(
            database="ANALYTICS",
            schema="STAGING",
            name="TRANSIENT_EVENTS",
            is_transient=True,
            retention_time=0,
            columns=(FakeColumn(name="AMOUNT", data_type="FLOAT"),),
        ),
        FakeRelation(
            database="ANALYTICS",
            schema="STAGING",
            name="PARTNER_FEED",
            table_type="EXTERNAL TABLE",
        ),
        FakeRelation(
            database="ANALYTICS",
            schema="STAGING",
            name="DAILY_TOTALS",
            table_type="MATERIALIZED VIEW",
        ),
        *(
            FakeRelation(database="ANALYTICS", schema="STAGING", name=f"INVENTORY_{index}")
            for index in range(60)
        ),
        FakeRelation(database="ANALYTICS", schema="MARTS", name="REVENUE", columns=_ORDER_COLUMNS),
        FakeRelation(database="ANALYTICS", schema="MARTS", name="revenue_v", table_type="VIEW"),
        FakeRelation(database="ANALYTICS", schema="Mixed_Case", name="SUPPORT_TICKETS"),
        FakeRelation(database="ARCHIVE", schema="STAGING", name="ORDERS", columns=_ORDER_COLUMNS),
    )


def build_offline_snowflake(
    *, relations: tuple[FakeRelation, ...], statement_latency_seconds: float = 0.0
) -> tuple[OfflineSnowflakeAdapter, _SnowflakeConnection, RecordingSnowflakeWarehouse]:
    """Build an offline adapter, an open connection, and its recording warehouse."""

    warehouse: RecordingSnowflakeWarehouse = RecordingSnowflakeWarehouse(
        relations=relations, statement_latency_seconds=statement_latency_seconds
    )
    adapter: OfflineSnowflakeAdapter = OfflineSnowflakeAdapter(warehouse=warehouse)
    return adapter, adapter.connect({}), warehouse


def build_schema_listing_snowflake(
    *, relations: tuple[FakeRelation, ...]
) -> tuple[OfflineSnowflakeAdapter, _SnowflakeConnection, RecordingSnowflakeWarehouse]:
    """Build an offline adapter that lists a schema's columns beyond a few relations."""

    warehouse: RecordingSnowflakeWarehouse = RecordingSnowflakeWarehouse(relations=relations)
    adapter: OfflineSnowflakeAdapter = SchemaListingSnowflakeAdapter(warehouse=warehouse)
    return adapter, adapter.connect({}), warehouse


def sorted_relation_reprs(relations: tuple[RelationInfo, ...]) -> list[str]:
    """Return an order-independent form of listed relations without SHOW-absent LAST_ALTERED."""

    return sorted(repr(replace(relation, last_altered_at=None)) for relation in relations)


def _project_toml(*, database: str, model_schemas: tuple[str, ...]) -> str:
    path_defaults: str = "".join(
        f'[path_defaults.{schema}]\nschema = "{schema}"\n\n' for schema in model_schemas
    )
    return (
        'name = "orders_platform"\n'
        'adapter = "snowflake"\n\n'
        "[connection]\n"
        'account = "example"\n'
        'user = "example"\n'
        f'database = "{database.lower()}"\n'
        f'schema = "{model_schemas[0]}"\n\n'
        "[connection.session_parameters]\n"
        "QUOTED_IDENTIFIERS_IGNORE_CASE = true\n\n"
        "[defaults]\n"
        f'database = "{database.lower()}"\n'
        'materialized = "table"\n\n' + path_defaults
    )


def _sources_yml(*, database: str, source_schema: str, sources: int) -> str:
    lines: list[str] = ["sources:"]
    index: int
    for index in range(sources):
        lines.extend(
            (
                f"  - name: raw_orders_{index}",
                f"    description: Test source raw_orders_{index}.",
                f"    database: {database.lower()}",
                f"    schema: {source_schema}",
                f"    table: orders_{index}",
                "    columns:",
                "      - name: order_id",
                "        type: NUMBER(38, 0)",
                "      - name: ordered_at",
                "        type: TIMESTAMP_NTZ",
            )
        )
    return "\n".join(lines) + "\n"


def build_wide_schema_relations(
    *, relation_count: int, columns_per_relation: int, schema: str = "INVENTORY"
) -> tuple[FakeRelation, ...]:
    """Return one schema of identical relations sized to exercise SHOW result caps."""

    columns: tuple[FakeColumn, ...] = tuple(
        FakeColumn(name=f"ATTRIBUTE_{index}") for index in range(columns_per_relation)
    )
    return tuple(
        FakeRelation(
            database="ANALYTICS",
            schema=schema,
            name=f"STOCK_{index:05d}",
            columns=columns,
        )
        for index in range(relation_count)
    )


def build_wide_schemas_relations(
    *, schema_count: int, relation_count: int, columns_per_relation: int
) -> tuple[FakeRelation, ...]:
    """Return several wide schemas named WAREHOUSE_<index>."""

    return tuple(
        chain.from_iterable(
            build_wide_schema_relations(
                relation_count=relation_count,
                columns_per_relation=columns_per_relation,
                schema=f"WAREHOUSE_{index}",
            )
            for index in range(schema_count)
        )
    )


def show_relation_row(*, created_on: datetime, **fields: object) -> dict[str, object]:
    """Return one SHOW TABLES or SHOW VIEWS row for a STAGING.ORDERS relation."""

    return {"created_on": created_on, "name": "ORDERS", "schema_name": "STAGING", **fields}


def attempted_query_kinds(warehouse: RecordingSnowflakeWarehouse) -> tuple[str, ...]:
    """Classify every statement sent to the warehouse, including ones that failed."""

    return tuple(
        RecordedQuery(
            sql=sql, params=(), thread_id=0, started_at=0.0, finished_at=0.0, row_count=0
        ).kind
        for sql in warehouse.attempted_sql
    )


type ListedSchema = tuple[
    list[str], dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]]
]


def live_inspection_catalog(warehouse: RecordingSnowflakeWarehouse) -> InspectionCatalog:
    """A fresh catalog over its own adapter, reading `warehouse` live."""

    adapter: OfflineSnowflakeAdapter = OfflineSnowflakeAdapter(warehouse=warehouse)
    return InspectionCatalog(adapter=adapter, connection=adapter.connect({}))


def unavailable_relation_changes(**_: object) -> object:
    """A synced state check whose warehouse read fails."""

    raise RuntimeError("information schema unavailable")


def listed_schema(*, catalog: InspectionCatalog, database: str, schema: str) -> ListedSchema:
    """One schema's listing, in an order-independent form, and its relations' columns."""

    relations: tuple[RelationInfo, ...] = catalog.list_relations(
        database=database, schemas=(schema,)
    )
    return sorted_relation_reprs(relations), catalog.get_columns_for_relations(relations=relations)


def synced_copy(
    *, warehouse: RecordingSnowflakeWarehouse, database: str, schema: str, observed_at: datetime
) -> SyncedCatalog:
    """The copy a location synced: the live listing and columns, observed at `observed_at`."""

    catalog: InspectionCatalog = live_inspection_catalog(warehouse)
    relations: tuple[RelationInfo, ...] = catalog.list_relations(
        database=database, schemas=(schema,)
    )
    columns: dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]] = (
        catalog.get_columns_for_relations(relations=relations)
    )
    return SyncedCatalog(
        relations=tuple(
            SyncedRelation(
                database=database.upper(),
                schema=schema.upper(),
                name=relation.name,
                columns=columns.get(relation.identity, ()),
                observed_at=observed_at,
            )
            for relation in relations
        )
    )


def relations_after_sync(
    *,
    relations: tuple[FakeRelation, ...],
    changed: tuple[FakeRelation, ...],
    dropped: tuple[str, ...],
) -> tuple[FakeRelation, ...]:
    """`relations` with `changed` replacing or joining them and `dropped` names removed."""

    replaced: dict[str, FakeRelation] = {relation.name: relation for relation in relations}
    replaced.update({relation.name: relation for relation in changed})
    for name in dropped:
        replaced.pop(name)
    return tuple(replaced.values())
