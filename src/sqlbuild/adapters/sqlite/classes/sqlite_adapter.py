"""SQLite adapter implementation."""

from __future__ import annotations

import csv
import hashlib
import json
import logging
import re
import sqlite3
from collections.abc import Mapping
from contextvars import ContextVar, Token
from datetime import date, datetime
from decimal import Decimal
from pathlib import Path
from types import MappingProxyType
from typing import Any, ClassVar
from urllib.parse import quote

from sqlbuild._native import (
    MICROBATCH_END_SENTINEL,
    MICROBATCH_START_SENTINEL,
    SeedCsvSettings,
    SourceFreshnessRecord,
    SqlValue,
    render_insert_source_freshness_records_sql,
)
from sqlbuild.adapter.contract.classes.audit_results import AuditResultsMixin
from sqlbuild.adapter.contract.classes.base_adapter import (
    BaseAdapter,
    _encode_typed_json,
    _quote_sql_string,
    _render_ansi_typed_scalar,
    _render_explicit_source_relation_cast_subquery,
    _render_typed_value_list,
)
from sqlbuild.adapter.contract.classes.consumption import ConsumptionMixin
from sqlbuild.adapter.contract.classes.historical_check_snapshot_sql import (
    HistoricalCheckSnapshotSql,
)
from sqlbuild.adapter.contract.classes.historical_snapshot_sql import (
    render_is_distinct_from,
)
from sqlbuild.adapter.contract.classes.historical_timestamp_snapshot_sql import (
    HistoricalTimestampSnapshotSql,
)
from sqlbuild.adapter.contract.classes.microbatch import MicrobatchMixin
from sqlbuild.adapter.contract.classes.observations import ObservationsMixin
from sqlbuild.adapter.contract.classes.snapshot_sql import SnapshotSql
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.classes.unkeyed_diff import UnkeyedDiffMixin
from sqlbuild.adapter.contract.constants import (
    CANCELLABLE_DURATION_LIMIT_SECONDS,
    DIFF_LEFT_SIDE,
    DIFF_RIGHT_SIDE,
)
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.contract.main.normalize_seed_csv_value import normalize_seed_csv_value
from sqlbuild.adapter.contract.main.run_relation_read_probe import run_relation_read_probe
from sqlbuild.adapter.contract.main.same_view_definition import same_view_definition
from sqlbuild.adapter.contract.main.unsupported_relation_usage import unsupported_relation_usage
from sqlbuild.adapter.contract.models import (
    ColumnInfo,
    CursorValue,
    ExpressionInferenceProfile,
    FunctionDefinition,
    FunctionInfo,
    MigrationStagePlan,
    QueryResult,
    RelationGrant,
    RelationInfo,
    RelationReadProbe,
    RelationRowCountEstimate,
    RelationUsageReport,
    RowDiffColumnResult,
    RowDiffCoverage,
    RowDiffPreparedRelations,
    RowDiffResult,
    RowDiffSampleCell,
    RowDiffSampleRow,
    RowDiffSampling,
    RowDiffTolerance,
    RowDiffTolerances,
    SchemaDiffResult,
    SnapshotChangeTarget,
    SnapshotSqlDialect,
    TableFreshnessMetadata,
    TableFreshnessRequest,
)
from sqlbuild.adapter.contract.types import (
    BuiltinAdapter,
    CursorKind,
    FrameworkType,
    HistoricalSnapshotCloseStyle,
    HistoricalSnapshotInsertStyle,
    LoaderLogicalType,
    MigrationTransfer,
    PromotionStrategy,
    RelationReadStatus,
    SnapshotLatestVersionStyle,
    SnapshotUpdateStyle,
    StatementSizeLimit,
    TablePromotionMode,
)
from sqlbuild.adapter.relations.main.get_columns_for_relations import (
    get_columns_for_relations_bulk,
)
from sqlbuild.adapter.type_system.main.normalize_numeric_family import normalize_numeric_family
from sqlbuild.adapter.type_system.main.types_equal import types_equal
from sqlbuild.adapters.sqlite._helpers.catalog import (
    catalog_relations,
    logical_location,
    physical_name,
    quote_identifier,
    registered_schemas,
    registry_locations,
    relation_columns,
    unquote_relation,
    validate_schema_name,
    view_body,
)
from sqlbuild.adapters.sqlite._helpers.relations import (
    record_relation_changes,
    relation_changes,
    rename_view,
    split_rename_view,
)
from sqlbuild.adapters.sqlite._helpers.tables import (
    create_table_typed,
    decimal_scale,
    split_create_table_as,
    storage_type,
)
from sqlbuild.adapters.sqlite._helpers.timestamps import canonical_timestamp_text
from sqlbuild.adapters.sqlite.classes.logical_relations import LogicalRelations
from sqlbuild.adapters.sqlite.classes.sqlite_connection import SqliteConnection
from sqlbuild.adapters.sqlite.constants import (
    BOOLEAN_TYPE_MARKER,
    CONNECTION_KEYS,
    CURSOR_TEXT_FUNCTION_NAME,
    DEFAULT_BUSY_TIMEOUT_SECONDS,
    HASH_FUNCTION_NAME,
    INSERT_KEYWORD,
    INVALID_CONNECTION_CODE,
    MAIN_SCHEMA,
    MEMORY_DATABASE_PATH,
    MINIMUM_SQLITE_VERSION,
    MISSING_RELATION_ERROR_FRAGMENT,
    SCHEMA_REGISTRY_TABLE,
    SQLITE_TABLE_TYPE,
    TEMPORAL_TYPE_PATTERN,
    UNSUPPORTED_FEATURE_CODE,
    UNTYPED_COLUMN_TYPE,
    VERSION_TOO_OLD_CODE,
)
from sqlbuild.compiler.compile.types import FunctionLanguage
from sqlbuild.diagnostics.main.log_sql import log_sql
from sqlbuild.spec.contracts.constants import DEFAULT_SEED_CSV_SETTINGS

_ENFORCED_COLUMN_TYPES: ContextVar[Mapping[str, str]] = ContextVar(
    "sqlite_enforced_column_types", default=MappingProxyType({})
)
_INFERRED_COLUMN_TYPES: ContextVar[Mapping[str, str]] = ContextVar(
    "sqlite_inferred_column_types", default=MappingProxyType({})
)
_TEMPORAL_TYPE: re.Pattern[str] = re.compile(TEMPORAL_TYPE_PATTERN, re.IGNORECASE)
_TRUE_SEED_VALUES: frozenset[str] = frozenset({"true", "t", "yes", "y", "1"})
_FALSE_SEED_VALUES: frozenset[str] = frozenset({"false", "f", "no", "n", "0"})


