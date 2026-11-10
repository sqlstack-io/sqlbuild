"""Base adapter with broad-compatibility default implementations."""

from __future__ import annotations

import json
from collections.abc import Callable
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
from sqlbuild.adapter.contract.classes.historical_check_snapshot_sql import (
    HistoricalCheckSnapshotSql,
)
from sqlbuild.adapter.contract.classes.historical_snapshot_sql import (
    render_is_distinct_from,
)
from sqlbuild.adapter.contract.classes.historical_timestamp_snapshot_sql import (
    HistoricalTimestampSnapshotSql,
)
from sqlbuild.adapter.contract.classes.retention_adapter import RetentionAdapterMixin
from sqlbuild.adapter.contract.classes.snapshot_sql import SnapshotSql
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.classes.strict_adapter import StrictAdapter
from sqlbuild.adapter.contract.constants import (
    INTEGER_TYPE_TOKEN,
    TYPED_OBJECT_ENTRY_PART_COUNT,
)
from sqlbuild.adapter.contract.exceptions import (
    AdapterUserError,
    UnsupportedTypedSqlRenderingError,
)
from sqlbuild.adapter.contract.main.column_name_key import column_name_key
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
    RowDiffCoverage,
    RowDiffPreparedRelations,
    RowDiffResult,
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
    CursorKind,
    FrameworkType,
    HistoricalSnapshotCloseStyle,
    HistoricalSnapshotInsertStyle,
    LoaderLogicalType,
    MigrationTransfer,
    PromotionStrategy,
    SnapshotLatestVersionStyle,
    SnapshotUpdateStyle,
    StatementSizeLimit,
    TablePromotionMode,
)
from sqlbuild.adapter.relations.main.get_columns_for_relations import (
    get_columns_for_relations_bulk,
)
from sqlbuild.compiler.compile.types import FunctionLanguage
from sqlbuild.spec.contracts.constants import DEFAULT_SEED_CSV_SETTINGS
from sqlbuild.sql_values.exceptions import SqlValueRenderingError

_SCALAR_SQL_VALUE_KINDS: frozenset[SqlValueKind] = frozenset(
    {
        SqlValueKind.STRING,
        SqlValueKind.INTEGER,
        SqlValueKind.BOOLEAN,
        SqlValueKind.FLOAT,
        SqlValueKind.DECIMAL,
        SqlValueKind.NULL,
    }
)
_COLLECTION_SQL_VALUE_KINDS: frozenset[SqlValueKind] = frozenset(
    {SqlValueKind.LIST, SqlValueKind.SET}
)


