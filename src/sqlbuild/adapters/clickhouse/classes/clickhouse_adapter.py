"""ClickHouse adapter implementation for a single server."""

from __future__ import annotations

import csv
import io
import json
import logging
import re
from collections.abc import Callable
from dataclasses import replace
from datetime import date, datetime
from decimal import Decimal
from pathlib import Path
from typing import Any, ClassVar, cast

from sqlbuild._native import (
    SeedCsvSettings,
    SourceFreshnessRecord,
    SqlLexicalSyntax,
    SqlValue,
    SqlValueKind,
    render_insert_source_freshness_records_sql,
)
from sqlbuild.adapter.contract.classes.audit_results import AuditResultsMixin
from sqlbuild.adapter.contract.classes.base_adapter import (
    BaseAdapter,
    _encode_typed_json,
    _render_typed_value_list,
    _typed_scalar_payload,
    _validate_rectangular_typed_array,
)
from sqlbuild.adapter.contract.classes.consumption import ConsumptionMixin
from sqlbuild.adapter.contract.classes.historical_snapshot_sql import render_is_distinct_from
from sqlbuild.adapter.contract.classes.microbatch import MicrobatchMixin
from sqlbuild.adapter.contract.classes.observations import ObservationsMixin
from sqlbuild.adapter.contract.classes.snapshot_sql import SnapshotSql
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.classes.unkeyed_diff import UnkeyedDiffMixin
from sqlbuild.adapter.contract.constants import DIFF_LEFT_SIDE, DIFF_RIGHT_SIDE
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.contract.main.normalize_seed_csv_value import normalize_seed_csv_value
from sqlbuild.adapter.contract.main.run_relation_read_probe import run_relation_read_probe
from sqlbuild.adapter.contract.main.same_view_definition import same_view_definition
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
from sqlbuild.adapters.clickhouse._helpers.connect_clickhouse import connect_clickhouse
from sqlbuild.adapters.clickhouse._helpers.output_columns import (
    alias_qualified_columns,
    key_identifiers,
)
from sqlbuild.adapters.clickhouse._helpers.partition_rewrite import (
    artifact_relation,
    generation_comment,
    rewrite_partitions,
)
from sqlbuild.adapters.clickhouse._helpers.relation_usage import read_relation_usage
from sqlbuild.adapters.clickhouse._helpers.snapshot_rewrite import (
    nullable_offset_windows,
    rewrite_snapshot_statements,
)
from sqlbuild.adapters.clickhouse._helpers.table_layout import render_table_layout
from sqlbuild.adapters.clickhouse._helpers.type_text import (
    canonical_type,
    is_nullable_type,
    nullable_type,
    quote_string,
)
from sqlbuild.adapters.clickhouse.classes.clickhouse_connection import _ClickHouseConnection
from sqlbuild.adapters.clickhouse.classes.partition_locks import PartitionLocks
from sqlbuild.adapters.clickhouse.classes.relation_write_locks import RelationWriteLocks
from sqlbuild.adapters.clickhouse.constants import (
    DEFAULT_COLUMN_DEFAULT_KIND,
    DEFAULT_DATABASE,
    EXCHANGE_UNSUPPORTED_CODES,
    EXPECTED_OUTPUT_COLUMNS_CONFIG_KEY,
    FRESH_GENERATION_CLAUSE,
    GENERATION_COMMENT_PREFIX,
    MAX_IDENTIFIER_LENGTH,
    MAX_QUERY_SIZE_BYTES,
    MODEL_NAME_CONFIG_KEY,
    NULL_IN_ORDINARY_COLUMN_CODE,
    NULL_IN_ORDINARY_COLUMN_SERVER_CODE,
    NULL_TARGET_COLUMN_PATTERN,
    NULLABLE_KEY_CODE,
    NULLABLE_KEY_SETTING,
    OUTPUT_COLUMN_NAME_CODE,
    PHYSICAL_STORAGE_CONFIG_KEY,
    RELATION_NOT_FOUND_CODES,
    RELATION_SEPARATOR,
    SNAPSHOT_REBUILD_INFIX,
    SNAPSHOT_WORK_INFIX,
    STATE_TABLE_ENGINE_KEYWORD,
    STATE_TIMESTAMP_TYPE,
    VIEW_ENGINES,
)
from sqlbuild.adapters.clickhouse.models import (
    PartitionRewrite,
    RewriteExecutor,
    SnapshotWorkspace,
)
from sqlbuild.compiler.compile.types import FunctionLanguage
from sqlbuild.diagnostics.main.log_sql import log_sql
from sqlbuild.spec.contracts.constants import DEFAULT_SEED_CSV_SETTINGS

_ERROR_CODE_PATTERN: re.Pattern[str] = re.compile(r"Code: (\d+)\.")
_BARE_BIGINT_PATTERN: re.Pattern[str] = re.compile(r"(\w+) BIGINT([,)])")
_NOT_NULL_COLUMN_PATTERN: re.Pattern[str] = re.compile(
    r"Nullable\(((?:[^()]|\([^()]*\))*)\) NOT NULL"
)