class SqliteAdapter(
    MicrobatchMixin,
    ConsumptionMixin,
    ObservationsMixin,
    AuditResultsMixin,
    UnkeyedDiffMixin,
    BaseAdapter,
):
    """SQLite adapter: one database file, SQLBuild schemas stored as `schema__name` tables."""

    adapter_name: ClassVar[str] = BuiltinAdapter.SQLITE.value
    execution_duration_limit_seconds: ClassVar[int | None] = CANCELLABLE_DURATION_LIMIT_SECONDS
    sql_analysis_dialect_name: ClassVar[str | None] = "sqlite"
    untyped_column_type: ClassVar[str | None] = UNTYPED_COLUMN_TYPE
    max_identifier_length: ClassVar[int] = 1024
    relation_grants_supported: ClassVar[bool] = False
    allows_implicit_managed_write_schema: ClassVar[bool] = True
    _snapshot_sql_dialect: ClassVar[SnapshotSqlDialect] = SnapshotSqlDialect(
        timestamp_type="TEXT",
        distinct_condition=render_is_distinct_from,
        update_style=SnapshotUpdateStyle.UPDATE_FROM,
        latest_version=SnapshotLatestVersionStyle.DERIVED_TABLE,
        historical_close=HistoricalSnapshotCloseStyle.CORRELATED,
        historical_insert=HistoricalSnapshotInsertStyle.WITH_INSERT,
    )

    def __init__(self) -> None:
        self._logical_relations: LogicalRelations = LogicalRelations()

    def connect(self, config: dict[str, Any]) -> SqliteConnection:
        if sqlite3.sqlite_version_info < MINIMUM_SQLITE_VERSION:
            minimum: str = ".".join(str(part) for part in MINIMUM_SQLITE_VERSION)
            raise AdapterUserError(
                message=(
                    f"SQLite adapter requires SQLite {minimum} or newer; this Python links "
                    f"SQLite {sqlite3.sqlite_version}"
                ),
                code=VERSION_TOO_OLD_CODE,
                help="Use a Python build that bundles a newer SQLite, such as one installed by uv.",
            )
        unknown: tuple[str, ...] = tuple(sorted(set(config) - CONNECTION_KEYS))
        if unknown:
            raise AdapterUserError(
                message=f"SQLite connection does not accept: {', '.join(unknown)}",
                code=INVALID_CONNECTION_CODE,
                help="SQLite connections accept 'path', 'busy_timeout' and 'read_only'.",
            )
        path: str = str(config.get("path", MEMORY_DATABASE_PATH))
        busy_timeout: float = float(config.get("busy_timeout", DEFAULT_BUSY_TIMEOUT_SECONDS))
        read_only: bool = bool(config.get("read_only", False))
        if path != MEMORY_DATABASE_PATH and not read_only:
            Path(path).parent.mkdir(parents=True, exist_ok=True)
        target: str = (
            f"file:{quote(Path(path).as_posix(), safe='/:')}?mode=ro" if read_only else path
        )
        raw_connection: sqlite3.Connection = sqlite3.connect(
            target,
            timeout=busy_timeout,
            isolation_level=None,
            check_same_thread=False,
            uri=read_only,
        )
        raw_connection.create_function(HASH_FUNCTION_NAME, 1, _sqb_hash, deterministic=True)
        raw_connection.create_function(
            CURSOR_TEXT_FUNCTION_NAME, 1, _cursor_text, deterministic=True
        )
        connection: SqliteConnection = SqliteConnection(
            raw_connection=raw_connection, path=path, busy_timeout_seconds=busy_timeout
        )
        if path != MEMORY_DATABASE_PATH and not read_only:
            connection.execute_as_writer("PRAGMA journal_mode=WAL")
        connection.execute(f"PRAGMA busy_timeout={int(busy_timeout * 1000)}")
        connection.execute("PRAGMA foreign_keys=ON")
        connection.execute("PRAGMA legacy_alter_table=ON")
        return connection

    def _execute(self, *, connection: SqliteConnection, sql: str) -> Any:
        log_sql(logger=logging.getLogger("sqlbuild.adapter.sqlite"), sql=sql)
        removed: tuple[str, ...]
        added: tuple[str, ...]
        removed, added = relation_changes(sql)
        if not removed and not added:
            return self._execute_statement(connection=connection, sql=sql)
        owns_transaction: bool = not connection.in_transaction()
        if owns_transaction:
            connection.execute("BEGIN IMMEDIATE")
        try:
            cursor: Any = self._execute_statement(connection=connection, sql=sql)
            record_relation_changes(
                connection=connection,
                removed=removed,
                added=added,
                locate=self._logical_relations.lookup,
            )
        except BaseException:
            if owns_transaction and connection.in_transaction():
                connection.execute("ROLLBACK")
            raise
        if owns_transaction:
            connection.execute("COMMIT")
        return cursor

    def _execute_statement(self, *, connection: SqliteConnection, sql: str) -> Any:
        view_rename: tuple[str, str] | None = split_rename_view(sql)
        if view_rename is not None:
            return rename_view(
                connection=connection, origin=view_rename[0], destination=view_rename[1]
            )
        create_table_as: tuple[str, str] | None = split_create_table_as(sql)
        if create_table_as is None:
            return connection.execute(sql)
        return create_table_typed(
            connection=connection,
            destination=create_table_as[0],
            sql=create_table_as[1],
            column_types=_ENFORCED_COLUMN_TYPES.get(),
            inferred_types=_INFERRED_COLUMN_TYPES.get(),
        )

    def close(self, connection: SqliteConnection) -> None:
        connection.close()

    def begin(self, connection: Any) -> None:
        """Take the write lock up front so a read lock is never upgraded mid-transaction."""

        self.execute(connection=connection, sql="BEGIN IMMEDIATE")

    def rollback(self, connection: Any) -> None:
        """Roll back only an open transaction; SQLite ends it itself on interrupts."""

        if connection.in_transaction():
            self.execute(connection=connection, sql="ROLLBACK")

    def interrupt_connection(self, connection: Any) -> bool:
        connection.interrupt()
        return True

    def lists_relations_authoritatively(self, *, database: str | None, schema: str) -> bool:
        del database, schema
        return True

    def supports_zero_copy_clone(self) -> bool:
        return False

    def supports_durable_clone(self) -> bool:
        return False

    def supports_relation_age_metadata(self) -> bool:
        """Return False because SQLite records no relation timestamps."""

        return False

    def supports_table_freshness_metadata(self) -> bool:
        return False

    def supports_concurrent_microbatch_dml(self) -> bool:
        """Return False: a SQLite file has one writer, so concurrent batches only serialize."""

        return False

    def probe_relation_read(self, *, connection: Any, relation: str) -> RelationReadProbe:
        return run_relation_read_probe(
            execute=self.execute,
            connection=connection,
            relation=relation,
            classify_not_found=self._classify_relation_not_found,
        )

    @staticmethod
    def _classify_relation_not_found(error: BaseException) -> RelationReadStatus | None:
        if isinstance(error, sqlite3.OperationalError) and str(error).startswith(
            MISSING_RELATION_ERROR_FRAGMENT
        ):
            return RelationReadStatus.MISSING
        return None

    def get_table_freshness_metadata(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> TableFreshnessMetadata:
        raise AdapterUserError(
            message=f"adapter '{self.adapter_name}' does not support table freshness metadata"
        )

    def relation_usage(
        self,
        *,
        connection: Any,
        relations: tuple[RelationInfo, ...],
        window_seconds: int,
    ) -> RelationUsageReport:
        del connection, relations, window_seconds
        return unsupported_relation_usage()

    def get_tables_freshness_metadata(
        self,
        *,
        connection: Any,
        requests: tuple[TableFreshnessRequest, ...],
    ) -> dict[TableFreshnessRequest, TableFreshnessMetadata]:
        raise AdapterUserError(
            message=f"adapter '{self.adapter_name}' does not support table freshness metadata"
        )

    def supports_python_functions(self) -> bool:
        return False

    def persists_python_functions(self) -> bool:
        return False

    def python_functions_inherit_default_namespace(self) -> bool:
        return False

    def supports_unqualified_function_fingerprints(self) -> bool:
        return True

    def supports_table_functions(self) -> bool:
        return False

    def max_statement_size(self) -> StatementSizeLimit | None:
        """SQLITE_MAX_SQL_LENGTH defaults to 1,000,000,000 bytes: https://sqlite.org/limits.html."""

        return (1_000_000_000, "bytes")

    def maximum_identifier_length(self) -> int:
        return self.max_identifier_length

    def query_column_names(self, *, connection: Any, sql: str) -> tuple[str, ...]:
        cursor: Any = self.execute(
            connection=connection, sql=f"SELECT * FROM ({sql}) AS __describe_source LIMIT 0"
        )
        description: Any | None = getattr(cursor, "description", None)
        if description is None:
            return ()
        return tuple(str(column[0]) for column in description)

    def render_max_cursor_at_or_before(
        self,
        *,
        relation: str,
        cursor_column: str,
        maximum_allowed: str,
        cursor_type: str | None,
        is_date: bool,
    ) -> str:
        quoted_cursor: str = self.render_cursor_operand(
            expression=self.render_identifier(cursor_column), cursor_type=cursor_type
        )
        maximum_literal: str = (
            _quote_sql_string(maximum_allowed)
            if is_date
            else self.render_cursor_bound_literal(value=maximum_allowed, cursor_type=cursor_type)
        )
        return (
            f"SELECT MAX({quoted_cursor}) FROM {relation} "
            f"WHERE {quoted_cursor} <= {maximum_literal}"
        )

    def build_cursor_filter(
        self,
        *,
        cursor_column: str | None,
        start_cursor: CursorValue | None,
        end_cursor: CursorValue | None,
    ) -> str:
        if cursor_column is None or start_cursor is None:
            return ""
        operand: str = self.render_cursor_operand(
            expression=cursor_column, cursor_type=start_cursor.kind
        )
        clauses: list[str] = [f"{operand} >= {self._cursor_value_literal(start_cursor)}"]
        if end_cursor is not None:
            clauses.append(f"{operand} < {self._cursor_value_literal(end_cursor)}")
        return " AND ".join(clauses)

    def render_cursor_operand(self, *, expression: str, cursor_type: str | None) -> str:
        """Compare timestamp cursors in canonical UTC text, whatever form the column stores."""

        if str(cursor_type).lower() == CursorKind.INTEGER:
            return expression
        return f"{CURSOR_TEXT_FUNCTION_NAME}({expression})"

    def schema_exists(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str,
    ) -> bool:
        del database
        if schema.lower() == MAIN_SCHEMA:
            return True
        return schema.lower() in registered_schemas(connection)

    def query(self, *, connection: Any, sql: str, limit: int | None) -> QueryResult:
        cursor: Any = self.execute(connection=connection, sql=sql)
        description: Any | None = getattr(cursor, "description", None)
        if description is None:
            return QueryResult()
        columns: tuple[str, ...] = tuple(str(column[0]) for column in description)
        rows: tuple[tuple[object, ...], ...]
        truncated: bool = False
        if limit is None:
            rows = tuple(tuple(row) for row in cursor.fetchall())
        else:
            fetched_rows: list[tuple[object, ...]] = [
                tuple(row) for row in cursor.fetchmany(limit + 1)
            ]
            truncated = len(fetched_rows) > limit
            rows = tuple(fetched_rows[:limit])
        return QueryResult(columns=columns, rows=rows, truncated=truncated)

    def relation_exists(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> bool:
        del database
        table_name: str = physical_name(schema=schema, name=name)
        row: tuple[Any, ...] | None = connection.execute_with(
            sql="SELECT name FROM sqlite_schema "
            "WHERE type IN ('table', 'view') AND lower(name) = lower(?)",
            parameters=(table_name,),
        ).fetchone()
        if row is None:
            return False
        located: tuple[str, str] = logical_location(
            table_name=str(row[0]), locations=registry_locations(connection)
        )
        return (located[0].lower(), located[1].lower()) == (
            (schema or MAIN_SCHEMA).lower(),
            name.lower(),
        )

    def list_relations(
        self,
        *,
        connection: Any,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[RelationInfo, ...]:
        del database
        schema_filter: frozenset[str] | None = (
            None if schemas is None else frozenset(schema.lower() for schema in schemas)
        )
        name_filter: frozenset[str] | None = (
            None if names is None else frozenset(name.lower() for name in names)
        )
        return tuple(
            RelationInfo(database=None, schema=schema, name=name, relation_type=relation_type)
            for schema, name, _, relation_type in catalog_relations(connection)
            if (schema_filter is None or schema.lower() in schema_filter)
            and (name_filter is None or name.lower() in name_filter)
        )

    def with_relation_age_metadata(
        self,
        *,
        connection: Any,
        relations: tuple[RelationInfo, ...],
    ) -> tuple[RelationInfo, ...]:
        return relations

    def list_functions(
        self,
        *,
        connection: Any,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[FunctionInfo, ...]:
        """Return no functions: SQLite has no persistent function catalog."""

        del connection, database, schemas, names
        return ()

    def get_columns(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> tuple[ColumnInfo, ...]:
        del database
        return relation_columns(
            connection=connection, table_name=physical_name(schema=schema, name=name)
        )

    def get_all_columns(
        self,
        *,
        connection: Any,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> dict[str, tuple[ColumnInfo, ...]]:
        del database
        schema_filter: frozenset[str] | None = (
            None if schemas is None else frozenset(schema.lower() for schema in schemas)
        )
        name_filter: frozenset[str] | None = (
            None if names is None else frozenset(name.lower() for name in names)
        )
        return {
            name: relation_columns(connection=connection, table_name=table_name)
            for schema, name, table_name, _ in catalog_relations(connection)
            if (schema_filter is None or schema.lower() in schema_filter)
            and (name_filter is None or name.lower() in name_filter)
        }

    def get_columns_for_relations(
        self,
        *,
        connection: Any,
        relations: tuple[RelationInfo, ...],
    ) -> dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]]:
        return get_columns_for_relations_bulk(
            adapter=self, connection=connection, relations=relations
        )

    def render_create_schema(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        """Record the schema in the registry that decides how `schema__name` tables split."""

        del database
        if schema.lower() == MAIN_SCHEMA:
            return ()
        validate_schema_name(schema)
        return (
            f"CREATE TABLE IF NOT EXISTS {SCHEMA_REGISTRY_TABLE} "
            "(schema_name TEXT PRIMARY KEY, created_at TEXT NOT NULL)",
            f"INSERT OR IGNORE INTO {SCHEMA_REGISTRY_TABLE} (schema_name, created_at) "
            f"VALUES ({_quote_sql_string(schema)}, {self.render_current_timestamp()})",
        )

    def render_drop_schema(self, *, database: str | None, schema: str) -> tuple[str, ...]:
        del database
        raise AdapterUserError(
            f"the {self.adapter_name} adapter cannot drop schema '{schema}' with its contents",
            code="A714",
            help="sqb review needs a warehouse that can drop a schema it created",
        )

    def ensure_schema(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str | None,
        statement_recorder: StatementRecorder,
    ) -> None:
        if schema is None or self.schema_exists(
            connection=connection, database=database, schema=schema
        ):
            return
        statements: tuple[str, ...] = self.render_create_schema(database=database, schema=schema)
        statement_recorder.record_many(statements)
        with self.transaction(connection):
            stmt: str
            for stmt in statements:
                self.execute(connection=connection, sql=stmt)

    def render_create_view_as(self, *, destination: str, sql: str) -> tuple[str, ...]:
        return (f"DROP VIEW IF EXISTS {destination}", f"CREATE VIEW {destination} AS {sql}")

    def render_table_function_call(self, *, target: str, call_suffix_sql: str) -> str:
        return f"{target}{call_suffix_sql}"

    def render_udf_call(self, *, target: str, call_suffix_sql: str) -> str:
        return f"{target}{call_suffix_sql}"

    def render_create_function(
        self,
        *,
        destination: str,
        arguments: tuple[Any, ...],
        returns: str,
        body_sql: str,
        return_columns: tuple[Any, ...] = (),
        language: FunctionLanguage = FunctionLanguage.SQL,
        runtime_version: str | None = None,
        entry_point: str | None = None,
        packages: tuple[str, ...] = (),
    ) -> tuple[str, ...]:
        del arguments, returns, body_sql, return_columns, runtime_version, entry_point, packages
        raise AdapterUserError(
            message=(
                f"SQLite cannot create function '{destination}': SQLite has no CREATE FUNCTION, "
                f"so {language.value.upper()} functions and table functions are not supported"
            ),
            code=UNSUPPORTED_FEATURE_CODE,
            help="Inline the expression in the model, or use a macro.",
        )

    def render_append(
        self, *, destination: str, sql: str, columns: tuple[str, ...] | None = None
    ) -> tuple[str, ...]:
        column_sql: str = ""
        if columns is not None:
            column_sql = (
                " (" + ", ".join(self.render_identifier(column) for column in columns) + ")"
            )
        return (f"INSERT INTO {destination}{column_sql} {sql}",)

    def render_delete_insert(
        self,
        *,
        destination: str,
        sql: str,
        unique_key: tuple[str, ...],
        columns: tuple[str, ...] | None = None,
    ) -> tuple[str, ...]:
        staged: str = self._with_replaced_relation_name(
            relation=destination, name=f"{self._relation_name(destination)}__delete_insert"
        )
        key_list: str = ", ".join(self.render_identifier(key) for key in unique_key)
        create_staged: tuple[str, ...] = self.render_create_table_as(destination=staged, sql=sql)
        delete_sql: str = (
            f"DELETE FROM {destination} WHERE ({key_list}) IN (SELECT {key_list} FROM {staged})"
        )
        insert_stmts: tuple[str, ...] = self.render_append(
            destination=destination,
            sql=f"SELECT * FROM {staged}",
            columns=columns,
        )
        drop_staged: tuple[str, ...] = self.render_drop(destination=staged, if_exists=True)
        return (*create_staged, delete_sql, *insert_stmts, *drop_staged)

    def render_delete_insert_cursor(
        self,
        *,
        destination: str,
        sql: str,
        cursor_column: str,
        cursor_start: str,
        cursor_end: str,
        columns: tuple[str, ...] | None = None,
        cursor_type: str | None = None,
    ) -> tuple[str, ...]:
        quoted_cursor_column: str = self.render_cursor_operand(
            expression=self.render_identifier(cursor_column), cursor_type=cursor_type
        )
        start_literal: str = self.render_cursor_bound_literal(
            value=cursor_start, cursor_type=cursor_type
        )
        end_literal: str = self.render_cursor_bound_literal(
            value=cursor_end, cursor_type=cursor_type
        )
        delete_sql: str = (
            f"DELETE FROM {destination} WHERE {quoted_cursor_column} >= {start_literal} "
            f"AND {quoted_cursor_column} < {end_literal}"
        )
        insert_stmts: tuple[str, ...] = self.render_append(
            destination=destination, sql=sql, columns=columns
        )
        return (delete_sql, *insert_stmts)

    def render_drop(self, *, destination: str, if_exists: bool = True) -> tuple[str, ...]:
        exists_clause: str = " IF EXISTS" if if_exists else ""
        return (f"DROP TABLE{exists_clause} {destination}",)

    def render_drop_view(self, *, destination: str, if_exists: bool = True) -> tuple[str, ...]:
        exists_clause: str = " IF EXISTS" if if_exists else ""
        return (f"DROP VIEW{exists_clause} {destination}",)

    def render_swap(self, *, left: str, right: str) -> tuple[str, ...]:
        staging: str = self._with_replaced_relation_name(
            relation=left, name=f"{self._relation_name(left)}__swap_staging"
        )
        return (
            *self.render_rename(origin=left, destination=staging),
            *self.render_rename(origin=right, destination=left),
            *self.render_rename(origin=staging, destination=right),
        )

    def render_clone(
        self,
        *,
        origin: str,
        destination: str,
        hard_copy: bool = False,
        origin_is_transient: bool = False,
    ) -> tuple[str, ...]:
        del hard_copy, origin_is_transient
        return self.render_create_table_as(destination=destination, sql=f"SELECT * FROM {origin}")

    def render_durable_clone(
        self, *, origin: str, destination: str, origin_is_transient: bool = False
    ) -> tuple[str, ...]:
        del origin_is_transient
        return self.render_create_table_as(destination=destination, sql=f"SELECT * FROM {origin}")

    def render_migration_stage(
        self,
        *,
        origin: str,
        stage: str,
        origin_is_transient: bool = False,
        stage_is_transient: bool | None = None,
    ) -> MigrationStagePlan:
        del origin_is_transient, stage_is_transient
        return MigrationStagePlan(
            transfer=MigrationTransfer.COPY,
            statements=(f"CREATE TABLE {stage} AS SELECT * FROM {origin}",),
        )

    def capture_dependent_view_rebinds(
        self, *, connection: Any, database: str | None, schema: str, name: str
    ) -> tuple[str, ...]:
        """Return nothing: SQLite views bind by name, so a renamed table needs no rebind."""

        del connection, database, schema, name
        return ()

    def views_bind_to_relation_identity(self) -> bool:
        return False

    def list_dependent_view_names(
        self, *, connection: Any, database: str | None, schema: str, name: str
    ) -> tuple[str, ...]:
        del database
        target: str = physical_name(schema=schema, name=name)
        reference: re.Pattern[str] = re.compile(
            r"(?<![\w$])[\"`\[]?" + re.escape(target) + r"[\"`\]]?(?![\w$])", re.IGNORECASE
        )
        rows: list[tuple[Any, ...]] = connection.execute(
            "SELECT name, sql FROM sqlite_schema WHERE type = 'view' ORDER BY name"
        ).fetchall()
        dependents: set[str] = {
            str(row[0])
            for row in rows
            if str(row[0]).lower() != target.lower() and reference.search(view_body(str(row[1])))
        }
        return tuple(
            f"{relation_schema}.{relation_name}"
            for relation_schema, relation_name, table_name, _ in catalog_relations(connection)
            if table_name in dependents
        )

    def read_relation_grants(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str,
        name: str,
        relation_type: str,
    ) -> tuple[RelationGrant, ...]:
        """Return no grants: SQLite has no privilege system."""

        del connection, database, schema, name, relation_type
        return ()

    def render_relation_grants(
        self, *, grants: tuple[RelationGrant, ...], destination: str, columns: tuple[str, ...]
    ) -> tuple[str, ...]:
        del grants, destination, columns
        return ()

    def render_replace_view_keeping_grants(
        self, *, destination: str, sql: str
    ) -> tuple[str, ...] | None:
        return self.render_create_view_as(destination=destination, sql=sql)

    def read_view_definition(
        self, *, connection: Any, database: str | None, schema: str, name: str
    ) -> str | None:
        del database
        row: tuple[Any, ...] | None = connection.execute_with(
            sql="SELECT sql FROM sqlite_schema WHERE type = 'view' AND lower(name) = lower(?)",
            parameters=(physical_name(schema=schema, name=name),),
        ).fetchone()
        return None if row is None else view_body(str(row[0]))

    def view_definition_matches(
        self, *, connection: Any, database: str | None, schema: str, name: str, sql: str
    ) -> bool:
        definition: str | None = self.read_view_definition(
            connection=connection, database=database, schema=schema, name=name
        )
        if definition is None:
            return False
        return same_view_definition(definition=definition, sql=sql)

    def render_relation_revokes(
        self, *, grants: tuple[RelationGrant, ...], destination: str
    ) -> tuple[str, ...]:
        del grants, destination
        return ()

    def rename_view(
        self,
        *,
        connection: Any,
        origin: str,
        destination: str,
        statement_recorder: StatementRecorder,
    ) -> None:
        """Rename a view; SQLite recreates it from its stored definition in one transaction."""

        statements: tuple[str, ...] = self.render_rename_view(
            origin=origin, destination=destination
        )
        statement_recorder.record_many(statements)
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

    def supports_transactional_ddl(self) -> bool:
        return True

    def render_seed_select_before_cursor(
        self,
        *,
        origin: str,
        cursor_column: str,
        cursor_end_exclusive: str,
        cursor_type: str | None,
    ) -> str:
        operand: str = self.render_cursor_operand(
            expression=self.render_identifier(cursor_column), cursor_type=cursor_type
        )
        end_literal: str = self.render_cursor_bound_literal(
            value=cursor_end_exclusive, cursor_type=cursor_type
        )
        return f"SELECT * FROM {origin} WHERE {operand} < {end_literal}"

    def relation_names_match(self, *, left: str, right: str) -> bool:
        return self._relation_names_match_impl(left=left, right=right)

    def render_replace_table_from_relation(
        self, *, destination: str, origin: str
    ) -> tuple[str, ...]:
        return self.render_create_table_as(destination=destination, sql=f"SELECT * FROM {origin}")

    def render_add_columns(
        self, *, destination: str, columns: tuple[ColumnInfo, ...]
    ) -> tuple[str, ...]:
        return tuple(
            f"ALTER TABLE {destination} ADD COLUMN {self.render_identifier(col.name)} "
            f"{storage_type(col.type)}"
            for col in columns
        )

    def render_drop_columns(
        self, *, destination: str, column_names: tuple[str, ...]
    ) -> tuple[str, ...]:
        return tuple(
            f"ALTER TABLE {destination} DROP COLUMN {self.render_identifier(col_name)}"
            for col_name in column_names
        )

    def render_rename_column(
        self, *, destination: str, old_name: str, new_name: str
    ) -> tuple[str, ...]:
        from sqlbuild.adapter.contract.main.render_rename_column_sql import (
            render_alter_rename_column_sql,
        )

        return (
            render_alter_rename_column_sql(
                destination=destination,
                old_identifier=self.render_exact_identifier(old_name),
                new_identifier=self.render_identifier(new_name),
            ),
        )

    def render_alter_column_types(
        self, *, destination: str, columns: tuple[ColumnInfo, ...]
    ) -> tuple[str, ...]:
        del columns
        raise AdapterUserError(
            message=(
                f"SQLite cannot alter column types of '{destination}' in place; "
                "alter_column_types rebuilds the table instead"
            ),
            code=UNSUPPORTED_FEATURE_CODE,
        )

    def render_merge(
        self,
        *,
        destination: str,
        sql: str,
        unique_key: tuple[str, ...],
        source_columns: tuple[str, ...] = (),
        exclude_columns: tuple[str, ...] = (),
    ) -> tuple[str, ...]:
        immutable_columns: frozenset[str] = frozenset(
            column.lower() for column in (*unique_key, *exclude_columns)
        )
        key_match_sql: str = " AND ".join(
            f"__target.{self.render_identifier(k)} = __source.{self.render_identifier(k)}"
            for k in unique_key
        )
        update_set: str = ", ".join(
            f"{self.render_identifier(col)} = __source.{self.render_identifier(col)}"
            for col in source_columns
            if col.lower() not in immutable_columns
        )
        col_list: str = ", ".join(self.render_identifier(col) for col in source_columns)
        statements: list[str] = []
        if update_set:
            statements.append(
                f"UPDATE {destination} AS __target SET {update_set} "
                f"FROM ({sql}) AS __source WHERE {key_match_sql}"
            )
        statements.append(
            f"INSERT INTO {destination} ({col_list}) "
            f"SELECT {col_list} FROM ({sql}) AS __source "
            f"WHERE NOT EXISTS (SELECT 1 FROM {destination} AS __target WHERE {key_match_sql})"
        )
        return tuple(statements)

    def render_current_timestamp(self) -> str:
        """Render UTC now as ISO-8601 text with milliseconds, the adapter's timestamp form."""

        return "strftime('%Y-%m-%d %H:%M:%f', 'now')"

    def render_current_utc_timestamp(self) -> str:
        return "strftime('%Y-%m-%dT%H:%M:%f', 'now')"

    def create_table_as(
        self,
        *,
        connection: Any,
        destination: str,
        sql: str,
        config: dict[str, Any] | None = None,
        statement_recorder: StatementRecorder,
    ) -> None:
        statements: tuple[str, ...] = self.render_create_table_as(destination=destination, sql=sql)
        statement_recorder.record_many(statements)
        enforced: Token[Mapping[str, str]] = _ENFORCED_COLUMN_TYPES.set(
            _lowered((config or {}).get("column_types"))
        )
        inferred: Token[Mapping[str, str]] = _INFERRED_COLUMN_TYPES.set(
            _lowered((config or {}).get("inferred_column_types"))
        )
        try:
            with self.transaction(connection):
                stmt: str
                for stmt in statements:
                    self.execute(connection=connection, sql=stmt)
        finally:
            _INFERRED_COLUMN_TYPES.reset(inferred)
            _ENFORCED_COLUMN_TYPES.reset(enforced)

    def create_view_as(
        self,
        *,
        connection: Any,
        destination: str,
        sql: str,
        statement_recorder: StatementRecorder,
    ) -> None:
        statements: tuple[str, ...] = self.render_create_view_as(destination=destination, sql=sql)
        statement_recorder.record_many(statements)
        with self.transaction(connection):
            stmt: str
            for stmt in statements:
                self.execute(connection=connection, sql=stmt)

    def create_function(
        self,
        *,
        connection: Any,
        definition: FunctionDefinition,
        statement_recorder: StatementRecorder,
    ) -> None:
        del connection, statement_recorder
        self.render_create_function(
            destination=definition.destination,
            arguments=definition.arguments,
            returns=definition.returns,
            body_sql=definition.body_sql,
            return_columns=definition.return_columns,
            language=definition.language,
        )

    def drop(
        self,
        *,
        connection: Any,
        destination: str,
        if_exists: bool = True,
        statement_recorder: StatementRecorder,
    ) -> None:
        statements: tuple[str, ...] = self.render_drop(destination=destination, if_exists=if_exists)
        statement_recorder.record_many(statements)
        stmt: str
        for stmt in statements:
            self.execute(connection=connection, sql=stmt)

    def drop_view(
        self,
        *,
        connection: Any,
        destination: str,
        if_exists: bool = True,
        statement_recorder: StatementRecorder,
    ) -> None:
        statements: tuple[str, ...] = self.render_drop_view(
            destination=destination, if_exists=if_exists
        )
        statement_recorder.record_many(statements)
        stmt: str
        for stmt in statements:
            self.execute(connection=connection, sql=stmt)

    def rename(
        self,
        *,
        connection: Any,
        origin: str,
        destination: str,
        statement_recorder: StatementRecorder,
    ) -> None:
        statements: tuple[str, ...] = self.render_rename(origin=origin, destination=destination)
        statement_recorder.record_many(statements)
        stmt: str
        for stmt in statements:
            self.execute(connection=connection, sql=stmt)

    def swap(
        self,
        *,
        connection: Any,
        left: str,
        right: str,
        statement_recorder: StatementRecorder,
    ) -> None:
        statements: tuple[str, ...] = self.render_swap(left=left, right=right)
        statement_recorder.record_many(statements)
        with self.transaction(connection):
            stmt: str
            for stmt in statements:
                self.execute(connection=connection, sql=stmt)

    def clone(
        self,
        *,
        connection: Any,
        origin: str,
        destination: str,
        hard_copy: bool = False,
        origin_is_transient: bool = False,
        statement_recorder: StatementRecorder,
    ) -> None:
        statements: tuple[str, ...] = self.render_clone(
            origin=origin,
            destination=destination,
            hard_copy=hard_copy,
            origin_is_transient=origin_is_transient,
        )
        statement_recorder.record_many(statements)
        with self.transaction(connection):
            stmt: str
            for stmt in statements:
                self.execute(connection=connection, sql=stmt)

    def durable_clone(
        self,
        *,
        connection: Any,
        origin: str,
        destination: str,
        origin_is_transient: bool = False,
        statement_recorder: StatementRecorder,
    ) -> None:
        statements: tuple[str, ...] = self.render_durable_clone(
            origin=origin, destination=destination, origin_is_transient=origin_is_transient
        )
        statement_recorder.record_many(statements)
        with self.transaction(connection):
            stmt: str
            for stmt in statements:
                self.execute(connection=connection, sql=stmt)

    def replace_table_from_relation(
        self,
        *,
        connection: Any,
        destination: str,
        origin: str,
        statement_recorder: StatementRecorder,
    ) -> None:
        statements: tuple[str, ...] = self.render_replace_table_from_relation(
            destination=destination,
            origin=origin,
        )
        statement_recorder.record_many(statements)
        with self.transaction(connection):
            stmt: str
            for stmt in statements:
                self.execute(connection=connection, sql=stmt)

    def move_or_copy_relation(
        self,
        *,
        connection: Any,
        origin: str,
        destination: str,
        remove_origin: bool,
        allow_copy_fallback: bool,
        statement_recorder: StatementRecorder,
    ) -> None:
        """Rename within the file; schemas are name prefixes, so every move is a rename."""

        del allow_copy_fallback
        if remove_origin:
            self.rename(
                connection=connection,
                origin=origin,
                destination=destination,
                statement_recorder=statement_recorder,
            )
            return
        self.replace_table_from_relation(
            connection=connection,
            destination=destination,
            origin=origin,
            statement_recorder=statement_recorder,
        )

    def append(
        self,
        *,
        connection: Any,
        destination: str,
        sql: str,
        columns: tuple[str, ...] | None = None,
        statement_recorder: StatementRecorder,
    ) -> int | None:
        statements: tuple[str, ...] = self.render_append(
            destination=destination, sql=sql, columns=columns
        )
        statement_recorder.record_many(statements)
        affected: int | None = None
        stmt: str
        for stmt in statements:
            cursor: Any = self.execute(connection=connection, sql=stmt)
            affected = self.affected_row_count(cursor=cursor)
        return affected

    def delete_insert(
        self,
        *,
        connection: Any,
        destination: str,
        sql: str,
        unique_key: str | tuple[str, ...],
        columns: tuple[str, ...] | None = None,
        statement_recorder: StatementRecorder,
    ) -> int | None:
        keys: tuple[str, ...] = (unique_key,) if isinstance(unique_key, str) else unique_key
        statements: tuple[str, ...] = self.render_delete_insert(
            destination=destination, sql=sql, unique_key=keys, columns=columns
        )
        statement_recorder.record_many(statements)
        affected: int | None = None
        with self.transaction(connection):
            stmt: str
            for stmt in statements:
                cursor: Any = self.execute(connection=connection, sql=stmt)
                if stmt.startswith(INSERT_KEYWORD):
                    affected = self.affected_row_count(cursor=cursor)
        return affected

    def delete_insert_cursor(
        self,
        *,
        connection: Any,
        destination: str,
        sql: str,
        cursor_column: str,
        cursor_start: str,
        cursor_end: str,
        columns: tuple[str, ...] | None = None,
        statement_recorder: StatementRecorder,
        cursor_type: str | None = None,
    ) -> int | None:
        statements: tuple[str, ...] = self.render_delete_insert_cursor(
            destination=destination,
            sql=sql,
            cursor_column=cursor_column,
            cursor_start=cursor_start,
            cursor_end=cursor_end,
            columns=columns,
            cursor_type=cursor_type,
        )
        statement_recorder.record_many(statements)
        affected: int | None = None
        with self.transaction(connection):
            stmt: str
            for stmt in statements:
                cursor: Any = self.execute(connection=connection, sql=stmt)
                affected = self.affected_row_count(cursor=cursor)
        return affected

    def merge(
        self,
        *,
        connection: Any,
        destination: str,
        sql: str,
        unique_key: str | tuple[str, ...],
        statement_recorder: StatementRecorder,
        exclude_columns: tuple[str, ...] = (),
    ) -> int | None:
        keys: tuple[str, ...] = (unique_key,) if isinstance(unique_key, str) else unique_key
        source_columns: tuple[str, ...] = self.query_column_names(connection=connection, sql=sql)
        statements: tuple[str, ...] = self.render_merge(
            destination=destination,
            sql=sql,
            unique_key=keys,
            source_columns=source_columns,
            exclude_columns=exclude_columns,
        )
        statement_recorder.record_many(statements)
        affected: int | None = 0
        with self.transaction(connection):
            stmt: str
            for stmt in statements:
                cursor: Any = self.execute(connection=connection, sql=stmt)
                count: int | None = self.affected_row_count(cursor=cursor)
                affected = None if affected is None or count is None else affected + count
        return affected

    def load_seed(
        self,
        *,
        connection: Any,
        destination: str,
        file_path: Path,
        columns: tuple[ColumnInfo, ...],
        csv_settings: SeedCsvSettings = DEFAULT_SEED_CSV_SETTINGS,
        replace: bool = True,
        infer_types: bool = False,
        statement_recorder: StatementRecorder,
    ) -> None:
        del infer_types
        column_names: tuple[str, ...] = tuple(col.name for col in columns)
        column_defs: str = ", ".join(
            f"{self.render_identifier(col.name)} {col.type}" for col in columns
        )
        create_sql: str = f"CREATE TABLE {destination} ({column_defs})"
        insert_sql: str = (
            f"INSERT INTO {destination} "
            f"({', '.join(self.render_identifier(name) for name in column_names)}) "
            f"VALUES ({', '.join(['?'] * len(column_names))})"
        )
        boolean_columns: frozenset[str] = frozenset(
            col.name for col in columns if BOOLEAN_TYPE_MARKER in col.type.upper()
        )
        rows: list[tuple[object, ...]] = []
        with file_path.open(
            "r", encoding=csv_settings.encoding or "utf-8", newline=""
        ) as seed_file:
            reader: csv.DictReader[str] = csv.DictReader(
                seed_file,
                delimiter=csv_settings.delimiter or ",",
                quotechar=csv_settings.quotechar or '"',
                escapechar=csv_settings.escapechar,
                doublequote=True if csv_settings.doublequote is None else csv_settings.doublequote,
                skipinitialspace=(
                    False
                    if csv_settings.skipinitialspace is None
                    else csv_settings.skipinitialspace
                ),
            )
            for row in reader:
                rows.append(
                    tuple(
                        _seed_value(
                            value=normalize_seed_csv_value(
                                value=row.get(col), column_name=col, csv_settings=csv_settings
                            ),
                            boolean=col in boolean_columns,
                        )
                        for col in column_names
                    )
                )
        with self.transaction(connection):
            if replace:
                self.drop(
                    connection=connection,
                    destination=destination,
                    if_exists=True,
                    statement_recorder=statement_recorder,
                )
            statement_recorder.record(create_sql)
            self.execute(connection=connection, sql=create_sql)
            if rows:
                statement_recorder.record(insert_sql)
                connection.executemany(sql=insert_sql, rows=rows)

    def add_columns(
        self,
        *,
        connection: Any,
        destination: str,
        columns: tuple[ColumnInfo, ...],
        statement_recorder: StatementRecorder,
    ) -> None:
        statements: tuple[str, ...] = self.render_add_columns(
            destination=destination, columns=columns
        )
        statement_recorder.record_many(statements)
        with self.transaction(connection):
            for stmt in statements:
                self.execute(connection=connection, sql=stmt)

    def drop_columns(
        self,
        *,
        connection: Any,
        destination: str,
        column_names: tuple[str, ...],
        statement_recorder: StatementRecorder,
    ) -> None:
        statements: tuple[str, ...] = self.render_drop_columns(
            destination=destination, column_names=column_names
        )
        statement_recorder.record_many(statements)
        with self.transaction(connection):
            for stmt in statements:
                self.execute(connection=connection, sql=stmt)

    def rename_column(
        self,
        *,
        connection: Any,
        destination: str,
        old_name: str,
        new_name: str,
        statement_recorder: StatementRecorder,
    ) -> None:
        statements: tuple[str, ...] = self.render_rename_column(
            destination=destination, old_name=old_name, new_name=new_name
        )
        statement_recorder.record_many(statements)
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

    def column_rename_unavailable_reason(self, *, connection: Any, destination: str) -> str | None:
        del connection, destination
        return None

    def widen_target_nullability(
        self,
        *,
        connection: Any,
        destination: str,
        target_columns: tuple[ColumnInfo, ...],
        incoming_columns: tuple[ColumnInfo, ...],
        statement_recorder: StatementRecorder,
    ) -> tuple[ColumnInfo, ...]:
        """Return the target columns unchanged; this warehouse never stores NULL as a default."""

        del connection, destination, incoming_columns, statement_recorder
        return target_columns

    def alter_column_types(
        self,
        *,
        connection: Any,
        destination: str,
        columns: tuple[ColumnInfo, ...],
        statement_recorder: StatementRecorder,
    ) -> None:
        """Rebuild the table with the new types in one transaction, keeping its indexes."""

        table_name: str = unquote_relation(destination)
        rebuilt: str = quote_identifier(f"{table_name}__sqlbuild_retype")
        new_types: dict[str, str] = {col.name.lower(): col.type for col in columns}
        current: tuple[ColumnInfo, ...] = relation_columns(
            connection=connection, table_name=table_name
        )
        projections: str = ", ".join(
            self.render_source_expression_cast(
                expression=self.render_identifier(col.name),
                target_type=new_types[col.name.lower()],
                alias=self.render_identifier(col.name),
            )
            if col.name.lower() in new_types
            else self.render_identifier(col.name)
            for col in current
        )
        index_rows: list[tuple[Any, ...]] = connection.execute_with(
            sql="SELECT sql FROM sqlite_schema WHERE type = 'index' AND lower(tbl_name) = lower(?) "
            "AND sql IS NOT NULL ORDER BY name",
            parameters=(table_name,),
        ).fetchall()
        statements: tuple[str, ...] = (
            f"DROP TABLE IF EXISTS {rebuilt}",
            f"CREATE TABLE {rebuilt} AS SELECT {projections} FROM {destination}",
            f"DROP TABLE {destination}",
            f"ALTER TABLE {rebuilt} RENAME TO {quote_identifier(table_name)}",
            *(str(row[0]) for row in index_rows),
        )
        statement_recorder.record_many(statements)
        token: Token[Mapping[str, str]] = _ENFORCED_COLUMN_TYPES.set(new_types)
        try:
            with self.transaction(connection):
                stmt: str
                for stmt in statements:
                    self.execute(connection=connection, sql=stmt)
        finally:
            _ENFORCED_COLUMN_TYPES.reset(token)

    def inspect_row_diff_coverage(
        self,
        *,
        connection: Any,
        relation: str,
        cursor_column: str | None = None,
        start_cursor: Any | None = None,
        end_cursor: Any | None = None,
    ) -> RowDiffCoverage:
        return self._inspect_row_diff_coverage(
            connection=connection,
            relation=relation,
            cursor_column=cursor_column,
            start_cursor=start_cursor,
            end_cursor=end_cursor,
        )

    def estimate_relation_row_count(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str | None,
        name: str,
        metadata_only: bool = False,
    ) -> RelationRowCountEstimate:
        """Keep interactive counts; background metadata sampling never reads table pages."""

        if metadata_only:
            return RelationRowCountEstimate(
                row_count=None, detail="SQLite does not report table row counts from metadata"
            )
        del database
        table_name: str = physical_name(schema=schema, name=name)
        row: tuple[Any, ...] | None = connection.execute_with(
            sql="SELECT type FROM sqlite_schema WHERE lower(name) = lower(?)",
            parameters=(table_name,),
        ).fetchone()
        if row is None:
            return RelationRowCountEstimate(row_count=None, detail="relation not found")
        if row[0] != SQLITE_TABLE_TYPE:
            return RelationRowCountEstimate(row_count=None, detail="views have no row count")
        count: tuple[Any, ...] = connection.execute(
            f"SELECT COUNT(*) FROM {quote_identifier(table_name)}"
        ).fetchone()
        return RelationRowCountEstimate(row_count=int(count[0]))

    def diff_schema(
        self,
        *,
        connection: Any,
        left: str,
        right: str,
    ) -> SchemaDiffResult:
        left_columns: tuple[ColumnInfo, ...] = self.describe_relation(
            connection=connection, relation=left
        )
        right_columns: tuple[ColumnInfo, ...] = self.describe_relation(
            connection=connection, relation=right
        )
        left_map: dict[str, str] = {col.name: col.type for col in left_columns}
        right_map: dict[str, str] = {col.name: col.type for col in right_columns}
        added: list[ColumnInfo] = []
        removed: list[ColumnInfo] = []
        type_changed: list[tuple[ColumnInfo, ColumnInfo]] = []
        for col_name, col_type in right_map.items():
            if col_name not in left_map:
                added.append(ColumnInfo(name=col_name, type=col_type))
            elif not types_equal(
                left=left_map[col_name], right=col_type, dialect=self.sql_analysis_dialect()
            ):
                type_changed.append(
                    (
                        ColumnInfo(name=col_name, type=left_map[col_name]),
                        ColumnInfo(name=col_name, type=col_type),
                    )
                )
        for col_name, col_type in left_map.items():
            if col_name not in right_map:
                removed.append(ColumnInfo(name=col_name, type=col_type))
        return SchemaDiffResult(
            added_columns=tuple(added),
            removed_columns=tuple(removed),
            type_changed_columns=tuple(type_changed),
            left_column_count=len(left_columns),
            right_column_count=len(right_columns),
        )

    def diff_rows(
        self,
        *,
        connection: Any,
        left: str,
        right: str,
        unique_key: str | tuple[str, ...],
        excluded_columns: tuple[str, ...] = (),
        tolerances: RowDiffTolerances | None = None,
        cursor_column: str | None = None,
        start_cursor: Any | None = None,
        end_cursor: Any | None = None,
    ) -> RowDiffResult:
        keys: tuple[str, ...] = (unique_key,) if isinstance(unique_key, str) else unique_key
        sampling: RowDiffSampling | None = tolerances.sampling if tolerances is not None else None
        left_columns: tuple[ColumnInfo, ...] = self.describe_relation(
            connection=connection, relation=left
        )
        compare_columns: tuple[str, ...] = tuple(
            col.name
            for col in left_columns
            if col.name not in keys and col.name not in excluded_columns
        )
        left_columns_by_name: dict[str, ColumnInfo] = {col.name: col for col in left_columns}
        cursor_filter: str = self.build_cursor_filter(
            cursor_column=cursor_column,
            start_cursor=start_cursor,
            end_cursor=end_cursor,
        )
        left_cte: str = f"SELECT * FROM {left}"
        right_cte: str = f"SELECT * FROM {right}"
        if cursor_filter:
            left_cte += f" WHERE {cursor_filter}"
            right_cte += f" WHERE {cursor_filter}"
        self.validate_row_diff_keys(
            connection=connection, relation_sql=left_cte, relation_label="left", keys=keys
        )
        self.validate_row_diff_keys(
            connection=connection, relation_sql=right_cte, relation_label="right", keys=keys
        )
        prepared_relations: RowDiffPreparedRelations = self._build_row_diff_relation_ctes(
            connection=connection,
            left_sql=left_cte,
            right_sql=right_cte,
            keys=keys,
            sampling=sampling,
            inspect_population=True,
        )
        join_condition: str = " AND ".join(f"__left.{k} = __right.{k}" for k in keys)
        column_equal_expressions: dict[str, str] = {
            col: self.build_row_diff_equal_expression(
                column=col,
                column_info=left_columns_by_name[col],
                tolerances=tolerances,
            )
            for col in compare_columns
        }
        column_tolerances: dict[str, RowDiffTolerance | None] = {
            col: self.resolve_row_diff_tolerance(
                column=col,
                column_type=left_columns_by_name[col].type,
                tolerances=tolerances,
            )
            for col in compare_columns
        }
        equal_condition: str = "TRUE"
        if compare_columns:
            equal_condition = " AND ".join(column_equal_expressions.values())
        column_count_sql_parts: list[str] = [
            f"COUNT(CASE WHEN __left.{keys[0]} IS NOT NULL "
            f"AND __right.{keys[0]} IS NOT NULL "
            f"AND NOT ({column_equal_expressions[col]}) THEN 1 END) AS __{col}_mismatch_count"
            for col in compare_columns
        ]
        column_count_sql: str = (
            ", " + ", ".join(column_count_sql_parts) if column_count_sql_parts else ""
        )
        diff_sql: str = (
            f"WITH {prepared_relations.cte_sql} "
            f"SELECT "
            f"COUNT(CASE WHEN __left.{keys[0]} IS NOT NULL THEN 1 END) AS left_count, "
            f"COUNT(CASE WHEN __right.{keys[0]} IS NOT NULL THEN 1 END) AS right_count, "
            f"COUNT(*) AS joined, "
            f"COUNT(CASE WHEN __left.{keys[0]} IS NOT NULL "
            f"AND __right.{keys[0]} IS NOT NULL AND ({equal_condition}) THEN 1 END) AS equal, "
            f"COUNT(CASE WHEN __left.{keys[0]} IS NOT NULL "
            f"AND __right.{keys[0]} IS NOT NULL AND NOT ({equal_condition}) "
            f"THEN 1 END) AS unequal, "
            f"COUNT(CASE WHEN __right.{keys[0]} IS NULL THEN 1 END) AS left_only, "
            f"COUNT(CASE WHEN __left.{keys[0]} IS NULL THEN 1 END) AS right_only"
            f"{column_count_sql} FROM __left FULL OUTER JOIN __right ON {join_condition}"
        )
        row: tuple[Any, ...] = self.execute(connection=connection, sql=diff_sql).fetchone()
        column_results: tuple[RowDiffColumnResult, ...] = tuple(
            RowDiffColumnResult(
                name=col,
                mismatched_count=int(row[index]),
                tolerance=column_tolerances[col],
            )
            for index, col in enumerate(compare_columns, start=7)
        )
        return RowDiffResult(
            left_count=int(row[0]),
            right_count=int(row[1]),
            joined_count=int(row[2]),
            equal_count=int(row[3]),
            unequal_count=int(row[4]),
            left_only_count=int(row[5]),
            right_only_count=int(row[6]),
            column_results=column_results,
            population_count=prepared_relations.population_count or int(row[2]),
            compared_count=int(row[2]),
            sampling=sampling,
        )

    def sample_unequal_rows(
        self,
        *,
        connection: Any,
        left: str,
        right: str,
        unique_key: str | tuple[str, ...],
        excluded_columns: tuple[str, ...] = (),
        tolerances: RowDiffTolerances | None = None,
        cursor_column: str | None = None,
        start_cursor: Any | None = None,
        end_cursor: Any | None = None,
        limit: int = 20,
    ) -> tuple[RowDiffSampleRow, ...]:
        sampling: RowDiffSampling | None = tolerances.sampling if tolerances is not None else None
        keys: tuple[str, ...] = (unique_key,) if isinstance(unique_key, str) else unique_key
        left_columns: tuple[ColumnInfo, ...] = self.describe_relation(
            connection=connection, relation=left
        )
        compare_columns: tuple[str, ...] = tuple(
            col.name
            for col in left_columns
            if col.name not in keys and col.name not in excluded_columns
        )
        left_columns_by_name: dict[str, ColumnInfo] = {col.name: col for col in left_columns}
        cursor_filter: str = self.build_cursor_filter(
            cursor_column=cursor_column,
            start_cursor=start_cursor,
            end_cursor=end_cursor,
        )
        left_cte: str = f"SELECT * FROM {left}"
        right_cte: str = f"SELECT * FROM {right}"
        if cursor_filter:
            left_cte += f" WHERE {cursor_filter}"
            right_cte += f" WHERE {cursor_filter}"
        self.validate_row_diff_keys(
            connection=connection, relation_sql=left_cte, relation_label="left", keys=keys
        )
        self.validate_row_diff_keys(
            connection=connection, relation_sql=right_cte, relation_label="right", keys=keys
        )
        prepared_relations: RowDiffPreparedRelations = self._build_row_diff_relation_ctes(
            connection=connection,
            left_sql=left_cte,
            right_sql=right_cte,
            keys=keys,
            sampling=sampling,
            inspect_population=False,
        )
        column_equal_expressions: dict[str, str] = {
            col: self.build_row_diff_equal_expression(
                column=col,
                column_info=left_columns_by_name[col],
                tolerances=tolerances,
            )
            for col in compare_columns
        }
        unequal_condition: str = (
            " OR ".join(f"NOT ({expr})" for expr in column_equal_expressions.values())
            if compare_columns
            else "FALSE"
        )
        join_condition: str = " AND ".join(f"__left.{k} = __right.{k}" for k in keys)
        key_select_sql: str = ", ".join(
            f"COALESCE(__left.{k}, __right.{k}) AS __key_{k}" for k in keys
        )
        compare_select_sql: str = ", ".join(
            f"__left.{col} AS __left_{col}, __right.{col} AS __right_{col}"
            for col in compare_columns
        )
        if compare_select_sql:
            compare_select_sql = ", " + compare_select_sql
        sample_sql: str = (
            f"WITH {prepared_relations.cte_sql} "
            f"SELECT {key_select_sql}{compare_select_sql} "
            f"FROM __left FULL OUTER JOIN __right ON {join_condition} "
            f"WHERE __left.{keys[0]} IS NOT NULL AND __right.{keys[0]} IS NOT NULL "
            f"AND ({unequal_condition}) "
            f"ORDER BY {', '.join(f'__key_{k}' for k in keys)} LIMIT {limit}"
        )
        rows: list[tuple[Any, ...]] = self.execute(connection=connection, sql=sample_sql).fetchall()
        samples: list[RowDiffSampleRow] = []
        for row in rows:
            key_values: tuple[tuple[str, object], ...] = tuple(
                (k, row[i]) for i, k in enumerate(keys)
            )
            changed_cells: list[RowDiffSampleCell] = []
            for col_index, col in enumerate(compare_columns):
                left_i: int = len(keys) + (col_index * 2)
                left_val: object = row[left_i]
                right_val: object = row[left_i + 1]
                if left_val != right_val:
                    changed_cells.append(
                        RowDiffSampleCell(name=col, left_value=left_val, right_value=right_val)
                    )
            samples.append(
                RowDiffSampleRow(key_values=key_values, changed_cells=tuple(changed_cells))
            )
        return tuple(samples)

    def sample_side_only_rows(
        self,
        *,
        connection: Any,
        left: str,
        right: str,
        unique_key: str | tuple[str, ...],
        side: str,
        cursor_column: str | None = None,
        start_cursor: Any | None = None,
        end_cursor: Any | None = None,
        limit: int = 20,
        sampling: RowDiffSampling | None = None,
    ) -> tuple[tuple[tuple[str, object], ...], ...]:
        keys: tuple[str, ...] = (unique_key,) if isinstance(unique_key, str) else unique_key
        cursor_filter: str = self.build_cursor_filter(
            cursor_column=cursor_column,
            start_cursor=start_cursor,
            end_cursor=end_cursor,
        )
        left_cte: str = f"SELECT * FROM {left}"
        right_cte: str = f"SELECT * FROM {right}"
        if cursor_filter:
            left_cte += f" WHERE {cursor_filter}"
            right_cte += f" WHERE {cursor_filter}"
        self.validate_row_diff_keys(
            connection=connection, relation_sql=left_cte, relation_label="left", keys=keys
        )
        self.validate_row_diff_keys(
            connection=connection, relation_sql=right_cte, relation_label="right", keys=keys
        )
        prepared_relations: RowDiffPreparedRelations = self._build_row_diff_relation_ctes(
            connection=connection,
            left_sql=left_cte,
            right_sql=right_cte,
            keys=keys,
            sampling=sampling,
            inspect_population=False,
        )
        join_condition: str = " AND ".join(f"__left.{k} = __right.{k}" for k in keys)
        key_select_sql: str = ", ".join(
            f"COALESCE(__left.{k}, __right.{k}) AS __key_{k}" for k in keys
        )
        if side == DIFF_LEFT_SIDE:
            side_condition: str = f"__left.{keys[0]} IS NOT NULL AND __right.{keys[0]} IS NULL"
        elif side == DIFF_RIGHT_SIDE:
            side_condition = f"__right.{keys[0]} IS NOT NULL AND __left.{keys[0]} IS NULL"
        else:
            raise AdapterUserError(message="sample_side_only_rows side must be 'left' or 'right'")
        sample_sql: str = (
            f"WITH {prepared_relations.cte_sql} "
            f"SELECT {key_select_sql} "
            f"FROM __left FULL OUTER JOIN __right ON {join_condition} "
            f"WHERE {side_condition} "
            f"ORDER BY {', '.join(f'__key_{k}' for k in keys)} LIMIT {limit}"
        )
        rows: list[tuple[Any, ...]] = self.execute(connection=connection, sql=sample_sql).fetchall()
        samples: list[tuple[tuple[str, object], ...]] = []
        row: tuple[Any, ...]
        for row in rows:
            samples.append(tuple(zip(keys, row, strict=False)))
        return tuple(samples)

    def validate_row_diff_keys(
        self,
        *,
        connection: Any,
        relation_sql: str,
        relation_label: str,
        keys: tuple[str, ...],
    ) -> None:
        if not keys:
            raise AdapterUserError(message="row diff requires at least one unique_key column")
        null_condition: str = " OR ".join(f"{key} IS NULL" for key in keys)
        null_row: tuple[Any, ...] = self.execute(
            connection=connection,
            sql=f"SELECT COUNT(*) FROM ({relation_sql}) AS __key_check WHERE {null_condition}",
        ).fetchone()
        if int(null_row[0]) > 0:
            raise AdapterUserError(
                message=f"row diff {relation_label} relation contains null unique_key values"
            )
        key_list: str = ", ".join(keys)
        duplicate_row: tuple[Any, ...] = self.execute(
            connection=connection,
            sql=(
                f"SELECT COUNT(*) FROM (SELECT {key_list} FROM ({relation_sql}) AS __key_check "
                f"GROUP BY {key_list} HAVING COUNT(*) > 1) AS __duplicates"
            ),
        ).fetchone()
        if int(duplicate_row[0]) > 0:
            raise AdapterUserError(
                message=f"row diff {relation_label} relation contains duplicate unique_key values"
            )

    def build_row_diff_equal_expression(
        self,
        *,
        column: str,
        column_info: ColumnInfo,
        tolerances: RowDiffTolerances | None,
    ) -> str:
        """Compare with tolerances using SQLite's multi-argument max(), which has no GREATEST."""

        tolerance: RowDiffTolerance | None = self.resolve_row_diff_tolerance(
            column=column,
            column_type=column_info.type,
            tolerances=tolerances,
        )
        left_expression: str = f"__left.{column}"
        right_expression: str = f"__right.{column}"
        if tolerance is None:
            return f"{left_expression} IS NOT DISTINCT FROM {right_expression}"
        threshold_parts: list[str] = []
        if tolerance.absolute is not None:
            threshold_parts.append(self.format_row_diff_decimal_sql(tolerance.absolute))
        if tolerance.relative is not None:
            threshold_parts.append(
                f"{self.format_row_diff_decimal_sql(tolerance.relative)} * "
                f"max(ABS({left_expression}), ABS({right_expression}))"
            )
        threshold_sql: str = threshold_parts[0]
        if len(threshold_parts) > 1:
            threshold_sql = f"max({', '.join(threshold_parts)})"
        return (
            f"(({left_expression} IS NULL AND {right_expression} IS NULL) OR "
            f"({left_expression} IS NOT NULL AND {right_expression} IS NOT NULL AND "
            f"ABS({left_expression} - {right_expression}) <= {threshold_sql}))"
        )

    def resolve_row_diff_tolerance(
        self,
        *,
        column: str,
        column_type: str,
        tolerances: RowDiffTolerances | None,
    ) -> RowDiffTolerance | None:
        if tolerances is None:
            return None
        column_tolerance: RowDiffTolerance | None = tolerances.by_column.get(column)
        if column_tolerance is not None:
            if self.normalize_row_diff_numeric_type(column_type) is None:
                raise AdapterUserError(
                    message=f"row diff tolerance for non-numeric column '{column}' is invalid"
                )
            self.validate_row_diff_tolerance(column=column, tolerance=column_tolerance)
            return column_tolerance
        normalized_type: str | None = self.normalize_row_diff_numeric_type(column_type)
        if normalized_type is None:
            return None
        type_tolerance: RowDiffTolerance | None = tolerances.by_type.get(normalized_type)
        if type_tolerance is not None:
            self.validate_row_diff_tolerance(column=column, tolerance=type_tolerance)
        return type_tolerance

    def validate_row_diff_tolerance(self, *, column: str, tolerance: RowDiffTolerance) -> None:
        if tolerance.absolute is None and tolerance.relative is None:
            raise AdapterUserError(
                message=f"row diff tolerance for column '{column}' must define absolute or relative"
            )

    def normalize_row_diff_numeric_type(self, column_type: str) -> str | None:
        return normalize_numeric_family(type_sql=column_type, dialect=self.sql_analysis_dialect())

    def format_row_diff_decimal_sql(self, value: Decimal) -> str:
        return format(value, "f")

    def _render_row_diff_digest(self, payload_sql: str) -> str:
        """Hash through the deterministic function every connection registers."""

        return f"{HASH_FUNCTION_NAME}({payload_sql})"

    def render_create_initial_snapshot_destination(
        self,
        *,
        table_type: str,
        destination: str,
        origin: str,
        snapshot_strategy: str | None,
        updated_at_column: str | None,
        observed_at_column: str | None,
        valid_from_column: str,
        valid_to_column: str,
        initial_valid_from: str | None,
    ) -> tuple[str, ...]:
        return self.render_create_table_as(
            destination=destination,
            sql=SnapshotSql(dialect=self._snapshot_sql_dialect).initial_select_sql(
                origin=origin,
                snapshot_strategy=snapshot_strategy,
                updated_at_column=updated_at_column,
                observed_at_column=observed_at_column,
                valid_from_column=valid_from_column,
                valid_to_column=valid_to_column,
                initial_valid_from=initial_valid_from,
                current_timestamp=self.render_current_timestamp(),
            ),
        )

    def render_apply_timestamp_snapshot_changes(
        self,
        *,
        destination: str,
        origin: str,
        unique_key: tuple[str, ...],
        updated_at_column: str,
        observed_at_column: str | None,
        valid_from_column: str,
        valid_to_column: str,
        initial_valid_from: str | None,
        output_columns: tuple[str, ...],
        invalidate_hard_deletes: bool,
    ) -> tuple[str, ...]:
        return SnapshotSql(dialect=self._snapshot_sql_dialect).timestamp_changes_sql(
            target=SnapshotChangeTarget(
                destination=destination,
                origin=origin,
                unique_key=unique_key,
                valid_from_column=valid_from_column,
                valid_to_column=valid_to_column,
                output_columns=output_columns,
            ),
            updated_at_column=updated_at_column,
            observed_at_column=observed_at_column,
            initial_valid_from=initial_valid_from,
            invalidate_hard_deletes=invalidate_hard_deletes,
            current_timestamp=self.render_current_timestamp(),
        )

    def render_apply_check_snapshot_changes(
        self,
        *,
        target: SnapshotChangeTarget,
        check_columns: tuple[str, ...],
        updated_at_column: str | None,
        observed_at_column: str | None,
        initial_valid_from: str | None,
        invalidate_hard_deletes: bool,
    ) -> tuple[str, ...]:
        return SnapshotSql(dialect=self._snapshot_sql_dialect).check_changes_sql(
            target=target,
            check_columns=check_columns,
            updated_at_column=updated_at_column,
            observed_at_column=observed_at_column,
            initial_valid_from=initial_valid_from,
            invalidate_hard_deletes=invalidate_hard_deletes,
            current_timestamp=self.render_current_timestamp(),
        )

    def render_create_initial_historical_timestamp_snapshot_destination(
        self,
        *,
        table_type: str,
        destination: str,
        origin: str,
        unique_key: tuple[str, ...],
        updated_at_column: str,
        observed_at_column: str,
        valid_from_column: str,
        valid_to_column: str,
        output_columns: tuple[str, ...],
        invalidate_hard_deletes: bool,
    ) -> tuple[str, ...]:
        historical_sql: str = HistoricalTimestampSnapshotSql(
            dialect=self._snapshot_sql_dialect
        ).initial_select_sql(
            origin=origin,
            unique_key=unique_key,
            updated_at_column=updated_at_column,
            observed_at_column=observed_at_column,
            valid_from_column=valid_from_column,
            valid_to_column=valid_to_column,
            output_columns=output_columns,
            invalidate_hard_deletes=invalidate_hard_deletes,
        )
        return self.render_create_table_as(destination=destination, sql=historical_sql)

    def render_create_initial_historical_timestamp_changes_destination(
        self,
        *,
        table_type: str,
        destination: str,
        origin: str,
        unique_key: tuple[str, ...],
        updated_at_column: str,
        valid_from_column: str,
        valid_to_column: str,
        output_columns: tuple[str, ...],
    ) -> tuple[str, ...]:
        historical_sql: str = HistoricalTimestampSnapshotSql.changes_initial_select_sql(
            origin=origin,
            unique_key=unique_key,
            updated_at_column=updated_at_column,
            valid_from_column=valid_from_column,
            valid_to_column=valid_to_column,
            output_columns=output_columns,
        )
        return self.render_create_table_as(destination=destination, sql=historical_sql)

    def render_apply_historical_timestamp_snapshot_changes(
        self,
        *,
        destination: str,
        origin: str,
        unique_key: tuple[str, ...],
        updated_at_column: str,
        observed_at_column: str,
        valid_from_column: str,
        valid_to_column: str,
        output_columns: tuple[str, ...],
        invalidate_hard_deletes: bool,
    ) -> tuple[str, ...]:
        return HistoricalTimestampSnapshotSql(dialect=self._snapshot_sql_dialect).apply_sql(
            destination=destination,
            origin=origin,
            unique_key=unique_key,
            updated_at_column=updated_at_column,
            observed_at_column=observed_at_column,
            valid_from_column=valid_from_column,
            valid_to_column=valid_to_column,
            output_columns=output_columns,
            invalidate_hard_deletes=invalidate_hard_deletes,
        )

    def render_apply_historical_timestamp_changes(
        self,
        *,
        destination: str,
        origin: str,
        unique_key: tuple[str, ...],
        updated_at_column: str,
        valid_from_column: str,
        valid_to_column: str,
        output_columns: tuple[str, ...],
    ) -> tuple[str, ...]:
        return HistoricalTimestampSnapshotSql(dialect=self._snapshot_sql_dialect).changes_apply_sql(
            destination=destination,
            origin=origin,
            unique_key=unique_key,
            updated_at_column=updated_at_column,
            valid_from_column=valid_from_column,
            valid_to_column=valid_to_column,
            output_columns=output_columns,
        )

    def render_create_initial_historical_check_snapshot_destination(
        self,
        *,
        table_type: str,
        destination: str,
        origin: str,
        unique_key: tuple[str, ...],
        check_columns: tuple[str, ...],
        observed_at_column: str,
        valid_from_column: str,
        valid_to_column: str,
        output_columns: tuple[str, ...],
        invalidate_hard_deletes: bool,
    ) -> tuple[str, ...]:
        historical_sql: str = HistoricalCheckSnapshotSql(
            dialect=self._snapshot_sql_dialect
        ).initial_select_sql(
            origin=origin,
            unique_key=unique_key,
            check_columns=check_columns,
            observed_at_column=observed_at_column,
            valid_from_column=valid_from_column,
            valid_to_column=valid_to_column,
            output_columns=output_columns,
            invalidate_hard_deletes=invalidate_hard_deletes,
        )
        return self.render_create_table_as(destination=destination, sql=historical_sql)

    def render_apply_historical_check_snapshot_changes(
        self,
        *,
        destination: str,
        origin: str,
        unique_key: tuple[str, ...],
        check_columns: tuple[str, ...],
        observed_at_column: str,
        valid_from_column: str,
        valid_to_column: str,
        output_columns: tuple[str, ...],
        invalidate_hard_deletes: bool,
    ) -> tuple[str, ...]:
        return HistoricalCheckSnapshotSql(dialect=self._snapshot_sql_dialect).apply_sql(
            destination=destination,
            origin=origin,
            unique_key=unique_key,
            check_columns=check_columns,
            observed_at_column=observed_at_column,
            valid_from_column=valid_from_column,
            valid_to_column=valid_to_column,
            output_columns=output_columns,
            invalidate_hard_deletes=invalidate_hard_deletes,
        )

    def star_exclude_keyword(self) -> str:
        """Return EXCLUDE; SQLite has no star exclusion, so casts list columns explicitly."""

        return "EXCLUDE"

    def render_qualified_name(
        self,
        *,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> str | None:
        """Render `schema__name`, with `main` unprefixed; one file has no databases."""

        if database is not None:
            raise AdapterUserError(
                message=(
                    f"SQLite relation '{name}' cannot name database '{database}': a SQLite "
                    "connection is one file, so targets and models set only a schema"
                ),
                code=INVALID_CONNECTION_CODE,
            )
        if schema is None:
            return None
        physical: str = physical_name(schema=schema, name=name)
        self._logical_relations.remember(schema=schema, name=name, physical=physical)
        return physical

    def render_identifier(self, name: str) -> str:
        return '"' + name.replace('"', '""') + '"'

    def render_exact_identifier(self, name: str) -> str:
        return self.render_identifier(name)

    def render_framework_type(self, type_name: FrameworkType) -> str:
        match type_name:
            case FrameworkType.STRING:
                return "TEXT"
            case FrameworkType.INTEGER:
                return "INTEGER"
            case FrameworkType.TIMESTAMP:
                return "TIMESTAMP"

    def render_loader_logical_type(self, type_name: LoaderLogicalType) -> str:
        match type_name:
            case LoaderLogicalType.BOOLEAN:
                return "BOOLEAN"
            case LoaderLogicalType.INTEGER:
                return "INTEGER"
            case LoaderLogicalType.FLOAT:
                return "REAL"
            case LoaderLogicalType.STRING:
                return "TEXT"
            case LoaderLogicalType.TIMESTAMP:
                return "TIMESTAMP"
            case LoaderLogicalType.DATE:
                return "DATE"
            case LoaderLogicalType.JSON:
                return "TEXT"

    def render_typed_scalar(self, *, value: SqlValue) -> str:
        return _render_ansi_typed_scalar(value=value)

    def render_typed_value_list(self, *, value: SqlValue) -> str:
        return _render_typed_value_list(value=value, render_scalar=self.render_typed_scalar)

    def render_typed_array(self, *, value: SqlValue) -> str:
        """Render arrays as JSON text, SQLite's only collection representation."""

        return _quote_sql_string(_encode_typed_json(value))

    def render_typed_object(self, *, value: SqlValue) -> str:
        return _quote_sql_string(_encode_typed_json(value))

    def render_loader_value_literal(
        self, *, value: object, logical_type: LoaderLogicalType | None
    ) -> str:
        if value is None:
            return "NULL"
        if logical_type == LoaderLogicalType.JSON or isinstance(value, dict | list):
            return _quote_sql_string(json.dumps(value, sort_keys=True))
        if isinstance(value, bool):
            return "1" if value else "0"
        if isinstance(value, int | float | Decimal):
            return str(value)
        if isinstance(value, datetime):
            return _quote_sql_string(canonical_timestamp_text(value.isoformat()))
        if isinstance(value, date):
            return _quote_sql_string(value.isoformat())
        return _quote_sql_string(str(value))

    def render_loader_rows_select(
        self,
        *,
        rows: tuple[dict[str, object], ...],
        column_names: tuple[str, ...],
        column_sql_types: dict[str, str],
        inferred_types: dict[str, LoaderLogicalType],
    ) -> str:
        """Render uncast rows as one VALUES; staging declares the types and affinity converts."""

        del column_sql_types
        if not rows:
            projections: str = ", ".join(
                f"NULL AS {self.render_identifier(column_name)}" for column_name in column_names
            )
            return f"SELECT {projections} WHERE 1 = 0"
        value_rows: list[str] = []
        row: dict[str, object]
        for row in rows:
            literals: str = ", ".join(
                self.render_loader_value_literal(
                    value=row.get(column_name), logical_type=inferred_types.get(column_name)
                )
                for column_name in column_names
            )
            value_rows.append(f"({literals})")
        values_sql: str = ", ".join(value_rows)
        select_sql: str = ", ".join(
            f"column{index} AS {self.render_identifier(column_name)}"
            for index, column_name in enumerate(column_names, start=1)
        )
        return f"SELECT {select_sql} FROM (VALUES {values_sql}) AS __loader_rows"

    def render_source_expression_cast(
        self, *, expression: str, target_type: str, alias: str
    ) -> str:
        """Cast, keeping date and time text and rendering exact decimals as fixed-scale text."""

        if _TEMPORAL_TYPE.match(target_type) is not None:
            return f"{expression} AS {alias}"
        scale: int | None = decimal_scale(target_type)
        if scale is None:
            return f"CAST({expression} AS {target_type}) AS {alias}"
        return (
            f"CASE WHEN {expression} IS NULL THEN NULL "
            f"ELSE printf('%.{scale}f', {expression}) END AS {alias}"
        )

    def render_source_expression_relation(self, *, expression: str) -> str:
        stripped_expression: str = expression.strip().removesuffix(";").strip()
        if stripped_expression.startswith("("):
            return stripped_expression
        if stripped_expression.lower().startswith(("select", "with", "values")):
            return f"({stripped_expression})"
        return stripped_expression

    def render_source_freshness_max_query(
        self, *, column: str, source_relation: str, source_is_subquery: bool, where_sql: str
    ) -> str:
        relation: str = (
            f"{source_relation} AS __source_freshness" if source_is_subquery else source_relation
        )
        return (
            f"SELECT MAX({self.render_identifier(column)}) AS data_version "
            f"FROM {relation}{where_sql}"
        )

    def render_source_expression_cast_subquery(
        self, *, source_relation: str, projections: tuple[str, ...]
    ) -> str:
        projection_clause: str = ", ".join(projections)
        return f"(SELECT {projection_clause} FROM {source_relation} AS __source_expression)"

    def render_source_relation_cast_subquery(
        self,
        *,
        source_relation: str,
        cast_projections: tuple[str, ...],
        cast_column_names: tuple[str, ...],
        all_columns_cast: bool,
    ) -> str:
        cast_clause: str = ", ".join(cast_projections)
        if all_columns_cast:
            return f"(SELECT {cast_clause} FROM {source_relation})"
        raise AdapterUserError(
            message=(
                "SQLite has no SELECT * EXCLUDE; partial source casts need the warehouse columns"
            ),
            code=UNSUPPORTED_FEATURE_CODE,
        )

    def _render_source_relation_cast_subquery_with_columns(
        self,
        *,
        source_relation: str,
        cast_projections: tuple[str, ...],
        cast_column_names: tuple[str, ...],
        warehouse_column_names: tuple[str, ...],
        all_columns_cast: bool,
    ) -> str:
        return _render_explicit_source_relation_cast_subquery(
            source_relation=source_relation,
            cast_projections=cast_projections,
            cast_column_names=cast_column_names,
            warehouse_column_names=warehouse_column_names,
            all_columns_cast=all_columns_cast,
            render_identifier=self.render_identifier,
        )

    def requires_derived_table_aliases(self) -> bool:
        return False

    def render_set_difference_operator(self) -> str:
        return "EXCEPT"

    def default_schema(self) -> str:
        return MAIN_SCHEMA

    def default_database(self) -> str | None:
        return None

    def describe_relation(self, *, connection: Any, relation: str) -> tuple[ColumnInfo, ...]:
        return relation_columns(connection=connection, table_name=unquote_relation(relation))

    def render_create_table_as(self, *, destination: str, sql: str) -> tuple[str, ...]:
        return (f"DROP TABLE IF EXISTS {destination}", f"CREATE TABLE {destination} AS {sql}")

    def render_rename(self, *, origin: str, destination: str) -> tuple[str, ...]:
        return (f"ALTER TABLE {origin} RENAME TO {destination}",)

    def render_rename_view(self, *, origin: str, destination: str) -> tuple[str, ...]:
        """Render ALTER VIEW, which the adapter runs by recreating the view under the new name."""

        return (f"ALTER VIEW {origin} RENAME TO {destination}",)

    def render_create_fingerprint_table_sql(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> str:
        from sqlbuild._native import build_create_table_sql

        return build_create_table_sql(
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
            render_framework_type=self.render_framework_type,
        )

    def render_create_fingerprint_index_sqls(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        from sqlbuild._native import FINGERPRINT_TABLE_NAME

        return self._index_sqls(
            database=database,
            schema=schema,
            table=FINGERPRINT_TABLE_NAME,
            indexes=(("latest_idx", "node_type, node_name, ts DESC, run_id DESC"),),
        )

    def render_create_microbatch_state_index_sqls(
        self, *, database: str | None, schema: str
    ) -> tuple[str, ...]:
        """Return no indexes: the state table serves only concurrency, which SQLite refuses."""

        del database, schema
        return ()

    def render_read_latest_fingerprints_sql(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> str:
        from sqlbuild._native import build_read_latest_sql

        return build_read_latest_sql(
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
        )

    def render_prune_fingerprint_history_sql(
        self,
        *,
        database: str | None,
        schema: str,
        retain_versions: int,
    ) -> str:
        from sqlbuild._native import FINGERPRINT_TABLE_NAME

        return self._prune_history_sql(
            table=self.render_qualified_name(
                database=database, schema=schema, name=FINGERPRINT_TABLE_NAME
            ),
            partition="node_type, node_name",
            order="ts DESC, run_id DESC",
            retain_versions=retain_versions,
        )

    def render_create_source_freshness_table_sql(self, *, database: str | None, schema: str) -> str:
        from sqlbuild._native import build_source_freshness_create_table_sql

        return build_source_freshness_create_table_sql(
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
            render_framework_type=self.render_framework_type,
        )

    def render_create_source_freshness_index_sqls(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        from sqlbuild._native import SOURCE_FRESHNESS_TABLE_NAME

        return self._index_sqls(
            database=database,
            schema=schema,
            table=SOURCE_FRESHNESS_TABLE_NAME,
            indexes=(
                (
                    "latest_idx",
                    "source_name, target_database, target_schema, target_name, "
                    "observed_at DESC, run_id DESC",
                ),
            ),
        )

    def render_insert_source_freshness_records_sql(
        self,
        *,
        database: str | None,
        schema: str,
        records: tuple[SourceFreshnessRecord, ...],
    ) -> str:
        return render_insert_source_freshness_records_sql(
            database=database,
            schema=schema,
            records=records,
            render_qualified_name=self.render_qualified_name,
        )

    def render_create_node_result_table_sql(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> str:
        from sqlbuild.executor.node_results.main.create_table_sql import (
            build_node_results_create_table_sql,
        )

        return build_node_results_create_table_sql(
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
            render_framework_type=self.render_framework_type,
        )

    def render_create_node_result_index_sqls(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        from sqlbuild._native import NODE_RESULTS_TABLE_NAME

        return self._index_sqls(
            database=database,
            schema=schema,
            table=NODE_RESULTS_TABLE_NAME,
            indexes=(
                (
                    "latest_idx",
                    "node_type, node_name, target_database, target_schema, target_name, "
                    "status, ts DESC, run_id DESC",
                ),
                (
                    "run_id_idx",
                    "run_id, node_type, node_name, target_database, target_schema, target_name",
                ),
            ),
        )

    def render_create_audit_result_table_sql(self, *, database: str | None, schema: str) -> str:
        from sqlbuild.executor.audit_results.main.create_table_sql import (
            build_audit_results_create_table_sql,
        )

        return build_audit_results_create_table_sql(
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
            render_framework_type=self.render_framework_type,
        )

    def render_create_audit_result_index_sqls(
        self, *, database: str | None, schema: str
    ) -> tuple[str, ...]:
        from sqlbuild._native import AUDIT_RESULTS_TABLE_NAME

        return self._index_sqls(
            database=database,
            schema=schema,
            table=AUDIT_RESULTS_TABLE_NAME,
            indexes=(
                (
                    "latest_idx",
                    "audit_name, binding_key, execution_fingerprint, run_scope_phase, "
                    "occurred_at DESC, result_id DESC",
                ),
                ("run_id_idx", "run_id, invocation_id, result_id"),
            ),
        )

    def render_create_janitor_event_table_sql(self, *, database: str | None, schema: str) -> str:
        from sqlbuild.executor.janitor_events.main.create_table_sql import (
            build_janitor_events_create_table_sql,
        )

        return build_janitor_events_create_table_sql(
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
            render_framework_type=self.render_framework_type,
        )

    def render_create_migration_state_table_sql(self, *, database: str | None, schema: str) -> str:
        from sqlbuild._native import render_migration_state_create_sql

        return render_migration_state_create_sql(
            kind="model",
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
            render_framework_type=self.render_framework_type,
        )

    def render_create_old_name_view_state_table_sql(
        self, *, database: str | None, schema: str
    ) -> str:
        from sqlbuild._native import render_migration_state_create_sql

        return render_migration_state_create_sql(
            kind="old_name",
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
            render_framework_type=self.render_framework_type,
        )

    def render_create_column_migration_state_table_sql(
        self, *, database: str | None, schema: str
    ) -> str:
        from sqlbuild._native import render_migration_state_create_sql

        return render_migration_state_create_sql(
            kind="column",
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
            render_framework_type=self.render_framework_type,
        )

    def render_read_latest_source_freshness_sql(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> str:
        from sqlbuild._native import build_source_freshness_read_latest_sql

        return build_source_freshness_read_latest_sql(
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
        )

    def render_prune_source_freshness_history_sql(
        self,
        *,
        database: str | None,
        schema: str,
        retain_versions: int,
    ) -> str:
        from sqlbuild._native import SOURCE_FRESHNESS_TABLE_NAME

        return self._prune_history_sql(
            table=self.render_qualified_name(
                database=database, schema=schema, name=SOURCE_FRESHNESS_TABLE_NAME
            ),
            partition="source_name, target_database, target_schema, target_name",
            order="observed_at DESC, run_id DESC",
            retain_versions=retain_versions,
        )

    def sql_analysis_dialect(self) -> str | None:
        return self.sql_analysis_dialect_name

    def expression_inference_profile(self) -> ExpressionInferenceProfile:
        return ExpressionInferenceProfile(sql_analysis_dialect=self.sql_analysis_dialect())

    def render_cursor_bound_literal(self, *, value: str, cursor_type: str | None) -> str:
        """Render bounds as literals; timestamps in the text form SQLite stores and compares."""

        if str(cursor_type).lower() == CursorKind.INTEGER:
            return value
        if value in {MICROBATCH_START_SENTINEL, MICROBATCH_END_SENTINEL}:
            return f"{CURSOR_TEXT_FUNCTION_NAME}({_quote_sql_string(value)})"
        return _quote_sql_string(canonical_timestamp_text(value))

    def default_table_promotion_mode(self) -> TablePromotionMode:
        return TablePromotionMode.STAGED

    def default_promotion_strategy(self) -> PromotionStrategy:
        return PromotionStrategy.ATOMIC_SWAP

    def _cursor_value_literal(self, cursor: CursorValue) -> str:
        value: datetime | int = cursor.value
        text: str = value.isoformat() if isinstance(value, datetime) else str(value)
        return self.render_cursor_bound_literal(value=text, cursor_type=cursor.kind)

    def _index_sqls(
        self,
        *,
        database: str | None,
        schema: str,
        table: str,
        indexes: tuple[tuple[str, str], ...],
    ) -> tuple[str, ...]:
        """Name indexes after their table, since SQLite index names span the whole file."""

        table_name: str | None = self.render_qualified_name(
            database=database, schema=schema, name=table
        )
        if table_name is None:
            return ()
        return tuple(
            f"CREATE INDEX IF NOT EXISTS {quote_identifier(f'{table_name}_{suffix}')} "
            f"ON {table_name} ({columns})"
            for suffix, columns in indexes
        )

    def _prune_history_sql(
        self, *, table: str | None, partition: str, order: str, retain_versions: int
    ) -> str:
        if table is None:
            return ""
        return (
            f"DELETE FROM {table} WHERE rowid IN ("
            "SELECT rowid FROM ("
            f"SELECT rowid, ROW_NUMBER() OVER (PARTITION BY {partition} ORDER BY {order}) "
            f"AS __sqlbuild_history_rank FROM {table}"
            ") AS __sqlbuild_ranked "
            f"WHERE __sqlbuild_history_rank > {retain_versions})"
        )


def _lowered(types: object) -> Mapping[str, str]:
    if not isinstance(types, Mapping):
        return MappingProxyType({})
    return MappingProxyType({str(name).lower(): str(kind) for name, kind in types.items()})


def _cursor_text(value: str | int | float | None) -> str | int | float | None:
    return canonical_timestamp_text(value) if isinstance(value, str) else value


def _sqb_hash(value: object) -> str | None:
    if value is None:
        return None
    return hashlib.md5(str(value).encode(), usedforsecurity=False).hexdigest()


def _seed_value(*, value: str | None, boolean: bool) -> object:
    if value is None or not boolean:
        return value
    lowered: str = value.strip().lower()
    if lowered in _TRUE_SEED_VALUES:
        return 1
    if lowered in _FALSE_SEED_VALUES:
        return 0
    return value