class BaseAdapter(RetentionAdapterMixin, StrictAdapter):
    """Adapter base with ANSI SQL defaults."""

    adapter_name: ClassVar[str]
    sql_analysis_dialect_name: ClassVar[str | None] = None
    sql_lexical_syntax: ClassVar[SqlLexicalSyntax] = SqlLexicalSyntax()
    max_identifier_length: ClassVar[int] = 63
    relation_grants_supported: ClassVar[bool] = True
    views_read_with_reader_access: ClassVar[bool] = False
    allows_implicit_managed_write_schema: ClassVar[bool] = False
    execution_duration_limit_seconds: ClassVar[int | None] = None
    statement_cancellation_is_asynchronous: ClassVar[bool] = False
    statement_limit_excludes_queueing: ClassVar[bool] = False
    metadata_inspection_concurrency: ClassVar[int] = 1
    concurrent_ddl_limit: ClassVar[int] = 1
    supports_session_warehouse: ClassVar[bool] = False
    state_timestamp_type: ClassVar[str | None] = None
    untyped_column_type: ClassVar[str | None] = None
    _snapshot_sql_dialect: ClassVar[SnapshotSqlDialect] = SnapshotSqlDialect(
        timestamp_type="TIMESTAMP",
        distinct_condition=render_is_distinct_from,
        update_style=SnapshotUpdateStyle.UPDATE_FROM,
        latest_version=SnapshotLatestVersionStyle.QUALIFY,
        historical_close=HistoricalSnapshotCloseStyle.CORRELATED,
        historical_insert=HistoricalSnapshotInsertStyle.WITH_INSERT,
    )

    def supports_zero_copy_clone(self) -> bool:
        return False

    def supports_durable_clone(self) -> bool:
        return False

    def supports_relation_age_metadata(self) -> bool:
        return False

    def supports_table_freshness_metadata(self) -> bool:
        return False

    def supports_python_functions(self) -> bool:
        return False

    def persists_python_functions(self) -> bool:
        return True

    def python_functions_inherit_default_namespace(self) -> bool:
        return True

    def supports_unqualified_function_fingerprints(self) -> bool:
        return False

    def supports_table_functions(self) -> bool:
        return False

    def interrupt_connection(self, connection: Any) -> bool:
        """Cancel a connection's running statement from another thread; False if unsupported."""

        del connection
        return False

    def lists_relations_authoritatively(self, *, database: str | None, schema: str) -> bool:
        """Return whether ``list_relations`` reports every relation in this scope."""

        del database, schema
        return False

    def supports_concurrent_microbatch_dml(self) -> bool:
        """Return whether disjoint same-target delete/insert batches may run concurrently."""

        return False

    def diff_unkeyed_rows(
        self,
        *,
        connection: Any,
        left: str,
        right: str,
        excluded_columns: tuple[str, ...] = (),
        cursor_column: str | None = None,
        start_cursor: CursorValue | None = None,
        end_cursor: CursorValue | None = None,
    ) -> RowDiffResult:
        """Compare complete-row multiplicities without a unique key."""

        _, joined_sql = self._unkeyed_multiset_sql(
            connection=connection,
            left=left,
            right=right,
            excluded_columns=excluded_columns,
            cursor_column=cursor_column,
            start_cursor=start_cursor,
            end_cursor=end_cursor,
        )
        sql: str = (
            f"{joined_sql} "
            "SELECT "
            "COALESCE(SUM(__left_count), 0), "
            "COALESCE(SUM(__right_count), 0), "
            "COALESCE(SUM(CASE WHEN __left_count < __right_count "
            "THEN __left_count ELSE __right_count END), 0), "
            "COALESCE(SUM(CASE WHEN __left_count > __right_count "
            "THEN __left_count - __right_count ELSE 0 END), 0), "
            "COALESCE(SUM(CASE WHEN __right_count > __left_count "
            "THEN __right_count - __left_count ELSE 0 END), 0) "
            "FROM __joined"
        )
        row: tuple[Any, ...] = self.execute(connection=connection, sql=sql).fetchone()
        left_count: int = int(row[0])
        right_count: int = int(row[1])
        equal_count: int = int(row[2])
        left_only_count: int = int(row[3])
        right_only_count: int = int(row[4])
        population_count: int = equal_count + left_only_count + right_only_count
        return RowDiffResult(
            left_count=left_count,
            right_count=right_count,
            joined_count=population_count,
            equal_count=equal_count,
            unequal_count=0,
            left_only_count=left_only_count,
            right_only_count=right_only_count,
            population_count=population_count,
            compared_count=population_count,
        )

    def _unkeyed_multiset_sql(
        self,
        *,
        connection: Any,
        left: str,
        right: str,
        excluded_columns: tuple[str, ...],
        cursor_column: str | None,
        start_cursor: CursorValue | None,
        end_cursor: CursorValue | None,
        carried_side: str | None = None,
    ) -> tuple[tuple[ColumnInfo, ...], str]:
        """Return the compared columns and the per-row multiplicity CTEs ending in ``__joined``."""

        left_columns: tuple[ColumnInfo, ...] = self.describe_relation(
            connection=connection, relation=left
        )
        excluded_names: frozenset[str] = frozenset(column.lower() for column in excluded_columns)
        compared_columns: tuple[ColumnInfo, ...] = tuple(
            column for column in left_columns if column.name.lower() not in excluded_names
        )
        if not compared_columns:
            raise AdapterUserError(message="unkeyed row diff requires at least one compared column")
        column_list: str = ", ".join(
            self.render_identifier(column.name) for column in compared_columns
        )
        multiplicity_column: str = "__sqlbuild_count"
        compared_names: frozenset[str] = frozenset(
            column.name.lower() for column in compared_columns
        )
        while multiplicity_column.lower() in compared_names:
            multiplicity_column += "_"
        equal_conditions: str = " AND ".join(
            self.build_row_diff_equal_expression(
                column=self.render_identifier(column.name),
                column_info=column,
                tolerances=None,
            )
            for column in compared_columns
        )
        cursor_filter: str = self.build_cursor_filter(
            cursor_column=cursor_column,
            start_cursor=start_cursor,
            end_cursor=end_cursor,
        )
        where_clause: str = f" WHERE {cursor_filter}" if cursor_filter else ""
        carried: str = ""
        if carried_side is not None:
            carried = "".join(
                f"__{carried_side}.{self.render_identifier(column.name)} AS __c{index}, "
                for index, column in enumerate(compared_columns)
            )
        return compared_columns, (
            f"WITH __left AS (SELECT {column_list}, COUNT(*) AS {multiplicity_column} "
            f"FROM {left}{where_clause} GROUP BY {column_list}), "
            f"__right AS (SELECT {column_list}, COUNT(*) AS {multiplicity_column} "
            f"FROM {right}{where_clause} GROUP BY {column_list}), "
            "__joined AS (SELECT "
            f"{carried}"
            f"COALESCE(__left.{multiplicity_column}, 0) AS __left_count, "
            f"COALESCE(__right.{multiplicity_column}, 0) AS __right_count "
            f"FROM __left FULL OUTER JOIN __right ON {equal_conditions})"
        )

    def physical_relation_generation(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> str | None:
        """Return a stable token for the current physical relation incarnation when available."""

        relations: tuple[RelationInfo, ...] = self.list_relations(
            connection=connection,
            database=database,
            schemas=None if schema is None else (schema,),
            names=(name,),
        )
        if len(relations) != 1 or relations[0].created_at is None:
            return None
        return relations[0].created_at.isoformat()

    def max_statement_size(self) -> StatementSizeLimit | None:
        """Return a hard SQL text limit and unit (bytes mean UTF-8), or None for no static limit."""

        return None

    def maximum_identifier_length(self) -> int:
        """Return the maximum unqualified identifier length supported by the adapter."""

        return self.max_identifier_length

    def describe_relation(self, *, connection: Any, relation: str) -> tuple[ColumnInfo, ...]:
        """Return relation column metadata using a generic DESCRIBE statement."""

        cursor: Any = self.execute(connection=connection, sql=f"DESCRIBE {relation}")
        return tuple(ColumnInfo(name=row[0], type=row[1]) for row in cursor.fetchall())

    def probe_relation_read(self, *, connection: Any, relation: str) -> RelationReadProbe:
        """Probe a rendered relation; only the adapter's not-found errors become missing."""

        return run_relation_read_probe(
            execute=self.execute,
            connection=connection,
            relation=relation,
            classify_not_found=lambda error: None,
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

    def relation_usage(
        self,
        *,
        connection: Any,
        relations: tuple[RelationInfo, ...],
        window_seconds: int,
    ) -> RelationUsageReport:
        """Report that usage is not checked; adapters with a usage source override this."""

        del connection, relations, window_seconds
        return unsupported_relation_usage()

    def query_column_names(self, *, connection: Any, sql: str) -> tuple[str, ...]:
        """Return column names produced by a SQL query without materializing full rows."""

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
        """Render a portable eligible-cursor MAX query."""

        quoted_cursor: str = self.render_identifier(cursor_column)
        maximum_literal: str = (
            f"CAST({self._quote_sql_string(maximum_allowed)} AS DATE)"
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
        """Build a WHERE clause fragment for cursor-bounded queries."""

        if cursor_column is None or start_cursor is None:
            return ""
        clauses: list[str] = [
            f"{cursor_column} >= {self._quote_sql_string(str(start_cursor.value))}"
        ]
        if end_cursor is not None:
            clauses.append(f"{cursor_column} < {self._quote_sql_string(str(end_cursor.value))}")
        return " AND ".join(clauses)

    def schema_exists(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str,
    ) -> bool:
        """Return whether the named schema exists in the warehouse."""

        query: str = (
            "SELECT 1 FROM information_schema.schemata "
            f"WHERE schema_name = {_quote_sql_string(schema)}"
        )
        if database is not None:
            query += f" AND catalog_name = {_quote_sql_string(database)}"
        cursor: Any = self.execute(connection=connection, sql=query)
        return cursor.fetchone() is not None

    def query(self, *, connection: Any, sql: str, limit: int | None) -> QueryResult:
        """Execute SQL and return normalized rows for ad hoc query output."""

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
        cursor: Any = connection.execute(
            "SELECT 1 FROM information_schema.tables "
            f"WHERE table_name = {_quote_sql_string(name)}"
            + (f" AND table_schema = {_quote_sql_string(schema)}" if schema else "")
            + (f" AND table_catalog = {_quote_sql_string(database)}" if database else "")
        )
        return cursor.fetchone() is not None

    def with_relation_age_metadata(
        self,
        *,
        connection: Any,
        relations: tuple[RelationInfo, ...],
    ) -> tuple[RelationInfo, ...]:
        return relations

    def list_relations(
        self,
        *,
        connection: Any,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[RelationInfo, ...]:
        query: str = (
            "SELECT table_name, table_schema, table_type "
            "FROM information_schema.tables WHERE 1=1"
            + _build_schemas_filter(schemas=schemas)
            + _build_names_filter(names=names)
            + (f" AND table_catalog = {_quote_sql_string(database)}" if database else "")
        )
        cursor: Any = connection.execute(query)
        return tuple(
            RelationInfo(
                database=database,
                schema=row[1],
                name=row[0],
                relation_type=row[2],
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
        query: str = (
            "SELECT routine_name, routine_schema, routine_type "
            "FROM information_schema.routines WHERE 1=1"
            + _build_schemas_filter(schemas=schemas, column_name="routine_schema")
            + _build_names_filter(names=names, column_name="routine_name")
            + (f" AND routine_catalog = {_quote_sql_string(database)}" if database else "")
        )
        cursor: Any = connection.execute(query)
        return tuple(
            FunctionInfo(
                database=database,
                schema=row[1],
                name=row[0],
                function_type=row[2],
            )
            for row in cursor.fetchall()
        )

    def get_columns(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> tuple[ColumnInfo, ...]:
        query: str = (
            "SELECT column_name, data_type FROM information_schema.columns "
            f"WHERE table_name = {_quote_sql_string(name)}"
            + (f" AND table_schema = {_quote_sql_string(schema)}" if schema else "")
            + (f" AND table_catalog = {_quote_sql_string(database)}" if database else "")
            + " ORDER BY ordinal_position"
        )
        cursor: Any = connection.execute(query)
        return tuple(ColumnInfo(name=row[0], type=row[1]) for row in cursor.fetchall())

    def get_all_columns(
        self,
        *,
        connection: Any,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> dict[str, tuple[ColumnInfo, ...]]:
        query: str = (
            "SELECT table_name, column_name, data_type "
            "FROM information_schema.columns WHERE 1=1"
            + _build_schemas_filter(schemas=schemas)
            + _build_names_filter(names=names)
            + (f" AND table_catalog = {_quote_sql_string(database)}" if database else "")
            + " ORDER BY table_name, ordinal_position"
        )
        cursor: Any = connection.execute(query)
        result: dict[str, list[ColumnInfo]] = {}
        row: Any
        for row in cursor.fetchall():
            table_name: str = row[0]
            if table_name not in result:
                result[table_name] = []
            result[table_name].append(ColumnInfo(name=row[1], type=row[2]))
        return {k: tuple(v) for k, v in result.items()}

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

    def render_create_schema(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        target: str = f"{database}.{schema}" if database is not None else schema
        return (f"CREATE SCHEMA IF NOT EXISTS {target}",)

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
        statements: tuple[str, ...] = self.render_create_schema(
            database=database,
            schema=schema,
        )
        statement_recorder.record_many(statements)
        stmt: str
        for stmt in statements:
            self.execute(connection=connection, sql=stmt)

    def render_create_table_as(self, *, destination: str, sql: str) -> tuple[str, ...]:
        return (f"CREATE OR REPLACE TABLE {destination} AS {sql}",)

    def render_create_view_as(self, *, destination: str, sql: str) -> tuple[str, ...]:
        return (f"CREATE OR REPLACE VIEW {destination} AS {sql}",)

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
        del runtime_version, entry_point, packages
        if return_columns:
            raise AdapterUserError(
                message=f"Adapter '{type(self).__name__}' does not support SQL table functions"
            )
        if language == FunctionLanguage.PYTHON:
            raise AdapterUserError(
                message=f"Adapter '{type(self).__name__}' does not support Python UDFs"
            )
        argument_sql: str = ", ".join(f"{arg.name} {arg.type}" for arg in arguments)
        return (
            f"CREATE OR REPLACE FUNCTION {destination}({argument_sql}) "
            f"RETURNS {returns} LANGUAGE SQL AS $$\n{body_sql}\n$$",
        )

    def render_append(
        self, *, destination: str, sql: str, columns: tuple[str, ...] | None = None
    ) -> tuple[str, ...]:
        if columns is not None:
            col_list: str = ", ".join(self.render_identifier(column) for column in columns)
            return (f"INSERT INTO {destination} ({col_list}) {sql}",)
        return (f"INSERT INTO {destination} {sql}",)

    def render_delete_insert(
        self,
        *,
        destination: str,
        sql: str,
        unique_key: tuple[str, ...],
        columns: tuple[str, ...] | None = None,
    ) -> tuple[str, ...]:
        key_condition: str = " AND ".join(
            f"{destination}.{self.render_identifier(k)} = __source.{self.render_identifier(k)}"
            for k in unique_key
        )
        delete_sql: str = (
            f"DELETE FROM {destination} WHERE EXISTS "
            f"(SELECT 1 FROM ({sql}) AS __source WHERE {key_condition})"
        )
        insert_stmts: tuple[str, ...] = self.render_append(
            destination=destination, sql=sql, columns=columns
        )
        return (delete_sql, *insert_stmts)

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
        delete_sql: str = (
            f"DELETE FROM {destination} "
            f"WHERE {self.render_identifier(cursor_column)} >= "
            f"{self._quote_sql_string(cursor_start)} "
            f"AND {self.render_identifier(cursor_column)} < {self._quote_sql_string(cursor_end)}"
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

    def render_rename(self, *, origin: str, destination: str) -> tuple[str, ...]:
        return (f"ALTER TABLE {origin} RENAME TO {destination}",)

    def render_rename_view(self, *, origin: str, destination: str) -> tuple[str, ...]:
        return (f"ALTER VIEW {origin} RENAME TO {destination}",)

    def render_swap(self, *, left: str, right: str) -> tuple[str, ...]:
        staging: str = self._with_replaced_relation_name(
            relation=left, name=f"{self._relation_name(left)}__swap_staging"
        )
        return (
            *self.render_rename(origin=left, destination=staging),
            *self.render_rename(origin=right, destination=left),
            *self.render_rename(origin=staging, destination=right),
        )

    def _relation_name(self, relation: str) -> str:
        part: str = relation.split(".")[-1].strip()
        quoted_identifier_character_count: int = 2
        if (
            part.startswith('"')
            and part.endswith('"')
            and len(part) >= quoted_identifier_character_count
        ):
            return part[1:-1].replace('""', '"')
        return part

    def _with_replaced_relation_name(self, *, relation: str, name: str) -> str:
        parts: list[str] = relation.split(".")
        parts[-1] = self.render_identifier(name)
        return ".".join(parts)

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
        del connection, database, schema, name
        return ()

    def views_bind_to_relation_identity(self) -> bool:
        """Return whether views follow a renamed relation instead of re-resolving its name."""

        return False

    def list_dependent_view_names(
        self, *, connection: Any, database: str | None, schema: str, name: str
    ) -> tuple[str, ...]:
        """Return qualified names of views bound to this relation."""

        del connection, database, schema, name
        return ()

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

    def render_relation_grants(
        self, *, grants: tuple[RelationGrant, ...], destination: str, columns: tuple[str, ...]
    ) -> tuple[str, ...]:
        """Render grants onto ``destination``, keeping column grants only for ``columns``."""

        del grants, destination, columns
        return ()

    def render_replace_view_keeping_grants(
        self, *, destination: str, sql: str
    ) -> tuple[str, ...] | None:
        """Return statements that redefine an existing view keeping its privileges, if any."""

        del destination, sql
        return None

    def read_view_definition(
        self, *, connection: Any, database: str | None, schema: str, name: str
    ) -> str | None:
        """Return the stored definition of a view, or None when unknown."""

        del connection, database, schema, name
        return None

    def view_definition_matches(
        self, *, connection: Any, database: str | None, schema: str, name: str, sql: str
    ) -> bool:
        """Return whether the view's stored definition is ``sql`` as this warehouse stores it."""

        definition: str | None = self.read_view_definition(
            connection=connection, database=database, schema=schema, name=name
        )
        return definition is not None and same_view_definition(definition=definition, sql=sql)

    def render_relation_revokes(
        self, *, grants: tuple[RelationGrant, ...], destination: str
    ) -> tuple[str, ...]:
        """Render statements that remove grants from ``destination``."""

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
        """Rename a view in place, keeping its privileges."""

        statements: tuple[str, ...] = self.render_rename_view(
            origin=origin, destination=destination
        )
        statement_recorder.record_many(statements)
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

    def supports_transactional_ddl(self) -> bool:
        return False

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

    def relation_names_match(self, *, left: str, right: str) -> bool:
        return self._relation_names_match_impl(left=left, right=right)

    def _render_seed_select_before_cursor_impl(
        self,
        *,
        origin: str,
        cursor_column: str,
        cursor_end_exclusive: str,
        cursor_type: str | None,
    ) -> str:
        quoted_cursor: str = self.render_identifier(cursor_column)
        end_literal: str = self.render_cursor_bound_literal(
            value=cursor_end_exclusive, cursor_type=cursor_type
        )
        return f"SELECT * FROM {origin} WHERE {quoted_cursor} < {end_literal}"

    def _relation_names_match_impl(self, *, left: str, right: str) -> bool:
        return left.replace('"', "") == right.replace('"', "")

    def render_replace_table_from_relation(
        self, *, destination: str, origin: str
    ) -> tuple[str, ...]:
        return self.render_create_table_as(destination=destination, sql=f"SELECT * FROM {origin}")

    def render_add_columns(
        self, *, destination: str, columns: tuple[ColumnInfo, ...]
    ) -> tuple[str, ...]:
        return tuple(
            f"ALTER TABLE {destination} ADD COLUMN {self.render_identifier(col.name)} {col.type}"
            for col in columns
        )

    def render_drop_columns(
        self, *, destination: str, column_names: tuple[str, ...]
    ) -> tuple[str, ...]:
        return tuple(
            f"ALTER TABLE {destination} DROP COLUMN {self.render_identifier(col_name)}"
            for col_name in column_names
        )

    def render_alter_column_types(
        self, *, destination: str, columns: tuple[ColumnInfo, ...]
    ) -> tuple[str, ...]:
        return tuple(
            f"ALTER TABLE {destination} ALTER COLUMN "
            f"{self.render_identifier(col.name)} TYPE {col.type}"
            for col in columns
        )

    def render_rename_column(
        self, *, destination: str, old_name: str, new_name: str
    ) -> tuple[str, ...]:
        """Render SQL that renames one table column in place, keeping its data."""

        return (
            f"ALTER TABLE {destination} RENAME COLUMN {self.render_identifier(old_name)} "
            f"TO {self.render_identifier(new_name)}",
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
            column_name_key(column) for column in (*unique_key, *exclude_columns)
        )
        join_condition: str = " AND ".join(
            f"__target.{self.render_identifier(k)} = __source.{self.render_identifier(k)}"
            for k in unique_key
        )
        update_assignments: str = ", ".join(
            f"{self.render_identifier(col)} = __source.{self.render_identifier(col)}"
            for col in source_columns
            if column_name_key(col) not in immutable_columns
        )
        insert_columns: str = ", ".join(self.render_identifier(col) for col in source_columns)
        insert_values: str = ", ".join(
            f"__source.{self.render_identifier(col)}" for col in source_columns
        )
        merge_sql: str = (
            f"MERGE INTO {destination} AS __target USING ({sql}) AS __source ON {join_condition} "
        )
        if update_assignments:
            merge_sql += f"WHEN MATCHED THEN UPDATE SET {update_assignments} "
        merge_sql += f"WHEN NOT MATCHED THEN INSERT ({insert_columns}) VALUES ({insert_values})"
        return (merge_sql,)

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

    def render_current_timestamp(self) -> str:
        return "CURRENT_TIMESTAMP"

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
        stmt: str
        for stmt in statements:
            self.execute(connection=connection, sql=stmt)

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
            connection.execute(stmt)

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
        if not allow_copy_fallback:
            raise AdapterUserError(
                message=f"Adapter '{type(self).__name__}' requires explicit copy fallback "
                "permission "
                "to move or copy relations"
            )
        statements: tuple[str, ...] = self.render_replace_table_from_relation(
            destination=destination,
            origin=origin,
        )
        if remove_origin:
            statements = (*statements, *self.render_drop(destination=origin))
        statement_recorder.record_many(statements)
        stmt: str
        for stmt in statements:
            self.execute(connection=connection, sql=stmt)

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
        raise AdapterUserError(message="load_seed requires an engine-specific implementation")

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
        raise AdapterUserError(message="merge requires an engine-specific implementation")

    def widen_target_nullability(
        self,
        *,
        connection: Any,
        destination: str,
        target_columns: tuple[ColumnInfo, ...],
        incoming_columns: tuple[ColumnInfo, ...],
        statement_recorder: StatementRecorder,
    ) -> tuple[ColumnInfo, ...]:
        """Let target columns admit NULL where incoming ones do; return the target columns."""

        del connection, destination, incoming_columns, statement_recorder
        return target_columns

    def add_columns(
        self,
        *,
        connection: Any,
        destination: str,
        columns: tuple[ColumnInfo, ...],
        statement_recorder: StatementRecorder,
    ) -> None:
        raise AdapterUserError(message="add_columns requires an engine-specific implementation")

    def drop_columns(
        self,
        *,
        connection: Any,
        destination: str,
        column_names: tuple[str, ...],
        statement_recorder: StatementRecorder,
    ) -> None:
        raise AdapterUserError(message="drop_columns requires an engine-specific implementation")

    def rename_column(
        self,
        *,
        connection: Any,
        destination: str,
        old_name: str,
        new_name: str,
        statement_recorder: StatementRecorder,
    ) -> None:
        raise AdapterUserError(message="rename_column requires an engine-specific implementation")

    def column_rename_unavailable_reason(self, *, connection: Any, destination: str) -> str | None:
        """Return why this table cannot rename a column in place, or None when it can."""

        del connection, destination
        return None

    def alter_column_types(
        self,
        *,
        connection: Any,
        destination: str,
        columns: tuple[ColumnInfo, ...],
        statement_recorder: StatementRecorder,
    ) -> None:
        raise AdapterUserError(
            message="alter_column_types requires an engine-specific implementation"
        )

    def diff_schema(
        self,
        *,
        connection: Any,
        left: str,
        right: str,
    ) -> SchemaDiffResult:
        raise AdapterUserError(message="diff_schema requires an engine-specific implementation")

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
        start_cursor: CursorValue | None = None,
        end_cursor: CursorValue | None = None,
    ) -> RowDiffResult:
        raise AdapterUserError(message="diff_rows requires an engine-specific implementation")

    def _inspect_row_diff_coverage(
        self,
        *,
        connection: Any,
        relation: str,
        cursor_column: str | None = None,
        start_cursor: CursorValue | None = None,
        end_cursor: CursorValue | None = None,
    ) -> RowDiffCoverage:
        """Return exact bounded row count and cursor extent for one relation."""

        cursor_filter: str = self.build_cursor_filter(
            cursor_column=cursor_column,
            start_cursor=start_cursor,
            end_cursor=end_cursor,
        )
        projections: str = "COUNT(*)"
        if cursor_column is not None:
            operand: str = self.render_cursor_operand(
                expression=cursor_column,
                cursor_type=None if start_cursor is None else start_cursor.kind,
            )
            projections += f", MIN({operand}), MAX({operand})"
        query: str = f"SELECT {projections} FROM {relation}"
        if cursor_filter:
            query += f" WHERE {cursor_filter}"
        row: tuple[Any, ...] = self.execute(connection=connection, sql=query).fetchone()
        return RowDiffCoverage(
            row_count=int(row[0]),
            minimum_cursor=row[1] if cursor_column is not None else None,
            maximum_cursor=row[2] if cursor_column is not None else None,
        )

    def inspect_row_diff_coverage(
        self,
        *,
        connection: Any,
        relation: str,
        cursor_column: str | None = None,
        start_cursor: CursorValue | None = None,
        end_cursor: CursorValue | None = None,
    ) -> RowDiffCoverage:
        """Return exact bounded row count and cursor extent for custom adapters."""

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
        """Report an unknown row count for custom adapters without metadata support."""

        return RelationRowCountEstimate(
            row_count=None,
            detail=f"adapter '{self.adapter_name}' does not report row counts from metadata",
        )

    def _build_row_diff_relation_ctes(
        self,
        *,
        connection: Any,
        left_sql: str,
        right_sql: str,
        keys: tuple[str, ...],
        sampling: RowDiffSampling | None,
        inspect_population: bool,
    ) -> RowDiffPreparedRelations:
        """Build bounded relation CTEs with optional deterministic union-key sampling."""

        if sampling is None:
            return RowDiffPreparedRelations(
                cte_sql=f"__left AS ({left_sql}), __right AS ({right_sql})"
            )
        key_list: str = ", ".join(keys)
        union_operator: str = self._render_row_diff_union_operator()
        key_union_sql: str = (
            f"SELECT {key_list} FROM ({left_sql}) AS __left_keys "
            f"{union_operator} SELECT {key_list} FROM ({right_sql}) AS __right_keys"
        )
        population_count: int | None = None
        if inspect_population:
            population_row: tuple[Any, ...] = self.execute(
                connection=connection,
                sql=f"SELECT COUNT(*) FROM ({key_union_sql}) AS __key_population",
            ).fetchone()
            population_count = int(population_row[0])
        sample_keys_sql: str = self._render_row_diff_sample_keys_sql(
            key_union_sql=key_union_sql,
            keys=keys,
            sampling=sampling,
        )
        left_join: str = " AND ".join(f"__bounded_left.{key} = __sample_keys.{key}" for key in keys)
        right_join: str = " AND ".join(
            f"__bounded_right.{key} = __sample_keys.{key}" for key in keys
        )
        return RowDiffPreparedRelations(
            cte_sql=(
                f"__bounded_left AS ({left_sql}), __bounded_right AS ({right_sql}), "
                f"__sample_keys AS ({sample_keys_sql}), "
                f"__left AS (SELECT __bounded_left.* FROM __bounded_left "
                f"INNER JOIN __sample_keys ON {left_join}), "
                f"__right AS (SELECT __bounded_right.* FROM __bounded_right "
                f"INNER JOIN __sample_keys ON {right_join})"
            ),
            population_count=population_count,
        )

    def _render_row_diff_sample_keys_sql(
        self,
        *,
        key_union_sql: str,
        keys: tuple[str, ...],
        sampling: RowDiffSampling,
    ) -> str:
        """Render deterministic top-N union-key selection for LIMIT-capable engines."""

        key_list: str = ", ".join(keys)
        hash_expression: str = self._render_row_diff_key_hash(
            keys=keys,
            alias="__key_union",
            seed=sampling.seed,
        )
        tie_break: str = ", ".join(f"__key_union.{key}" for key in keys)
        return (
            f"SELECT {key_list} FROM ({key_union_sql}) AS __key_union "
            f"ORDER BY {hash_expression}, {tie_break} LIMIT {sampling.row_limit}"
        )

    def _render_row_diff_union_operator(self) -> str:
        """Render the distinct union operator used for key population selection."""

        return "UNION"

    def _render_row_diff_key_hash(
        self,
        *,
        keys: tuple[str, ...],
        alias: str,
        seed: int,
    ) -> str:
        """Render a stable hash over one canonical composite key representation."""

        key_parts: list[str] = []
        key: str
        for key in keys:
            value_sql: str = f"CAST({alias}.{key} AS VARCHAR)"
            key_parts.append(f"CONCAT(LENGTH({value_sql}), ':', {value_sql})")
        payload_parts: str = ", ".join((f"'{seed}'", *key_parts))
        return self._render_row_diff_digest(f"CONCAT_WS('|', {payload_parts})")

    def _render_row_diff_digest(self, payload_sql: str) -> str:
        """Render the hash function applied to one canonical key payload."""

        return f"MD5({payload_sql})"

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
        start_cursor: CursorValue | None = None,
        end_cursor: CursorValue | None = None,
        limit: int = 20,
    ) -> tuple[RowDiffSampleRow, ...]:
        raise AdapterUserError(
            message="sample_unequal_rows requires an engine-specific implementation"
        )

    def sample_side_only_rows(
        self,
        *,
        connection: Any,
        left: str,
        right: str,
        unique_key: str | tuple[str, ...],
        side: str,
        cursor_column: str | None = None,
        start_cursor: CursorValue | None = None,
        end_cursor: CursorValue | None = None,
        limit: int = 20,
        sampling: RowDiffSampling | None = None,
    ) -> tuple[tuple[tuple[str, object], ...], ...]:
        raise AdapterUserError(
            message="sample_side_only_rows requires an engine-specific implementation"
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

    def normalize_row_diff_numeric_type(self, column_type: str) -> str | None:
        normalized: str = column_type.upper()
        if any(token in normalized for token in ("DOUBLE", "FLOAT", "REAL")):
            return "float"
        if any(token in normalized for token in ("DECIMAL", "NUMERIC")):
            return "decimal"
        if INTEGER_TYPE_TOKEN in normalized:
            return "integer"
        return self.sql_analysis_dialect_name

    def format_row_diff_decimal_sql(self, value: Decimal) -> str:
        return format(value, "f")

    def default_schema(self) -> str | None:
        """Return None — most adapters require explicit schema configuration."""
        return self.sql_analysis_dialect_name

    def default_database(self) -> str | None:
        """Return None — most adapters require explicit database configuration."""
        return None

    def star_exclude_keyword(self) -> str:
        """Return the SQL keyword for SELECT * EXCLUDE/EXCEPT syntax."""
        return "EXCLUDE"

    def render_qualified_name(
        self,
        *,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> str | None:
        """Render a dot-separated qualified relation name from resolved parts."""

        if database is not None and schema is not None:
            return f"{database}.{schema}.{name}"
        if schema is not None:
            return f"{schema}.{name}"
        return None

    def render_identifier(self, name: str) -> str:
        """Render one SQL identifier using standard double-quote escaping."""

        return '"' + name.replace('"', '""') + '"'

    def render_exact_identifier(self, name: str) -> str:
        """Render one explicitly quoted identifier without changing its logical spelling."""

        return self.render_identifier(name)

    def render_framework_type(self, type_name: FrameworkType) -> str:
        """Render one framework-internal logical type using generic SQL defaults."""

        match type_name:
            case FrameworkType.STRING:
                return "VARCHAR"
            case FrameworkType.INTEGER:
                return "BIGINT"
            case FrameworkType.TIMESTAMP:
                return "TIMESTAMP"

    def render_loader_logical_type(self, type_name: LoaderLogicalType) -> str:
        """Render one source-loader logical type using generic SQL defaults."""

        match type_name:
            case LoaderLogicalType.BOOLEAN:
                return "BOOLEAN"
            case LoaderLogicalType.INTEGER:
                return "BIGINT"
            case LoaderLogicalType.FLOAT:
                return "DOUBLE"
            case LoaderLogicalType.STRING:
                return "VARCHAR"
            case LoaderLogicalType.TIMESTAMP:
                return "TIMESTAMP"
            case LoaderLogicalType.DATE:
                return "DATE"
            case LoaderLogicalType.JSON:
                return "JSON"

    def render_typed_scalar(self, *, value: SqlValue) -> str:
        """Render a validated typed scalar using ANSI-compatible literals."""

        return _render_ansi_typed_scalar(value=value)

    def render_typed_value_list(self, *, value: SqlValue) -> str:
        """Render scalar collection members as a parenthesized SQL value list."""

        return _render_typed_value_list(value=value, render_scalar=self.render_typed_scalar)

    def render_typed_array(self, *, value: SqlValue) -> str:
        del value
        raise UnsupportedTypedSqlRenderingError(adapter_name=self.adapter_name, rendering="array")

    def render_typed_object(self, *, value: SqlValue) -> str:
        del value
        raise UnsupportedTypedSqlRenderingError(adapter_name=self.adapter_name, rendering="object")

    def _render_typed_array_item(self, value: SqlValue) -> str:
        if value.kind in _SCALAR_SQL_VALUE_KINDS:
            return self.render_typed_scalar(value=value)
        if value.kind in _COLLECTION_SQL_VALUE_KINDS:
            return self.render_typed_array(value=value)
        if value.kind == SqlValueKind.OBJECT:
            return self.render_typed_object(value=value)
        raise AdapterUserError(
            message=f"unsupported trusted typed SQL value kind '{value.kind.value}'"
        )

    def _render_typed_array_items(self, value: SqlValue) -> str:
        return ", ".join(
            self._render_typed_array_item(item) for item in _typed_collection_items(value)
        )

    def render_loader_value_literal(
        self, *, value: object, logical_type: LoaderLogicalType | None
    ) -> str:
        """Render one generic source-loader value literal."""

        del logical_type
        if value is None:
            return "NULL"
        if isinstance(value, bool):
            return "TRUE" if value else "FALSE"
        if isinstance(value, int | float | Decimal):
            return str(value)
        if isinstance(value, datetime | date):
            return _quote_sql_string(value.isoformat())
        if isinstance(value, dict | list):
            return _quote_sql_string(json.dumps(value, sort_keys=True))
        return _quote_sql_string(str(value))

    def render_loader_rows_select(
        self,
        *,
        rows: tuple[dict[str, object], ...],
        column_names: tuple[str, ...],
        column_sql_types: dict[str, str],
        inferred_types: dict[str, LoaderLogicalType],
    ) -> str:
        """Render generic VALUES-backed source-loader rows as a SELECT."""

        if not rows:
            projections: str = ", ".join(
                "CAST(NULL AS "
                f"{column_sql_types.get(column_name, 'VARCHAR')}) AS "
                f"{self.render_identifier(column_name)}"
                for column_name in column_names
            )
            return f"SELECT {projections} WHERE 1 = 0"
        value_rows: list[str] = []
        for row in rows:
            row_values: list[str] = []
            for column_name in column_names:
                row_values.append(
                    self.render_loader_value_literal(
                        value=row.get(column_name),
                        logical_type=inferred_types.get(column_name),
                    )
                )
            value_rows.append("(" + ", ".join(row_values) + ")")
        values_sql: str = ", ".join(value_rows)
        column_sql: str = ", ".join(
            self.render_identifier(column_name) for column_name in column_names
        )
        select_sql: str = ", ".join(
            self._loader_rows_projection_sql(
                column_name=column_name,
                column_sql_types=column_sql_types,
            )
            for column_name in column_names
        )
        return f"SELECT {select_sql} FROM (VALUES {values_sql}) AS __loader_rows({column_sql})"

    def _loader_rows_projection_sql(
        self, *, column_name: str, column_sql_types: dict[str, str]
    ) -> str:
        quoted_column: str = self.render_identifier(column_name)
        sql_type: str | None = column_sql_types.get(column_name)
        if sql_type is None:
            return quoted_column
        return f"CAST({quoted_column} AS {sql_type}) AS {quoted_column}"

    def render_source_expression_cast(
        self, *, expression: str, target_type: str, alias: str
    ) -> str:
        """Render a generic cast projection for source expression type enforcement."""

        return f"CAST({expression} AS {target_type}) AS {alias}"

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

    def render_source_expression_cast_subquery(
        self, *, source_relation: str, projections: tuple[str, ...]
    ) -> str:
        """Render a generic type-enforced source expression table factor."""

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
        """Render a generic type-enforced source relation table factor."""

        cast_clause: str = ", ".join(cast_projections)
        if all_columns_cast:
            return f"(SELECT {cast_clause} FROM {source_relation})"
        exclude_list: str = ", ".join(self.render_identifier(name) for name in cast_column_names)
        return (
            f"(SELECT * {self.star_exclude_keyword()} ({exclude_list}), {cast_clause} "
            f"FROM {source_relation})"
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
        """Render a type-enforced source relation with warehouse column context."""

        del warehouse_column_names
        return self.render_source_relation_cast_subquery(
            source_relation=source_relation,
            cast_projections=cast_projections,
            cast_column_names=cast_column_names,
            all_columns_cast=all_columns_cast,
        )

    def requires_derived_table_aliases(self) -> bool:
        """Return whether derived table factors need explicit aliases."""

        return False

    def render_set_difference_operator(self) -> str:
        """Render the generic SQL set-difference operator."""

        return "EXCEPT"

    def render_create_fingerprint_table_sql(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> str:
        """Render DDL that creates the fingerprint table when it is missing."""

        from sqlbuild._native import (
            build_create_table_sql,
        )

        return build_create_table_sql(
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
            render_framework_type=self.render_framework_type,
        )

    def render_create_microbatch_state_table_sql(self, *, database: str | None, schema: str) -> str:
        """Render DDL that creates the direct microbatch state table when missing."""

        from sqlbuild._native import build_microbatch_create_table_sql

        return build_microbatch_create_table_sql(
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
            render_framework_type=self.render_framework_type,
        )

    def render_create_microbatch_state_index_sqls(
        self, *, database: str | None, schema: str
    ) -> tuple[str, ...]:
        """Render optional direct-state indexes for warehouses that support them."""

        return ()

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

    def render_create_fingerprint_index_sqls(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        """Render optional fingerprint table index DDL statements."""

        del database, schema
        return ()

    def render_create_source_freshness_table_sql(self, *, database: str | None, schema: str) -> str:
        """Render DDL that creates the source freshness table when it is missing."""

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
        """Render optional source freshness table index DDL statements."""

        del database, schema
        return ()

    def render_create_consumption_table_sql(self, *, database: str | None, schema: str) -> str:
        """Render DDL that creates the consumption table when it is missing."""

        from sqlbuild._native import build_consumption_create_table_sql

        return build_consumption_create_table_sql(
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
            render_framework_type=self.render_framework_type,
        )

    def render_create_consumption_index_sqls(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        """Render optional consumption table index DDL statements."""

        del database, schema
        return ()

    def render_current_utc_timestamp(self) -> str:
        """Render the warehouse clock as a UTC timestamp expression."""

        return "CURRENT_TIMESTAMP"

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

    def render_create_node_result_table_sql(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> str:
        """Render DDL that creates the node result table when it is missing."""

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
        """Render optional node result table index DDL statements."""

        del database, schema
        return ()

    def render_create_audit_result_table_sql(self, *, database: str | None, schema: str) -> str:
        """Render DDL that creates the audit result table when it is missing."""
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
        """Render optional audit result table index DDL statements."""
        del database, schema
        return ()

    def render_create_janitor_event_table_sql(self, *, database: str | None, schema: str) -> str:
        """Render DDL that creates the janitor audit event table when it is missing."""

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
        """Render DDL that creates the model migration event table when it is missing."""

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
        """Render DDL that creates the old-name view fact table when it is missing."""

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
        """Render DDL that creates the column migration event table when it is missing."""

        from sqlbuild._native import render_migration_state_create_sql

        return render_migration_state_create_sql(
            kind="column",
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
            render_framework_type=self.render_framework_type,
        )

    def render_prune_fingerprint_history_sql(
        self,
        *,
        database: str | None,
        schema: str,
        retain_versions: int,
    ) -> str:
        """Render SQL that prunes old fingerprint history rows."""

        del database, schema, retain_versions
        raise AdapterUserError(message="direct-state history pruning is adapter-specific")

    def render_prune_source_freshness_history_sql(
        self,
        *,
        database: str | None,
        schema: str,
        retain_versions: int,
    ) -> str:
        """Render SQL that prunes old source freshness history rows."""

        del database, schema, retain_versions
        raise AdapterUserError(message="direct-state history pruning is adapter-specific")

    def render_prune_consumption_history_sql(
        self,
        *,
        database: str | None,
        schema: str,
        retain_versions: int,
    ) -> str:
        """Render SQL that prunes old consumption executions."""

        del database, schema, retain_versions
        raise AdapterUserError(message="direct-state history pruning is adapter-specific")

    def sql_analysis_dialect(self) -> str | None:
        """Return the configured SQL analysis dialect name, if any."""

        return self.sql_analysis_dialect_name

    def expression_inference_profile(self) -> ExpressionInferenceProfile:
        """Return portable static expression inference behavior by default."""

        return ExpressionInferenceProfile(sql_analysis_dialect=self.sql_analysis_dialect())

    def _quote_sql_string(self, value: str) -> str:
        """Quote one string literal in this adapter's dialect; adapters override the escaping."""

        return _quote_sql_string(value)

    def render_cursor_bound_literal(self, *, value: str, cursor_type: str | None) -> str:
        """Render one generic cursor bound literal from a normalized string value."""

        if cursor_type == CursorKind.INTEGER:
            return value
        if cursor_type == CursorKind.TIMESTAMP:
            return f"TIMESTAMP {self._quote_sql_string(value)}"
        return self._quote_sql_string(value)

    def default_table_promotion_mode(self) -> TablePromotionMode:
        """Return staged as the generic default promotion mode."""
        return TablePromotionMode.STAGED

    def default_promotion_strategy(self) -> PromotionStrategy:
        """Return atomic swap as the generic staged promotion strategy."""
        return PromotionStrategy.ATOMIC_SWAP


def _build_schemas_filter(
    *,
    schemas: tuple[str, ...] | None,
    column_name: str = "table_schema",
) -> str:
    """Build an AND clause filtering to the given schemas."""

    if schemas is None:
        return ""
    quoted: str = ", ".join(_quote_sql_string(schema) for schema in schemas)
    return f" AND {column_name} IN ({quoted})"


def _build_names_filter(
    *,
    names: tuple[str, ...] | None,
    column_name: str = "table_name",
) -> str:
    """Build an AND clause filtering to the given relation names."""

    if not names:
        return ""
    quoted: str = ", ".join(_quote_sql_string(name) for name in names)
    return f" AND {column_name} IN ({quoted})"


def _typed_collection_items(value: SqlValue) -> tuple[SqlValue, ...]:
    if value.kind not in _COLLECTION_SQL_VALUE_KINDS or not isinstance(value.value, tuple):
        raise AdapterUserError(
            message=f"expected trusted typed SQL collection, received '{value.kind.value}'"
        )
    if not all(isinstance(item, SqlValue) for item in value.value):
        raise AdapterUserError(message="typed SQL collection contains an untrusted child")
    return cast(tuple[SqlValue, ...], value.value)


def _typed_object_items(value: SqlValue) -> tuple[tuple[str, SqlValue], ...]:
    if value.kind != SqlValueKind.OBJECT or not isinstance(value.value, tuple):
        raise AdapterUserError(
            message=f"expected trusted typed SQL object, received '{value.kind.value}'"
        )
    entries: list[tuple[str, SqlValue]] = []
    for entry in value.value:
        if (
            not isinstance(entry, tuple)
            or len(entry) != TYPED_OBJECT_ENTRY_PART_COUNT
            or not isinstance(entry[0], str)
            or not isinstance(entry[1], SqlValue)
        ):
            raise AdapterUserError(message="typed SQL object contains an untrusted entry")
        entries.append(cast(tuple[str, SqlValue], entry))
    return tuple(entries)


def _typed_scalar_payload(value: SqlValue) -> str | int | bool | float | Decimal | None:
    payload: object = value.value
    valid: bool
    match value.kind:
        case SqlValueKind.STRING:
            valid = isinstance(payload, str)
        case SqlValueKind.INTEGER:
            valid = isinstance(payload, int) and not isinstance(payload, bool)
        case SqlValueKind.BOOLEAN:
            valid = isinstance(payload, bool)
        case SqlValueKind.FLOAT:
            valid = isinstance(payload, float)
        case SqlValueKind.DECIMAL:
            valid = isinstance(payload, Decimal)
        case SqlValueKind.NULL:
            valid = payload is None
        case _:
            raise AdapterUserError(
                message=f"expected trusted typed SQL scalar, received '{value.kind.value}'"
            )
    if not valid:
        raise AdapterUserError(message=f"typed SQL {value.kind.value} contains an invalid payload")
    return cast(str | int | bool | float | Decimal | None, payload)


def _render_ansi_typed_scalar(*, value: SqlValue) -> str:
    payload: str | int | bool | float | Decimal | None = _typed_scalar_payload(value)
    match value.kind:
        case SqlValueKind.STRING:
            return _quote_sql_string(cast(str, payload))
        case SqlValueKind.BOOLEAN:
            return "TRUE" if payload else "FALSE"
        case SqlValueKind.NULL:
            return "NULL"
        case _:
            return str(payload)


def _render_typed_value_list(*, value: SqlValue, render_scalar: Callable[..., str]) -> str:
    items: tuple[SqlValue, ...] = _typed_collection_items(value)
    if any(item.kind not in _SCALAR_SQL_VALUE_KINDS for item in items):
        raise SqlValueRenderingError(
            "typed SQL value-list rendering supports scalar collection members only"
        )
    return "(" + ", ".join(render_scalar(value=item) for item in items) + ")"


def _encode_typed_json(value: SqlValue) -> str:
    if value.kind in _SCALAR_SQL_VALUE_KINDS:
        payload: str | int | bool | float | Decimal | None = _typed_scalar_payload(value)
        if value.kind == SqlValueKind.NULL:
            return "null"
        if value.kind == SqlValueKind.BOOLEAN:
            return "true" if payload else "false"
        if value.kind == SqlValueKind.STRING:
            return json.dumps(payload, ensure_ascii=True, separators=(",", ":"))
        return str(payload)
    if value.kind in _COLLECTION_SQL_VALUE_KINDS:
        return (
            "["
            + ",".join(_encode_typed_json(item) for item in _typed_collection_items(value))
            + "]"
        )
    if value.kind == SqlValueKind.OBJECT:
        return (
            "{"
            + ",".join(
                json.dumps(key, ensure_ascii=True, separators=(",", ":"))
                + ":"
                + _encode_typed_json(item)
                for key, item in sorted(_typed_object_items(value), key=lambda entry: entry[0])
            )
            + "}"
        )
    raise AdapterUserError(message=f"unsupported trusted typed SQL value kind '{value.kind.value}'")


def _validate_rectangular_typed_array(*, value: SqlValue, adapter_name: str) -> None:
    def dimensions(node: SqlValue) -> tuple[int, ...] | None:
        if node.kind == SqlValueKind.NULL:
            return None
        if node.kind not in _COLLECTION_SQL_VALUE_KINDS:
            return ()
        items: tuple[SqlValue, ...] = _typed_collection_items(node)
        child_dimensions: tuple[tuple[int, ...], ...] = tuple(
            child_shape for item in items if (child_shape := dimensions(item)) is not None
        )
        if child_dimensions and any(shape != child_dimensions[0] for shape in child_dimensions[1:]):
            raise UnsupportedTypedSqlRenderingError(
                adapter_name=adapter_name,
                rendering="array",
                reason="requires rectangular nested arrays",
            )
        return (len(items), *(child_dimensions[0] if child_dimensions else ()))

    dimensions(value)


def _render_explicit_source_relation_cast_subquery(
    *,
    source_relation: str,
    cast_projections: tuple[str, ...],
    cast_column_names: tuple[str, ...],
    warehouse_column_names: tuple[str, ...],
    all_columns_cast: bool,
    render_identifier: Callable[[str], str],
) -> str:
    """Render source casts by listing pass-through columns, for engines without star exclusion."""

    cast_clause: str = ", ".join(cast_projections)
    if all_columns_cast:
        return f"(SELECT {cast_clause} FROM {source_relation})"
    projection_names: set[str] = set(cast_column_names)
    passthrough_clause: str = ", ".join(
        render_identifier(column)
        for column in warehouse_column_names
        if column not in projection_names
    )
    projection_clause: str = (
        f"{passthrough_clause}, {cast_clause}" if passthrough_clause else cast_clause
    )
    return f"(SELECT {projection_clause} FROM {source_relation})"


def _quote_sql_string(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"