class ClickHouseAdapter(
    MicrobatchMixin,
    ConsumptionMixin,
    ObservationsMixin,
    AuditResultsMixin,
    UnkeyedDiffMixin,
    BaseAdapter,
):
    """ClickHouse adapter that builds tables aside and rewrites whole partitions."""

    adapter_name: ClassVar[str] = BuiltinAdapter.CLICKHOUSE.value
    sql_analysis_dialect_name: ClassVar[str | None] = "clickhouse"
    sql_lexical_syntax: ClassVar[SqlLexicalSyntax] = SqlLexicalSyntax(
        backslash_escape_quotes=frozenset({"'", '"', "`"}),
        line_comment_prefixes=frozenset({"--", "#", "//"}),
    )
    max_identifier_length: ClassVar[int] = MAX_IDENTIFIER_LENGTH
    relation_grants_supported: ClassVar[bool] = False
    state_timestamp_type: ClassVar[str | None] = STATE_TIMESTAMP_TYPE
    _snapshot_sql_dialect: ClassVar[SnapshotSqlDialect] = SnapshotSqlDialect(
        timestamp_type="Nullable(DateTime64(6, 'UTC'))",
        distinct_condition=render_is_distinct_from,
        update_style=SnapshotUpdateStyle.UPDATE_FROM,
        latest_version=SnapshotLatestVersionStyle.DERIVED_TABLE,
        historical_close=HistoricalSnapshotCloseStyle.UPDATE_FROM,
        historical_insert=HistoricalSnapshotInsertStyle.INSERT_WITH,
    )
    _partition_locks: ClassVar[PartitionLocks] = PartitionLocks()
    _relation_locks: ClassVar[RelationWriteLocks] = RelationWriteLocks()

    def connect(self, config: dict[str, Any]) -> _ClickHouseConnection:
        return connect_clickhouse(config=config)

    def _execute(self, *, connection: _ClickHouseConnection, sql: str) -> Any:
        log_sql(logger=logging.getLogger("sqlbuild.adapter.clickhouse"), sql=sql)
        try:
            return connection.execute(sql)
        except Exception as error:
            if _error_code(error) != NULL_IN_ORDINARY_COLUMN_SERVER_CODE:
                raise
            column: re.Match[str] | None = NULL_TARGET_COLUMN_PATTERN.search(str(error))
            raise AdapterUserError(
                message=(
                    "ClickHouse cannot store NULL in non-Nullable column "
                    f"'{column.group(1) if column else '?'}'"
                ),
                code=NULL_IN_ORDINARY_COLUMN_CODE,
                help=(
                    "The model produces NULL where the table's column is not Nullable. With "
                    "on_schema_change append_new_columns (the default) or sync_all_columns "
                    "SQLBuild makes the column Nullable; otherwise wrap the expression in "
                    "coalesce() or rebuild the table with --full-refresh."
                ),
            ) from error

    def close(self, connection: _ClickHouseConnection) -> None:
        connection.close()

    def supports_transactions(self) -> bool:
        return False

    def begin(self, connection: Any) -> None:
        del connection

    def commit(self, connection: Any) -> None:
        del connection

    def rollback(self, connection: Any) -> None:
        del connection

    def supports_transactional_ddl(self) -> bool:
        return False

    def interrupt_connection(self, connection: Any) -> bool:
        """Kill the connection's running queries by query ID from a side client."""

        return bool(connection.cancel_active_statements())

    def query(self, *, connection: Any, sql: str, limit: int | None) -> QueryResult:
        cursor: Any = self.execute(connection=connection, sql=sql)
        description: Any | None = getattr(cursor, "description", None)
        if description is None:
            return QueryResult()
        columns: tuple[str, ...] = tuple(str(column[0]) for column in description)
        column_types: tuple[str | None, ...] = tuple(
            None if column[1] is None else str(column[1]) for column in description
        )
        if limit is None:
            return QueryResult(
                columns=columns, rows=tuple(cursor.fetchall()), column_types=column_types
            )
        fetched: list[tuple[object, ...]] = cursor.fetchmany(limit + 1)
        return QueryResult(
            columns=columns,
            rows=tuple(fetched[:limit]),
            truncated=len(fetched) > limit,
            column_types=column_types,
        )

    def supports_zero_copy_clone(self) -> bool:
        return True

    def supports_durable_clone(self) -> bool:
        return True

    def supports_relation_age_metadata(self) -> bool:
        return False

    def supports_table_freshness_metadata(self) -> bool:
        return False

    def supports_python_functions(self) -> bool:
        return False

    def supports_table_functions(self) -> bool:
        return False

    def supports_concurrent_microbatch_dml(self) -> bool:
        """Batches rewrite partitions under per-partition locks, so disjoint windows run at once."""

        return True

    def max_statement_size(self) -> StatementSizeLimit | None:
        """The server's default ``max_query_size``; seeds never travel as SQL text."""

        return (MAX_QUERY_SIZE_BYTES, "bytes")

    def lists_relations_authoritatively(self, *, database: str | None, schema: str) -> bool:
        del database, schema
        return True

    def default_table_promotion_mode(self) -> TablePromotionMode:
        return TablePromotionMode.STAGED

    def default_promotion_strategy(self) -> PromotionStrategy:
        return PromotionStrategy.ATOMIC_SWAP

    def requires_derived_table_aliases(self) -> bool:
        return False

    def sql_analysis_dialect(self) -> str | None:
        return self.sql_analysis_dialect_name

    def expression_inference_profile(self) -> ExpressionInferenceProfile:
        return ExpressionInferenceProfile(sql_analysis_dialect=self.sql_analysis_dialect())

    def default_schema(self) -> str:
        return DEFAULT_DATABASE

    def default_database(self) -> str | None:
        return None

    def render_qualified_name(
        self, *, database: str | None, schema: str | None, name: str
    ) -> str | None:
        if database is not None:
            raise AdapterUserError(
                message=(
                    f"ClickHouse has no database level above schemas; got database '{database}'"
                ),
                code="A704",
                help="Remove 'database' from the target; a SQLBuild schema is a ClickHouse "
                "database.",
            )
        if schema is not None:
            return f"{schema}.{name}"
        return None

    def render_identifier(self, name: str) -> str:
        return '"' + name.replace("\\", "\\\\").replace('"', '\\"') + '"'

    def render_exact_identifier(self, name: str) -> str:
        return self.render_identifier(name)

    def star_exclude_keyword(self) -> str:
        return "EXCEPT"

    def relation_names_match(self, *, left: str, right: str) -> bool:
        return _unquote(left) == _unquote(right)

    def schema_exists(self, *, connection: Any, database: str | None, schema: str) -> bool:
        del database
        cursor: Any = self.execute(
            connection=connection,
            sql=f"SELECT 1 FROM system.databases WHERE name = {quote_string(schema)}",
        )
        return cursor.fetchone() is not None

    def relation_exists(
        self, *, connection: Any, database: str | None, schema: str | None, name: str
    ) -> bool:
        del database
        cursor: Any = self.execute(
            connection=connection,
            sql="SELECT 1 FROM system.tables WHERE "
            + _database_filter(schema)
            + f" AND name = {quote_string(name)}",
        )
        return cursor.fetchone() is not None

    def list_relations(
        self,
        *,
        connection: Any,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[RelationInfo, ...]:
        del database
        cursor: Any = self.execute(
            connection=connection,
            sql="SELECT database, name, engine FROM system.tables WHERE NOT is_temporary"
            + _in_filter(column="database", values=schemas)
            + _in_filter(column="name", values=names)
            + " ORDER BY database, name",
        )
        return tuple(
            RelationInfo(
                database=None,
                schema=str(row[0]),
                name=str(row[1]),
                relation_type="VIEW" if str(row[2]) in VIEW_ENGINES else "BASE TABLE",
            )
            for row in cursor.fetchall()
        )

    def list_functions(
        self,
        *,
        connection: Any,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[FunctionInfo, ...]:
        del database
        cursor: Any = self.execute(
            connection=connection,
            sql="SELECT name FROM system.functions WHERE origin = 'SQLUserDefined' ORDER BY name",
        )
        functions: list[FunctionInfo] = []
        for row in cursor.fetchall():
            schema, separator, name = str(row[0]).partition("__")
            if not separator:
                continue
            if schemas is not None and schema not in schemas:
                continue
            if names and name not in names:
                continue
            functions.append(
                FunctionInfo(database=None, schema=schema, name=name, function_type="FUNCTION")
            )
        return tuple(functions)

    def get_columns(
        self, *, connection: Any, database: str | None, schema: str | None, name: str
    ) -> tuple[ColumnInfo, ...]:
        del database
        cursor: Any = self.execute(
            connection=connection,
            sql="SELECT name, type FROM system.columns WHERE "
            + _database_filter(schema)
            + f" AND table = {quote_string(name)} ORDER BY position",
        )
        return tuple(ColumnInfo(name=str(row[0]), type=str(row[1])) for row in cursor.fetchall())

    def get_all_columns(
        self,
        *,
        connection: Any,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> dict[str, tuple[ColumnInfo, ...]]:
        del database
        cursor: Any = self.execute(
            connection=connection,
            sql="SELECT table, name, type FROM system.columns WHERE 1 = 1"
            + _in_filter(column="database", values=schemas)
            + _in_filter(column="table", values=names)
            + " ORDER BY table, position",
        )
        result: dict[str, list[ColumnInfo]] = {}
        for row in cursor.fetchall():
            result.setdefault(str(row[0]), []).append(
                ColumnInfo(name=str(row[1]), type=str(row[2]))
            )
        return {table: tuple(columns) for table, columns in result.items()}

    def describe_relation(self, *, connection: Any, relation: str) -> tuple[ColumnInfo, ...]:
        cursor: Any = self.execute(connection=connection, sql=f"DESCRIBE TABLE {relation}")
        return tuple(
            ColumnInfo(name=str(row[0]), type=str(row[1]))
            for row in cursor.fetchall()
            if not row[2] or str(row[2]) == DEFAULT_COLUMN_DEFAULT_KIND
        )

    def probe_relation_read(self, *, connection: Any, relation: str) -> RelationReadProbe:
        return run_relation_read_probe(
            execute=self.execute,
            connection=connection,
            relation=relation,
            classify_not_found=_classify_relation_not_found,
        )

    def physical_relation_generation(
        self, *, connection: Any, database: str | None, schema: str | None, name: str
    ) -> str | None:
        """The table UUID, kept in the comment of copies a partition rewrite exchanges in."""

        del database
        row: Any = self.execute(
            connection=connection,
            sql=_table_generation_query(schema=schema, name=name),
        ).fetchone()
        if row is None:
            return None
        return generation_comment(tuple(row)).removeprefix(GENERATION_COMMENT_PREFIX)

    def estimate_relation_row_count(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str | None,
        name: str,
        metadata_only: bool = False,
    ) -> RelationRowCountEstimate:
        del database
        row: Any = self.execute(
            connection=connection,
            sql=(
                "SELECT engine, total_rows, total_bytes FROM system.tables WHERE "
                if metadata_only
                else "SELECT engine, total_rows FROM system.tables WHERE "
            )
            + _database_filter(schema)
            + f" AND name = {quote_string(name)}",
        ).fetchone()
        if row is None:
            return RelationRowCountEstimate(row_count=None, detail="relation not found")
        byte_count: int | None = int(row[2]) if metadata_only and row[2] is not None else None
        return RelationRowCountEstimate(
            row_count=int(row[1]) if row[1] is not None else None,
            detail=f"engine {row[0]} does not report row counts" if row[1] is None else None,
            byte_count=byte_count,
            byte_count_basis="system_tables_total_bytes" if byte_count is not None else None,
        )

    def read_view_definition(
        self, *, connection: Any, database: str | None, schema: str, name: str
    ) -> str | None:
        del database
        row: Any = self.execute(
            connection=connection,
            sql="SELECT as_select FROM system.tables WHERE "
            + _database_filter(schema)
            + f" AND name = {quote_string(name)} AND engine = 'View'",
        ).fetchone()
        return None if row is None else str(row[0])

    def column_rename_unavailable_reason(self, *, connection: Any, destination: str) -> str | None:
        del connection, destination
        return None

    def render_framework_type(self, type_name: FrameworkType) -> str:
        match type_name:
            case FrameworkType.STRING:
                return "Nullable(String)"
            case FrameworkType.INTEGER:
                return "Nullable(Int64)"
            case FrameworkType.TIMESTAMP:
                return "Nullable(DateTime64(6, 'UTC'))"

    def render_loader_logical_type(
        self, *, type_name: LoaderLogicalType, precision: int = 38, scale: int = 0
    ) -> str:
        match type_name:
            case LoaderLogicalType.BOOLEAN:
                return "Nullable(Bool)"
            case LoaderLogicalType.INTEGER:
                return "Nullable(Int64)"
            case LoaderLogicalType.FLOAT:
                return "Nullable(Float64)"
            case LoaderLogicalType.DECIMAL:
                return f"Nullable(Decimal({precision}, {scale}))"
            case LoaderLogicalType.STRING:
                return "Nullable(String)"
            case LoaderLogicalType.TIMESTAMP:
                return "Nullable(DateTime64(6, 'UTC'))"
            case LoaderLogicalType.DATE:
                return "Nullable(Date32)"
            case LoaderLogicalType.JSON:
                return "Nullable(String)"

    def render_typed_scalar(self, *, value: SqlValue) -> str:
        payload: object = _typed_scalar_payload(value)
        match value.kind:
            case SqlValueKind.STRING:
                return quote_string(str(payload))
            case SqlValueKind.BOOLEAN:
                return "true" if payload else "false"
            case SqlValueKind.NULL:
                return "NULL"
            case SqlValueKind.DECIMAL:
                return f"toDecimal256({quote_string(str(payload))}, {_decimal_scale(payload)})"
            case _:
                return str(payload)

    def render_typed_array(self, *, value: SqlValue) -> str:
        _validate_rectangular_typed_array(value=value, adapter_name=self.adapter_name)
        return "[" + self._render_typed_array_items(value) + "]"

    def render_typed_object(self, *, value: SqlValue) -> str:
        return f"CAST({quote_string(_encode_typed_json(value))} AS JSON)"

    def render_loader_value_literal(
        self, *, value: object, logical_type: LoaderLogicalType | None
    ) -> str:
        if value is None:
            return "NULL"
        if logical_type == LoaderLogicalType.JSON:
            return quote_string(json.dumps(value, sort_keys=True))
        if isinstance(value, bool):
            return "true" if value else "false"
        if isinstance(value, int | float | Decimal):
            return str(value)
        if isinstance(value, datetime | date):
            return quote_string(value.isoformat())
        if isinstance(value, dict | list):
            return quote_string(json.dumps(value, sort_keys=True))
        return quote_string(str(value))

    def render_loader_rows_select(
        self,
        *,
        rows: tuple[dict[str, object], ...],
        column_names: tuple[str, ...],
        column_sql_types: dict[str, str],
        inferred_types: dict[str, LoaderLogicalType],
    ) -> str:
        """Render rows as a UNION ALL of selects; ClickHouse has no aliased VALUES list."""

        if not rows:
            return (
                "SELECT "
                + ", ".join(
                    f"CAST(NULL AS {nullable_type(column_sql_types.get(name, 'String'))}) AS "
                    f"{self.render_identifier(name)}"
                    for name in column_names
                )
                + " WHERE 1 = 0"
            )
        return " UNION ALL ".join(
            self._loader_row_select(
                row=row,
                column_names=column_names,
                column_sql_types=column_sql_types,
                inferred_types=inferred_types,
            )
            for row in rows
        )

    def _loader_row_select(
        self,
        *,
        row: dict[str, object],
        column_names: tuple[str, ...],
        column_sql_types: dict[str, str],
        inferred_types: dict[str, LoaderLogicalType],
    ) -> str:
        projections: list[str] = []
        name: str
        for name in column_names:
            value_sql: str = self.render_loader_value_literal(
                value=row.get(name), logical_type=inferred_types.get(name)
            )
            sql_type: str | None = column_sql_types.get(name)
            rendered: str = (
                value_sql if sql_type is None else f"CAST({value_sql} AS {nullable_type(sql_type)})"
            )
            projections.append(f"{rendered} AS {self.render_identifier(name)}")
        return "SELECT " + ", ".join(projections)

    def render_source_expression_cast(
        self, *, expression: str, target_type: str, alias: str
    ) -> str:
        return f"CAST({expression} AS {nullable_type(target_type)}) AS {alias}"

    def render_cursor_bound_literal(self, *, value: str, cursor_type: str | None) -> str:
        if cursor_type == CursorKind.INTEGER:
            return value
        if cursor_type == CursorKind.TIMESTAMP:
            return f"parseDateTime64BestEffort({quote_string(value)}, 6, 'UTC')"
        return quote_string(value)

    def build_cursor_filter(
        self,
        *,
        cursor_column: str | None,
        start_cursor: CursorValue | None,
        end_cursor: CursorValue | None,
    ) -> str:
        if cursor_column is None or start_cursor is None:
            return ""
        clauses: list[str] = [f"{cursor_column} >= {_cursor_value_literal(start_cursor)}"]
        if end_cursor is not None:
            clauses.append(f"{cursor_column} < {_cursor_value_literal(end_cursor)}")
        return " AND ".join(clauses)

    def render_max_cursor_at_or_before(
        self,
        *,
        relation: str,
        cursor_column: str,
        maximum_allowed: str,
        cursor_type: str | None,
        is_date: bool,
    ) -> str:
        quoted_cursor: str = self.render_identifier(cursor_column)
        maximum_literal: str = (
            f"toDate32({quote_string(maximum_allowed)})"
            if is_date
            else self.render_cursor_bound_literal(value=maximum_allowed, cursor_type=cursor_type)
        )
        return (
            f"SELECT max({quoted_cursor}) FROM {relation} "
            f"WHERE {quoted_cursor} <= {maximum_literal}"
        )

    def render_current_timestamp(self) -> str:
        return "now64(6, 'UTC')"

    def render_current_utc_timestamp(self) -> str:
        return "now64(6, 'UTC')"

    def normalize_row_diff_numeric_type(self, column_type: str) -> str | None:
        return normalize_numeric_family(type_sql=canonical_type(column_type), dialect=None)

    def format_row_diff_decimal_sql(self, value: Decimal) -> str:
        return f"toDecimal256({quote_string(format(value, 'f'))}, {_decimal_scale(value)})"

    def _render_row_diff_key_hash(self, *, keys: tuple[str, ...], alias: str, seed: int) -> str:
        key_values: str = ", ".join(f"{alias}.{key}" for key in keys)
        return f"sipHash64({seed}, {key_values})"

    def render_create_schema(self, *, database: str | None, schema: str) -> tuple[str, ...]:
        del database
        return (f"CREATE DATABASE IF NOT EXISTS {schema}",)

    def render_drop_schema(self, *, database: str | None, schema: str) -> tuple[str, ...]:
        del database
        return (f"DROP DATABASE IF EXISTS {schema}",)

    def ensure_schema(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str | None,
        statement_recorder: StatementRecorder,
    ) -> None:
        if schema is None:
            return
        statements: tuple[str, ...] = self.render_create_schema(database=database, schema=schema)
        statement_recorder.record_many(statements)
        for statement in statements:
            self.execute(connection=connection, sql=statement)

    def render_create_table_as(self, *, destination: str, sql: str) -> tuple[str, ...]:
        return self._render_create_table_with_layout(destination=destination, sql=sql, layout={})

    def _render_create_table_with_layout(
        self, *, destination: str, sql: str, layout: dict[str, Any]
    ) -> tuple[str, ...]:
        return (
            f"DROP TABLE IF EXISTS {destination}",
            f"CREATE TABLE {destination} {render_table_layout(layout)} AS {sql}",
        )

    def render_create_view_as(self, *, destination: str, sql: str) -> tuple[str, ...]:
        return (f"CREATE OR REPLACE VIEW {destination} AS {sql}",)

    def render_replace_view_keeping_grants(
        self, *, destination: str, sql: str
    ) -> tuple[str, ...] | None:
        return self.render_create_view_as(destination=destination, sql=sql)

    def render_drop(self, *, destination: str, if_exists: bool = True) -> tuple[str, ...]:
        exists_clause: str = " IF EXISTS" if if_exists else ""
        return (f"DROP TABLE{exists_clause} {destination}",)

    def render_drop_view(self, *, destination: str, if_exists: bool = True) -> tuple[str, ...]:
        exists_clause: str = " IF EXISTS" if if_exists else ""
        return (f"DROP VIEW{exists_clause} {destination}",)

    def render_rename(self, *, origin: str, destination: str) -> tuple[str, ...]:
        return (
            f"RENAME TABLE {origin} TO {_qualify_like(destination=destination, origin=origin)}",
        )

    def render_rename_view(self, *, origin: str, destination: str) -> tuple[str, ...]:
        return self.render_rename(origin=origin, destination=destination)

    def render_swap(self, *, left: str, right: str) -> tuple[str, ...]:
        """Exchange two tables atomically; this needs an Atomic database engine."""

        return (f"EXCHANGE TABLES {left} AND {right}",)

    def swap(
        self, *, connection: Any, left: str, right: str, statement_recorder: StatementRecorder
    ) -> None:
        statements: tuple[str, ...] = self.render_swap(left=left, right=right)
        statement_recorder.record_many(statements)
        for statement in statements:
            try:
                self.execute(connection=connection, sql=statement)
            except Exception as error:
                if _error_code(error) in EXCHANGE_UNSUPPORTED_CODES:
                    raise AdapterUserError(
                        message=f"ClickHouse cannot exchange {left} and {right}: {error}",
                        code="A705",
                        help="Atomic promotion needs databases with the Atomic engine (the "
                        "default since ClickHouse 20.10).",
                    ) from error
                raise

    def render_replace_table_from_relation(
        self, *, destination: str, origin: str
    ) -> tuple[str, ...]:
        return (
            f"DROP TABLE IF EXISTS {destination}",
            f"CREATE TABLE {destination} AS {origin}{FRESH_GENERATION_CLAUSE}",
            f"INSERT INTO {destination} SELECT * FROM {origin}",
        )

    def render_clone(
        self,
        *,
        origin: str,
        destination: str,
        hard_copy: bool = False,
        origin_is_transient: bool = False,
    ) -> tuple[str, ...]:
        """Clone by hard-linking every part; the clone keeps the engine, keys and settings."""

        del origin_is_transient
        if hard_copy:
            return self.render_replace_table_from_relation(destination=destination, origin=origin)
        return (
            f"DROP TABLE IF EXISTS {destination}",
            f"CREATE TABLE {destination} AS {origin}{FRESH_GENERATION_CLAUSE}",
            f"ALTER TABLE {destination} ATTACH PARTITION ALL FROM {origin}",
        )

    def render_durable_clone(
        self, *, origin: str, destination: str, origin_is_transient: bool = False
    ) -> tuple[str, ...]:
        return self.render_clone(
            origin=origin, destination=destination, origin_is_transient=origin_is_transient
        )

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
            statements=(
                f"CREATE TABLE {stage} AS {origin}{FRESH_GENERATION_CLAUSE}",
                f"INSERT INTO {stage} SELECT * FROM {origin}",
            ),
        )

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
        """Move with RENAME TABLE, which also crosses databases; copy otherwise."""

        statements: tuple[str, ...]
        if remove_origin:
            statements = self.render_rename(origin=origin, destination=destination)
        else:
            if not allow_copy_fallback:
                raise AdapterUserError(
                    message="ClickHouse relation copy requires --allow-copy", code="A706"
                )
            statements = self.render_replace_table_from_relation(
                destination=destination, origin=origin
            )
        statement_recorder.record_many(statements)
        for statement in statements:
            self.execute(connection=connection, sql=statement)

    def render_add_columns(
        self, *, destination: str, columns: tuple[ColumnInfo, ...]
    ) -> tuple[str, ...]:
        return tuple(
            f"ALTER TABLE {destination} ADD COLUMN {self.render_identifier(column.name)} "
            f"{nullable_type(column.type)}"
            for column in columns
        )

    def render_drop_columns(
        self, *, destination: str, column_names: tuple[str, ...]
    ) -> tuple[str, ...]:
        return tuple(
            f"ALTER TABLE {destination} DROP COLUMN {self.render_identifier(name)}"
            for name in column_names
        )

    def render_alter_column_types(
        self, *, destination: str, columns: tuple[ColumnInfo, ...]
    ) -> tuple[str, ...]:
        return tuple(
            f"ALTER TABLE {destination} MODIFY COLUMN {self.render_identifier(column.name)} "
            f"{column.type}"
            for column in columns
        )

    def render_rename_column(
        self, *, destination: str, old_name: str, new_name: str
    ) -> tuple[str, ...]:
        return (
            f"ALTER TABLE {destination} RENAME COLUMN {self.render_identifier(old_name)} "
            f"TO {self.render_identifier(new_name)}",
        )

    def render_udf_call(self, *, target: str, call_suffix_sql: str) -> str:
        return f"{_mangled_function_name(target)}{call_suffix_sql}"

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
        """Create a server-global SQL lambda named ``<schema>__<function>``."""

        del runtime_version, entry_point, packages
        if language != FunctionLanguage.SQL:
            raise AdapterUserError(
                message="ClickHouse does not deploy Python functions; executable UDFs need "
                "server configuration",
                code="A707",
                help="Install the function on the server and declare it as an external function.",
            )
        if return_columns:
            raise AdapterUserError(
                message="ClickHouse does not support SQL table functions", code="A708"
            )
        argument_names: str = ", ".join(str(argument.name) for argument in arguments)
        body: str = body_sql.strip().removesuffix(";").strip()
        if body.upper().startswith("SELECT "):
            body = body[len("SELECT ") :].strip()
        cast_body: str = f"CAST(({body}) AS {nullable_type(returns)})" if returns else f"({body})"
        return (
            f"CREATE OR REPLACE FUNCTION {_mangled_function_name(destination)} "
            f"AS ({argument_names}) -> {cast_body}",
        )

    def render_append(
        self, *, destination: str, sql: str, columns: tuple[str, ...] | None = None
    ) -> tuple[str, ...]:
        if columns is None:
            return (f"INSERT INTO {destination} {sql}",)
        column_list: str = ", ".join(self.render_identifier(column) for column in columns)
        return (f"INSERT INTO {destination} ({column_list}) {sql}",)

    def render_delete_insert(
        self,
        *,
        destination: str,
        sql: str,
        unique_key: tuple[str, ...],
        columns: tuple[str, ...] | None = None,
    ) -> tuple[str, ...]:
        request: PartitionRewrite = self._key_rewrite(
            destination=destination, sql=sql, unique_key=unique_key, columns=columns
        )
        return (
            request.delta_insert_sql,
            f"-- rewrite partitions where {request.displaced_predicate}",
        )

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
        unique_key: tuple[str, ...] = (),
    ) -> tuple[str, ...]:
        request: PartitionRewrite = self._window_rewrite(
            destination=destination,
            sql=sql,
            cursor_column=cursor_column,
            cursor_start=cursor_start,
            cursor_end=cursor_end,
            columns=columns,
            cursor_type=cursor_type,
            unique_key=unique_key,
        )
        return (
            request.delta_insert_sql,
            f"-- rewrite partitions where {request.displaced_predicate}",
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
        request: PartitionRewrite = self._merge_rewrite(
            destination=destination,
            sql=sql,
            unique_key=unique_key,
            source_columns=source_columns,
            exclude_columns=exclude_columns,
        )
        return (
            request.delta_insert_sql,
            f"-- rewrite partitions where {request.displaced_predicate}",
        )

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
        return self._rewrite(
            connection=connection,
            request=self._key_rewrite(
                destination=destination, sql=sql, unique_key=keys, columns=columns
            ),
            statement_recorder=statement_recorder,
        )

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
        unique_key: tuple[str, ...] = (),
    ) -> int | None:
        return self._rewrite(
            connection=connection,
            request=self._window_rewrite(
                destination=destination,
                sql=sql,
                cursor_column=cursor_column,
                cursor_start=cursor_start,
                cursor_end=cursor_end,
                columns=columns,
                cursor_type=cursor_type,
                unique_key=unique_key,
            ),
            statement_recorder=statement_recorder,
        )

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
        return self._rewrite(
            connection=connection,
            request=self._merge_rewrite(
                destination=destination,
                sql=sql,
                unique_key=keys,
                source_columns=source_columns,
                exclude_columns=exclude_columns,
            ),
            statement_recorder=statement_recorder,
        )

    def _rewrite(
        self, *, connection: Any, request: PartitionRewrite, statement_recorder: StatementRecorder
    ) -> int:
        return rewrite_partitions(
            request=request,
            executor=RewriteExecutor(
                execute=lambda sql: self.execute(connection=connection, sql=sql),
                statement_recorder=statement_recorder,
                partition_locks=self._partition_locks,
                relation_locks=self._relation_locks,
                generation_query=_generation_query(request.destination),
            ),
        )

    def _key_rewrite(
        self,
        *,
        destination: str,
        sql: str,
        unique_key: tuple[str, ...],
        columns: tuple[str, ...] | None,
    ) -> PartitionRewrite:
        key_tuple: str = _key_tuple(adapter=self, keys=unique_key)
        return PartitionRewrite(
            destination=destination,
            delta_insert_sql=self.render_append(destination="{delta}", sql=sql, columns=columns)[0],
            displaced_predicate=f"{key_tuple} IN (SELECT {key_tuple} FROM {{delta}})",
        )

    def _window_rewrite(
        self,
        *,
        destination: str,
        sql: str,
        cursor_column: str,
        cursor_start: str,
        cursor_end: str,
        columns: tuple[str, ...] | None,
        cursor_type: str | None,
        unique_key: tuple[str, ...],
    ) -> PartitionRewrite:
        quoted: str = self.render_identifier(cursor_column)
        start: str = self.render_cursor_bound_literal(value=cursor_start, cursor_type=cursor_type)
        end: str = self.render_cursor_bound_literal(value=cursor_end, cursor_type=cursor_type)
        displaced: str = f"({quoted} >= {start} AND {quoted} < {end})"
        if unique_key:
            key_tuple: str = _key_tuple(adapter=self, keys=unique_key)
            displaced = f"({displaced} OR {key_tuple} IN (SELECT {key_tuple} FROM {{delta}}))"
        return PartitionRewrite(
            destination=destination,
            delta_insert_sql=self.render_append(destination="{delta}", sql=sql, columns=columns)[0],
            displaced_predicate=displaced,
        )

    def _merge_rewrite(
        self,
        *,
        destination: str,
        sql: str,
        unique_key: tuple[str, ...],
        source_columns: tuple[str, ...],
        exclude_columns: tuple[str, ...],
    ) -> PartitionRewrite:
        """Upsert: incoming rows win, except excluded columns keep matched target values."""

        excluded: frozenset[str] = frozenset(
            column.lower() for column in exclude_columns
        ) - frozenset(key.lower() for key in unique_key)
        key_tuple: str = _key_tuple(adapter=self, keys=unique_key)
        projections: list[str] = []
        column: str
        for column in source_columns:
            quoted: str = self.render_identifier(column)
            if column.lower() in excluded:
                projections.append(
                    f"if(__target.__sqb_matched = 1, __target.{quoted}, __source.{quoted})"
                )
            else:
                projections.append(f"__source.{quoted}")
        column_list: str = ", ".join(self.render_identifier(column) for column in source_columns)
        source_sql: str = f"SELECT * FROM ({sql}) AS __source"
        if excluded:
            kept: str = ", ".join(
                self.render_identifier(column)
                for column in source_columns
                if column.lower() in excluded
            )
            key_list: str = ", ".join(self.render_identifier(key) for key in unique_key)
            join_condition: str = " AND ".join(
                f"__source.{self.render_identifier(key)} = __target.{self.render_identifier(key)}"
                for key in unique_key
            )
            source_sql = (
                f"SELECT {', '.join(projections)} FROM ({sql}) AS __source "
                f"LEFT JOIN (SELECT {key_list}, {kept}, 1 AS __sqb_matched FROM {destination}) "
                f"AS __target ON {join_condition}"
            )
        return PartitionRewrite(
            destination=destination,
            delta_insert_sql=f"INSERT INTO {{delta}} ({column_list}) {source_sql}",
            displaced_predicate=f"{key_tuple} IN (SELECT {key_tuple} FROM {{delta}})",
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
        return nullable_offset_windows(
            super().render_create_initial_historical_timestamp_snapshot_destination(
                table_type=table_type,
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
        )

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
        return nullable_offset_windows(
            super().render_create_initial_historical_timestamp_changes_destination(
                table_type=table_type,
                destination=destination,
                origin=origin,
                unique_key=unique_key,
                updated_at_column=updated_at_column,
                valid_from_column=valid_from_column,
                valid_to_column=valid_to_column,
                output_columns=output_columns,
            )
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
        return nullable_offset_windows(
            super().render_create_initial_historical_check_snapshot_destination(
                table_type=table_type,
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
        return self._staged_snapshot(
            destination=destination,
            valid_to_column=valid_to_column,
            render=lambda work: super(
                ClickHouseAdapter, self
            ).render_apply_timestamp_snapshot_changes(
                destination=work,
                origin=origin,
                unique_key=unique_key,
                updated_at_column=updated_at_column,
                observed_at_column=observed_at_column,
                valid_from_column=valid_from_column,
                valid_to_column=valid_to_column,
                initial_valid_from=initial_valid_from,
                output_columns=output_columns,
                invalidate_hard_deletes=invalidate_hard_deletes,
            ),
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
        return self._staged_snapshot(
            destination=target.destination,
            valid_to_column=target.valid_to_column,
            render=lambda work: super(ClickHouseAdapter, self).render_apply_check_snapshot_changes(
                target=replace(target, destination=work),
                check_columns=check_columns,
                updated_at_column=updated_at_column,
                observed_at_column=observed_at_column,
                initial_valid_from=initial_valid_from,
                invalidate_hard_deletes=invalidate_hard_deletes,
            ),
        )

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
        return self._staged_snapshot(
            destination=destination,
            valid_to_column=valid_to_column,
            render=lambda work: super(
                ClickHouseAdapter, self
            ).render_apply_historical_timestamp_snapshot_changes(
                destination=work,
                origin=origin,
                unique_key=unique_key,
                updated_at_column=updated_at_column,
                observed_at_column=observed_at_column,
                valid_from_column=valid_from_column,
                valid_to_column=valid_to_column,
                output_columns=output_columns,
                invalidate_hard_deletes=invalidate_hard_deletes,
            ),
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
        return self._staged_snapshot(
            destination=destination,
            valid_to_column=valid_to_column,
            render=lambda work: super(
                ClickHouseAdapter, self
            ).render_apply_historical_timestamp_changes(
                destination=work,
                origin=origin,
                unique_key=unique_key,
                updated_at_column=updated_at_column,
                valid_from_column=valid_from_column,
                valid_to_column=valid_to_column,
                output_columns=output_columns,
            ),
        )

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
        return self._staged_snapshot(
            destination=destination,
            valid_to_column=valid_to_column,
            render=lambda work: super(
                ClickHouseAdapter, self
            ).render_apply_historical_check_snapshot_changes(
                destination=work,
                origin=origin,
                unique_key=unique_key,
                check_columns=check_columns,
                observed_at_column=observed_at_column,
                valid_from_column=valid_from_column,
                valid_to_column=valid_to_column,
                output_columns=output_columns,
                invalidate_hard_deletes=invalidate_hard_deletes,
            ),
        )

    def _staged_snapshot(
        self,
        *,
        destination: str,
        valid_to_column: str,
        render: Callable[[str], tuple[str, ...]],
    ) -> tuple[str, ...]:
        workspace: SnapshotWorkspace = SnapshotWorkspace(
            destination=destination,
            work=artifact_relation(relation=destination, infix=SNAPSHOT_WORK_INFIX, digest=""),
            rebuild=artifact_relation(
                relation=destination, infix=SNAPSHOT_REBUILD_INFIX, digest=""
            ),
            valid_to_column=valid_to_column,
        )
        return rewrite_snapshot_statements(workspace=workspace, statements=render(workspace.work))

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
        """Create the seed table and stream its rows as CSV, never as SQL text."""

        del infer_types
        if replace:
            self.drop(
                connection=connection,
                destination=destination,
                if_exists=True,
                statement_recorder=statement_recorder,
            )
        column_definitions: str = ", ".join(
            f"{self.render_identifier(column.name)} {nullable_type(column.type)}"
            for column in columns
        )
        create_sql: str = (
            f"CREATE TABLE {destination} ({column_definitions}) ENGINE = MergeTree ORDER BY tuple()"
        )
        statement_recorder.record(create_sql)
        self.execute(connection=connection, sql=create_sql)
        column_names: tuple[str, ...] = tuple(column.name for column in columns)
        buffer: io.StringIO = io.StringIO()
        writer: Any = csv.writer(buffer, lineterminator="\n", quoting=csv.QUOTE_MINIMAL)
        row_count: int = 0
        with file_path.open("r", encoding=csv_settings.encoding or "utf-8", newline="") as seed:
            reader: csv.DictReader[str] = csv.DictReader(
                seed,
                delimiter=csv_settings.delimiter or ",",
                quotechar=csv_settings.quotechar or '"',
                escapechar=csv_settings.escapechar,
                doublequote=True if csv_settings.doublequote is None else csv_settings.doublequote,
                skipinitialspace=bool(csv_settings.skipinitialspace),
            )
            for row in reader:
                values: list[str] = []
                for name in column_names:
                    value: object = normalize_seed_csv_value(
                        value=row.get(name), column_name=name, csv_settings=csv_settings
                    )
                    values.append("\\N" if value is None else str(value))
                writer.writerow(values)
                row_count += 1
        if row_count == 0:
            return
        statement_recorder.record(f"INSERT INTO {destination} FORMAT CSV")
        connection.insert_csv(
            table=destination,
            column_names=column_names,
            payload=buffer.getvalue().encode("utf-8"),
        )

    def diff_schema(self, *, connection: Any, left: str, right: str) -> SchemaDiffResult:
        left_columns: tuple[ColumnInfo, ...] = self.describe_relation(
            connection=connection, relation=left
        )
        right_columns: tuple[ColumnInfo, ...] = self.describe_relation(
            connection=connection, relation=right
        )
        left_map: dict[str, str] = {column.name: column.type for column in left_columns}
        right_map: dict[str, str] = {column.name: column.type for column in right_columns}
        added: tuple[ColumnInfo, ...] = tuple(
            ColumnInfo(name=name, type=type_sql)
            for name, type_sql in right_map.items()
            if name not in left_map
        )
        removed: tuple[ColumnInfo, ...] = tuple(
            ColumnInfo(name=name, type=type_sql)
            for name, type_sql in left_map.items()
            if name not in right_map
        )
        changed: tuple[tuple[ColumnInfo, ColumnInfo], ...] = tuple(
            (ColumnInfo(name=name, type=left_map[name]), ColumnInfo(name=name, type=type_sql))
            for name, type_sql in right_map.items()
            if name in left_map
            and (
                not types_equal(
                    left=canonical_type(left_map[name]),
                    right=canonical_type(type_sql),
                    dialect=None,
                )
                or is_nullable_type(left_map[name]) != is_nullable_type(type_sql)
            )
        )
        return SchemaDiffResult(
            added_columns=added,
            removed_columns=removed,
            type_changed_columns=changed,
            left_column_count=len(left_columns),
            right_column_count=len(right_columns),
        )

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
        null_count_sql: str = (
            f"SELECT COUNT(*) FROM ({relation_sql}) AS __key_check WHERE {null_condition}"
        )
        null_row: tuple[Any, ...] = self.execute(
            connection=connection, sql=null_count_sql
        ).fetchone()
        if int(null_row[0]) > 0:
            raise AdapterUserError(
                message=f"row diff {relation_label} relation contains null unique_key values"
            )

        key_list: str = ", ".join(keys)
        duplicate_count_sql: str = (
            f"SELECT COUNT(*) FROM ("
            f"SELECT {key_list} FROM ({relation_sql}) AS __key_check "
            f"GROUP BY {key_list} HAVING COUNT(*) > 1"
            f") AS __duplicates"
        )
        duplicate_row: tuple[Any, ...] = self.execute(
            connection=connection, sql=duplicate_count_sql
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
                f"GREATEST(ABS({left_expression}), ABS({right_expression}))"
            )
        threshold_sql: str = threshold_parts[0]
        if len(threshold_parts) > 1:
            threshold_sql = f"GREATEST({', '.join(threshold_parts)})"
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
            self.validate_row_diff_tolerance(
                column=column,
                tolerance=column_tolerance,
            )
            return column_tolerance
        normalized_type: str | None = self.normalize_row_diff_numeric_type(column_type)
        if normalized_type is None:
            return None
        type_tolerance: RowDiffTolerance | None = tolerances.by_type.get(normalized_type)
        if type_tolerance is not None:
            self.validate_row_diff_tolerance(
                column=column,
                tolerance=type_tolerance,
            )
        return type_tolerance

    def validate_row_diff_tolerance(self, *, column: str, tolerance: RowDiffTolerance) -> None:
        if tolerance.absolute is None and tolerance.relative is None:
            raise AdapterUserError(
                message=f"row diff tolerance for column '{column}' must define absolute or relative"
            )

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
        for row in rows:
            sample: list[tuple[str, object]] = []
            for index, key in enumerate(keys):
                sample.append((key, row[index]))
            samples.append(tuple(sample))
        return tuple(samples)

    def render_create_fingerprint_table_sql(self, *, database: str | None, schema: str) -> str:
        return _state_table_ddl(
            super().render_create_fingerprint_table_sql(database=database, schema=schema)
        )

    def render_create_source_freshness_table_sql(self, *, database: str | None, schema: str) -> str:
        return _state_table_ddl(
            super().render_create_source_freshness_table_sql(database=database, schema=schema)
        )

    def render_create_consumption_table_sql(self, *, database: str | None, schema: str) -> str:
        return _state_table_ddl(
            super().render_create_consumption_table_sql(database=database, schema=schema)
        )

    def render_create_observations_table_sql(self, *, database: str | None, schema: str) -> str:
        return _state_table_ddl(
            super().render_create_observations_table_sql(database=database, schema=schema)
        )

    def render_create_node_result_table_sql(self, *, database: str | None, schema: str) -> str:
        return _state_table_ddl(
            super().render_create_node_result_table_sql(database=database, schema=schema)
        )

    def render_create_audit_result_table_sql(self, *, database: str | None, schema: str) -> str:
        return _state_table_ddl(
            super().render_create_audit_result_table_sql(database=database, schema=schema)
        )

    def render_create_janitor_event_table_sql(self, *, database: str | None, schema: str) -> str:
        return _state_table_ddl(
            super().render_create_janitor_event_table_sql(database=database, schema=schema)
        )

    def render_create_migration_state_table_sql(self, *, database: str | None, schema: str) -> str:
        return _state_table_ddl(
            super().render_create_migration_state_table_sql(database=database, schema=schema)
        )

    def render_create_old_name_view_state_table_sql(
        self, *, database: str | None, schema: str
    ) -> str:
        return _state_table_ddl(
            super().render_create_old_name_view_state_table_sql(database=database, schema=schema)
        )

    def render_create_column_migration_state_table_sql(
        self, *, database: str | None, schema: str
    ) -> str:
        return _state_table_ddl(
            super().render_create_column_migration_state_table_sql(database=database, schema=schema)
        )

    def render_create_microbatch_state_table_sql(self, *, database: str | None, schema: str) -> str:
        return _state_table_ddl(
            super().render_create_microbatch_state_table_sql(database=database, schema=schema)
        )

    def render_create_fingerprint_index_sqls(
        self, *, database: str | None, schema: str
    ) -> tuple[str, ...]:
        del database, schema
        return ()

    def render_create_source_freshness_index_sqls(
        self, *, database: str | None, schema: str
    ) -> tuple[str, ...]:
        del database, schema
        return ()

    def render_create_node_result_index_sqls(
        self, *, database: str | None, schema: str
    ) -> tuple[str, ...]:
        del database, schema
        return ()

    def render_create_audit_result_index_sqls(
        self, *, database: str | None, schema: str
    ) -> tuple[str, ...]:
        del database, schema
        return ()

    def render_prune_fingerprint_history_sql(
        self, *, database: str | None, schema: str, retain_versions: int
    ) -> str:
        from sqlbuild._native import FINGERPRINT_TABLE_NAME

        return _prune_history_sql(
            table=f"{schema}.{FINGERPRINT_TABLE_NAME}",
            partition_columns=("node_type", "node_name"),
            order_columns=("ts", "run_id"),
            retain_versions=retain_versions,
        )

    def render_prune_source_freshness_history_sql(
        self, *, database: str | None, schema: str, retain_versions: int
    ) -> str:
        from sqlbuild._native import SOURCE_FRESHNESS_TABLE_NAME

        return _prune_history_sql(
            table=f"{schema}.{SOURCE_FRESHNESS_TABLE_NAME}",
            partition_columns=("source_name", "target_database", "target_schema", "target_name"),
            order_columns=("observed_at", "run_id"),
            retain_versions=retain_versions,
        )

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
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

    def widen_target_nullability(
        self,
        *,
        connection: Any,
        destination: str,
        target_columns: tuple[ColumnInfo, ...],
        incoming_columns: tuple[ColumnInfo, ...],
        statement_recorder: StatementRecorder,
    ) -> tuple[ColumnInfo, ...]:
        """Make target columns ``Nullable`` where the incoming column of the same type is."""

        incoming: dict[str, str] = {column.name.lower(): column.type for column in incoming_columns}
        widened: tuple[ColumnInfo, ...] = tuple(
            ColumnInfo(name=column.name, type=nullable_type(column.type))
            for column in target_columns
            if column.name.lower() in incoming
            and is_nullable_type(incoming[column.name.lower()])
            and not is_nullable_type(column.type)
            and nullable_type(column.type) != column.type
            and canonical_type(incoming[column.name.lower()]) == canonical_type(column.type)
        )
        if not widened:
            return target_columns
        self._refuse_nullable_key_columns(
            connection=connection,
            destination=destination,
            columns=tuple(column.name for column in widened),
        )
        self.alter_column_types(
            connection=connection,
            destination=destination,
            columns=widened,
            statement_recorder=statement_recorder,
        )
        by_name: dict[str, ColumnInfo] = {column.name: column for column in widened}
        return tuple(by_name.get(column.name, column) for column in target_columns)

    def _refuse_nullable_key_columns(
        self, *, connection: Any, destination: str, columns: tuple[str, ...]
    ) -> None:
        """Explain a key column that cannot become ``Nullable`` before ClickHouse refuses it."""

        schema, name = _split_relation(destination)
        row: Any = self.execute(
            connection=connection,
            sql="SELECT sorting_key, partition_key, primary_key, create_table_query "
            f"FROM system.tables WHERE {_database_filter(schema)} AND name = {quote_string(name)}",
        ).fetchone()
        if row is None or NULLABLE_KEY_SETTING in str(row[3]):
            return
        keyed: tuple[str, ...] = tuple(
            column for column in columns if column in key_identifiers(tuple(map(str, row[:3])))
        )
        if not keyed:
            return
        raise AdapterUserError(
            message=(
                f"{destination}: the model now produces NULL in key column(s) "
                f"{', '.join(keyed)}, which ClickHouse cannot make Nullable"
            ),
            code=NULLABLE_KEY_CODE,
            help=(
                "Keep the key non-null in the model, for example coalesce(column, default) or "
                "assumeNotNull(column), or set table_settings (allow_nullable_key true) and "
                "rebuild the table with --full-refresh."
            ),
        )

    def alter_column_types(
        self,
        *,
        connection: Any,
        destination: str,
        columns: tuple[ColumnInfo, ...],
        statement_recorder: StatementRecorder,
    ) -> None:
        statements: tuple[str, ...] = self.render_alter_column_types(
            destination=destination, columns=columns
        )
        statement_recorder.record_many(statements)
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

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

    def capture_dependent_view_rebinds(
        self, *, connection: Any, database: str | None, schema: str, name: str
    ) -> tuple[str, ...]:
        del connection, database, schema, name
        return ()

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
        statements: tuple[str, ...] = self.render_create_function(
            destination=definition.destination,
            arguments=definition.arguments,
            returns=definition.returns,
            body_sql=definition.body_sql,
            return_columns=definition.return_columns,
            language=definition.language,
            runtime_version=definition.runtime_version,
            entry_point=definition.entry_point,
            packages=definition.packages,
        )
        statement_recorder.record_many(statements)
        stmt: str
        for stmt in statements:
            self.execute(connection=connection, sql=stmt)

    def create_table_as(
        self,
        *,
        connection: Any,
        destination: str,
        sql: str,
        config: dict[str, Any] | None = None,
        statement_recorder: StatementRecorder,
    ) -> None:
        statements: tuple[str, ...] = self._render_create_table_with_layout(
            destination=destination,
            sql=sql,
            layout=dict((config or {}).get(PHYSICAL_STORAGE_CONFIG_KEY) or {}),
        )
        statement_recorder.record_many(statements)
        stmt: str
        for stmt in statements:
            self.execute(connection=connection, sql=stmt)
        expected: object = (config or {}).get(EXPECTED_OUTPUT_COLUMNS_CONFIG_KEY)
        if expected:
            self._check_output_column_names(
                connection=connection,
                destination=destination,
                model_name=str((config or {}).get(MODEL_NAME_CONFIG_KEY) or destination),
                expected=tuple(str(name) for name in cast(tuple[object, ...], expected)),
            )

    def _check_output_column_names(
        self, *, connection: Any, destination: str, model_name: str, expected: tuple[str, ...]
    ) -> None:
        """Refuse a table whose columns ClickHouse named ``alias.column`` instead of ``column``."""

        renamed: tuple[tuple[str, str], ...] = alias_qualified_columns(
            actual=tuple(
                column.name
                for column in self.describe_relation(connection=connection, relation=destination)
            ),
            expected=expected,
        )
        if not renamed:
            return
        self.execute(connection=connection, sql=f"DROP TABLE IF EXISTS {destination}")
        pairs: str = ", ".join(
            f"'{produced}' instead of '{wanted}'" for produced, wanted in renamed
        )
        aliases: str = ", ".join(f"{produced} AS {wanted}" for produced, wanted in renamed)
        raise AdapterUserError(
            message=(
                f"model '{model_name}' produced columns named after their table alias on "
                f"ClickHouse: {pairs}"
            ),
            code=OUTPUT_COLUMN_NAME_CODE,
            help=(
                "ClickHouse names a selected alias.column after the alias when another joined "
                f"table has the same column; alias it in the model: {aliases}"
            ),
        )

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
        stmt: str
        for stmt in statements:
            self.execute(connection=connection, sql=stmt)

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
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

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
        stmt: str
        for stmt in statements:
            self.execute(connection=connection, sql=stmt)

    def get_columns_for_relations(
        self,
        *,
        connection: Any,
        relations: tuple[RelationInfo, ...],
    ) -> dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]]:
        """Fetch columns keyed by fully qualified physical relation identity."""

        return get_columns_for_relations_bulk(
            adapter=self, connection=connection, relations=relations
        )

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
        """Read candidate reads from this node's system.query_log."""

        return read_relation_usage(
            run_query=lambda sql: list(self.execute(connection=connection, sql=sql).fetchall()),
            relations=relations,
            window_seconds=window_seconds,
        )

    def get_tables_freshness_metadata(
        self,
        *,
        connection: Any,
        requests: tuple[TableFreshnessRequest, ...],
    ) -> dict[TableFreshnessRequest, TableFreshnessMetadata]:
        results: dict[TableFreshnessRequest, TableFreshnessMetadata] = {}
        request: TableFreshnessRequest
        for request in requests:
            try:
                results[request] = self.get_table_freshness_metadata(
                    connection=connection,
                    database=request.database,
                    schema=request.schema,
                    name=request.name,
                )
            except AdapterUserError as error:
                results[request] = TableFreshnessMetadata.unavailable(message=error.message)
        return results

    def list_dependent_view_names(
        self, *, connection: Any, database: str | None, schema: str, name: str
    ) -> tuple[str, ...]:
        """Return qualified names of views bound to this relation."""

        del connection, database, schema, name
        return ()

    def maximum_identifier_length(self) -> int:
        """Return the maximum unqualified identifier length supported by the adapter."""

        return self.max_identifier_length

    def persists_python_functions(self) -> bool:
        return True

    def python_functions_inherit_default_namespace(self) -> bool:
        return True

    def query_column_names(self, *, connection: Any, sql: str) -> tuple[str, ...]:
        """Return column names produced by a SQL query without materializing full rows."""

        cursor: Any = self.execute(
            connection=connection, sql=f"SELECT * FROM ({sql}) AS __describe_source LIMIT 0"
        )
        description: Any | None = getattr(cursor, "description", None)
        if description is None:
            return ()
        return tuple(str(column[0]) for column in description)

    def read_relation_grants(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str,
        name: str,
        relation_type: str,
    ) -> tuple[RelationGrant, ...]:
        """Return the privileges granted or denied on a relation and its columns."""

        del connection, database, schema, name, relation_type
        return ()

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

    def rename_view(
        self,
        *,
        connection: Any,
        origin: str,
        destination: str,
        statement_recorder: StatementRecorder,
    ) -> None:
        """Rename a view in place, keeping its privileges."""

        statements: tuple[str, ...] = self.render_rename_view(
            origin=origin, destination=destination
        )
        statement_recorder.record_many(statements)
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

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

    def render_insert_source_freshness_records_sql(
        self,
        *,
        database: str | None,
        schema: str,
        records: tuple[SourceFreshnessRecord, ...],
    ) -> str:
        """Render DML that appends source freshness records."""

        return render_insert_source_freshness_records_sql(
            database=database,
            schema=schema,
            records=records,
            render_qualified_name=self.render_qualified_name,
        )

    def render_read_latest_fingerprints_sql(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> str:
        """Render SQL that reads latest fingerprint rows per identity."""

        from sqlbuild._native import (
            build_read_latest_sql,
        )

        return build_read_latest_sql(
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
        )

    def render_read_latest_source_freshness_sql(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> str:
        """Render SQL that reads latest source freshness rows per identity."""

        from sqlbuild._native import build_source_freshness_read_latest_sql

        return build_source_freshness_read_latest_sql(
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
        )

    def render_relation_grants(
        self, *, grants: tuple[RelationGrant, ...], destination: str, columns: tuple[str, ...]
    ) -> tuple[str, ...]:
        """Render grants onto ``destination``, keeping column grants only for ``columns``."""

        del grants, destination, columns
        return ()

    def render_relation_revokes(
        self, *, grants: tuple[RelationGrant, ...], destination: str
    ) -> tuple[str, ...]:
        """Render statements that remove grants from ``destination``."""

        del grants, destination
        return ()

    def render_seed_select_before_cursor(
        self,
        *,
        origin: str,
        cursor_column: str,
        cursor_end_exclusive: str,
        cursor_type: str | None,
    ) -> str:
        return self._render_seed_select_before_cursor_impl(
            origin=origin,
            cursor_column=cursor_column,
            cursor_end_exclusive=cursor_end_exclusive,
            cursor_type=cursor_type,
        )

    def render_set_difference_operator(self) -> str:
        """Render the generic SQL set-difference operator."""

        return "EXCEPT"

    def render_source_expression_cast_subquery(
        self, *, source_relation: str, projections: tuple[str, ...]
    ) -> str:
        """Render a generic type-enforced source expression table factor."""

        projection_clause: str = ", ".join(projections)
        return f"(SELECT {projection_clause} FROM {source_relation} AS __source_expression)"

    def render_source_expression_relation(self, *, expression: str) -> str:
        """Render a generic source expression as a SQL table factor."""

        stripped_expression: str = expression.strip().removesuffix(";").strip()
        if stripped_expression.startswith("("):
            return stripped_expression
        lowered: str = stripped_expression.lower()
        if lowered.startswith(("select", "with", "values")):
            return f"({stripped_expression})"
        return stripped_expression

    def render_source_freshness_max_query(
        self, *, column: str, source_relation: str, source_is_subquery: bool, where_sql: str
    ) -> str:
        """Render a max-column source freshness query over a source table factor."""

        del source_is_subquery
        return (
            f"SELECT MAX({self.render_identifier(column)}) AS data_version "
            f"FROM {source_relation}{where_sql}"
        )

    def render_source_relation_cast_subquery(
        self,
        *,
        source_relation: str,
        cast_projections: tuple[str, ...],
        cast_column_names: tuple[str, ...],
        all_columns_cast: bool,
    ) -> str:
        """Render a generic type-enforced source relation table factor."""

        cast_clause: str = ", ".join(cast_projections)
        if all_columns_cast:
            return f"(SELECT {cast_clause} FROM {source_relation})"
        exclude_list: str = ", ".join(self.render_identifier(name) for name in cast_column_names)
        return (
            f"(SELECT * {self.star_exclude_keyword()} ({exclude_list}), {cast_clause} "
            f"FROM {source_relation})"
        )

    def render_table_function_call(self, *, target: str, call_suffix_sql: str) -> str:
        return f"{target}{call_suffix_sql}"

    def render_typed_value_list(self, *, value: SqlValue) -> str:
        """Render scalar collection members as a parenthesized SQL value list."""

        return _render_typed_value_list(value=value, render_scalar=self.render_typed_scalar)

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
        stmt: str
        for stmt in statements:
            self.execute(connection=connection, sql=stmt)

    def supports_unqualified_function_fingerprints(self) -> bool:
        return False

    def view_definition_matches(
        self, *, connection: Any, database: str | None, schema: str, name: str, sql: str
    ) -> bool:
        """Return whether the view's stored definition is ``sql`` as this warehouse stores it."""

        definition: str | None = self.read_view_definition(
            connection=connection, database=database, schema=schema, name=name
        )
        return definition is not None and same_view_definition(definition=definition, sql=sql)

    def views_bind_to_relation_identity(self) -> bool:
        """Return whether views follow a renamed relation instead of re-resolving its name."""

        return False

    def with_relation_age_metadata(
        self,
        *,
        connection: Any,
        relations: tuple[RelationInfo, ...],
    ) -> tuple[RelationInfo, ...]:
        return relations


def _state_table_ddl(sql: str) -> str:
    """Make NOT NULL columns plain types and give the append-only table an explicit engine."""

    ddl: str = _NOT_NULL_COLUMN_PATTERN.sub(r"\1", sql.strip().removesuffix(";"))
    ddl = _BARE_BIGINT_PATTERN.sub(r"\1 Nullable(Int64)\2", ddl)
    if STATE_TABLE_ENGINE_KEYWORD in ddl.upper().rsplit(")", 1)[-1]:
        return ddl
    return f"{ddl} ENGINE = MergeTree ORDER BY tuple()"


def _error_code(error: BaseException) -> int | None:
    current: BaseException | None = error
    while current is not None:
        match: re.Match[str] | None = _ERROR_CODE_PATTERN.search(str(current))
        if match is not None:
            return int(match.group(1))
        current = current.__cause__ or getattr(current, "original_error", None)
    return None


def _classify_relation_not_found(error: BaseException) -> RelationReadStatus | None:
    return RelationReadStatus.MISSING if _error_code(error) in RELATION_NOT_FOUND_CODES else None


def _database_filter(schema: str | None) -> str:
    if schema is None:
        return "database = currentDatabase()"
    return f"database = {quote_string(schema)}"


def _in_filter(*, column: str, values: tuple[str, ...] | None) -> str:
    if values is None:
        return ""
    if not values:
        return " AND 0"
    return f" AND {column} IN ({', '.join(quote_string(value) for value in values)})"


def _unquote(relation: str) -> str:
    return relation.replace('"', "").replace("`", "")


def _split_relation(relation: str) -> tuple[str | None, str]:
    parts: list[str] = relation.rsplit(".", 1)
    name: str = parts[-1].strip().strip('"`')
    if len(parts) == 1:
        return None, name
    return parts[0].strip().strip('"`'), name


def _generation_query(relation: str) -> str:
    schema, name = _split_relation(relation)
    return _table_generation_query(schema=schema, name=name)


def _table_generation_query(*, schema: str | None, name: str) -> str:
    return (
        "SELECT toString(uuid), comment FROM system.tables WHERE "
        f"{_database_filter(schema)} AND name = {quote_string(name)}"
    )


def _qualify_like(*, destination: str, origin: str) -> str:
    if RELATION_SEPARATOR in destination or RELATION_SEPARATOR not in origin:
        return destination
    return f"{origin.rsplit(RELATION_SEPARATOR, 1)[0]}{RELATION_SEPARATOR}{destination}"


def _mangled_function_name(target: str) -> str:
    """Name a UDF ``<schema>__<function>``; ClickHouse SQL functions are server-global."""

    database, name = _split_relation(target)
    return name if database is None else f"{database}__{name}"


def _key_tuple(*, adapter: ClickHouseAdapter, keys: tuple[str, ...]) -> str:
    rendered: str = ", ".join(adapter.render_identifier(key) for key in keys)
    return f"({rendered})" if len(keys) > 1 else rendered


def _decimal_scale(value: object) -> int:
    exponent: object = Decimal(str(value)).as_tuple().exponent
    return max(0, -exponent) if isinstance(exponent, int) else 0


def _cursor_value_literal(cursor: CursorValue) -> str:
    if cursor.kind == CursorKind.INTEGER:
        return str(cursor.value)
    value: object = cursor.value
    text: str = value.isoformat() if isinstance(value, datetime) else str(value)
    return f"parseDateTime64BestEffort({quote_string(text)}, 6, 'UTC')"


def _prune_history_sql(
    *,
    table: str,
    partition_columns: tuple[str, ...],
    order_columns: tuple[str, ...],
    retain_versions: int,
) -> str:
    identity: str = ", ".join((*partition_columns, *order_columns))
    partition: str = ", ".join(partition_columns)
    order: str = ", ".join(f"{column} DESC" for column in order_columns)
    return (
        f"DELETE FROM {table} WHERE ({identity}) IN ("
        f"SELECT {identity} FROM (SELECT {identity}, row_number() OVER ("
        f"PARTITION BY {partition} ORDER BY {order}) AS __sqlbuild_history_rank FROM {table}) "
        f"WHERE __sqlbuild_history_rank > {retain_versions}) SETTINGS transform_null_in = 1"
    )
