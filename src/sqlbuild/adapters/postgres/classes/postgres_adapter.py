"""PostgreSQL adapter implementation."""

from __future__ import annotations

import csv
import json
import logging
from datetime import date, datetime
from decimal import Decimal
from pathlib import Path
from typing import Any, ClassVar

from sqlbuild._native import (
    SeedCsvSettings,
    SourceFreshnessRecord,
    SqlLexicalSyntax,
    SqlValue,
    render_insert_source_freshness_records_sql,
)
from sqlbuild.adapter.contract.classes.audit_results import AuditResultsMixin
from sqlbuild.adapter.contract.classes.base_adapter import (
    BaseAdapter,
    _build_names_filter,
    _build_schemas_filter,
    _encode_typed_json,
    _quote_sql_string,
    _render_ansi_typed_scalar,
    _render_explicit_source_relation_cast_subquery,
    _render_typed_value_list,
    _validate_rectangular_typed_array,
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
    QUALIFIED_NAME_SEPARATOR,
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
    TemporaryFunction,
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
from sqlbuild.adapters.postgres._helpers.grants import (
    postgres_relation_grants,
    render_postgres_grants,
    render_postgres_revokes,
)
from sqlbuild.adapters.postgres._helpers.view_rebind import render_postgres_view_rebind
from sqlbuild.adapters.postgres.classes.postgres_connection import _PostgresConnection
from sqlbuild.adapters.postgres.constants import (
    ROW_COUNT_RELATION_KINDS,
    TABLE_FUNCTION_RETURN_TYPE,
    TEMPORARY_TEST_FUNCTIONS_KEY,
)
from sqlbuild.compiler.compile.types import FunctionLanguage
from sqlbuild.diagnostics.main.log_sql import log_sql
from sqlbuild.runtime.execution_limits.main.execution_deadlines_suspended import (
    execution_deadlines_suspended,
)
from sqlbuild.spec.contracts.constants import DEFAULT_SEED_CSV_SETTINGS


class PostgresAdapter(
    MicrobatchMixin,
    ConsumptionMixin,
    ObservationsMixin,
    AuditResultsMixin,
    UnkeyedDiffMixin,
    BaseAdapter,
):
    """PostgreSQL adapter backed by psycopg."""

    def get_columns_for_relations(
        self,
        *,
        connection: Any,
        relations: tuple[RelationInfo, ...],
    ) -> dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]]:
        return get_columns_for_relations_bulk(
            adapter=self, connection=connection, relations=relations
        )

    adapter_name: ClassVar[str] = BuiltinAdapter.POSTGRES.value
    execution_duration_limit_seconds: ClassVar[int | None] = CANCELLABLE_DURATION_LIMIT_SECONDS
    sql_analysis_dialect_name: ClassVar[str | None] = "postgres"
    sql_lexical_syntax: ClassVar[SqlLexicalSyntax] = SqlLexicalSyntax(
        escape_string_prefix=True, nested_block_comments=True
    )
    max_identifier_length: ClassVar[int] = 63
    temporary_test_functions: bool = True
    _snapshot_sql_dialect: ClassVar[SnapshotSqlDialect] = SnapshotSqlDialect(
        timestamp_type="TIMESTAMP",
        distinct_condition=render_is_distinct_from,
        update_style=SnapshotUpdateStyle.UPDATE_FROM,
        latest_version=SnapshotLatestVersionStyle.DERIVED_TABLE,
        historical_close=HistoricalSnapshotCloseStyle.CORRELATED,
        historical_insert=HistoricalSnapshotInsertStyle.WITH_INSERT,
    )

    def create_sql_test_output(self, *, connection: Any, sql: str, name: str) -> str | None:
        """Store a unit test's output in a temporary table of one transaction, for poolers."""

        if connection.needs_sql_test_transaction():
            _ = self.execute(connection=connection, sql="BEGIN")
            connection.claim_sql_test_transaction()
        try:
            _ = self.execute(
                connection=connection,
                sql=f"CREATE TEMPORARY TABLE {name} ON COMMIT DROP AS {sql}",
            )
        except BaseException:
            with execution_deadlines_suspended():
                self.drop_sql_test_output(connection=connection, relation=name)
            raise
        return name

    def drop_sql_test_output(self, *, connection: Any, relation: str) -> None:
        """End the test's transaction, which drops its outputs, or drop one inside a caller's."""

        if connection.release_sql_test_transaction():
            _ = self.execute(connection=connection, sql="ROLLBACK")
            return
        if connection.in_transaction():
            _ = self.execute(connection=connection, sql=f"DROP TABLE IF EXISTS {relation}")

    def interrupt_connection(self, connection: Any) -> bool:
        """Cancel a connection's running statement from another thread; False if unsupported."""

        connection.raw_connection.cancel_safe()
        return True

    def lists_relations_authoritatively(self, *, database: str | None, schema: str) -> bool:
        """Return whether ``list_relations`` reports every relation in this scope."""

        del database, schema
        return True

    def supports_zero_copy_clone(self) -> bool:
        return False

    def supports_durable_clone(self) -> bool:
        return False

    def supports_relation_age_metadata(self) -> bool:
        """Return False because the Postgres catalog records no relation timestamps."""

        return False

    def supports_table_freshness_metadata(self) -> bool:
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
        undefined_table: str = "42P01"
        codes: tuple[object, ...] = (
            getattr(error, "sqlstate", None),
            getattr(error, "pgcode", None),
        )
        return RelationReadStatus.MISSING if undefined_table in codes else None

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
        return True

    def python_functions_inherit_default_namespace(self) -> bool:
        return True

    def supports_unqualified_function_fingerprints(self) -> bool:
        return False

    def supports_table_functions(self) -> bool:
        return False

    def max_statement_size(self) -> StatementSizeLimit | None:
        """No documented practical SQL text cap: https://www.postgresql.org/docs/current/limits.html."""

        return None

    def maximum_identifier_length(self) -> int:
        """Return the maximum unqualified identifier length supported by the adapter."""

        return self.max_identifier_length

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
        column_types: tuple[str | None, ...] = tuple(
            getattr(column, "type_display", None) for column in description
        )
        return QueryResult(
            columns=columns, rows=rows, truncated=truncated, column_types=column_types
        )

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

    def render_create_schema(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        del database
        return (f"CREATE SCHEMA IF NOT EXISTS {schema}",)

    def render_drop_schema(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        del database
        return (f"DROP SCHEMA IF EXISTS {schema} CASCADE",)

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
        arg_sql: str = ", ".join(f"{arg.name} {arg.type}" for arg in arguments)
        if language != FunctionLanguage.SQL:
            raise AdapterUserError(
                message="Python functions require an engine-specific implementation"
            )
        if returns.upper() == TABLE_FUNCTION_RETURN_TYPE:
            if return_columns:
                return_sql: str = ", ".join(f"{col.name} {col.type}" for col in return_columns)
                returns_clause: str = f"RETURNS TABLE ({return_sql})"
            else:
                returns_clause = "RETURNS TABLE"
        else:
            returns_clause = f"RETURNS {returns}"
        return (
            f"CREATE OR REPLACE FUNCTION {destination}({arg_sql})\n"
            f"{returns_clause}\n"
            f"LANGUAGE SQL AS $$\n{body_sql}\n$$",
        )

    def render_create_temporary_function(
        self, *, name: str, definition: FunctionDefinition
    ) -> TemporaryFunction | None:
        """Copy a SQL function into the session's `pg_temp` schema, called schema-qualified."""

        if definition.language != FunctionLanguage.SQL or not self.temporary_test_functions:
            return None
        target: str = f"pg_temp.{self.render_identifier(name)}"
        return TemporaryFunction(
            target=target,
            statements=self.render_create_function(
                destination=target,
                arguments=definition.arguments,
                returns=definition.returns,
                body_sql=definition.body_sql,
                return_columns=definition.return_columns,
            ),
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
        staged: str = f"{destination}__delete_insert"
        key_condition: str = " AND ".join(
            f"{destination}.{self.render_identifier(k)} = {staged}.{self.render_identifier(k)}"
            for k in unique_key
        )
        create_staged: tuple[str, ...] = self.render_create_table_as(destination=staged, sql=sql)
        delete_sql: str = f"DELETE FROM {destination} USING {staged} WHERE {key_condition}"
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
        unique_key: tuple[str, ...] = (),
    ) -> tuple[str, ...]:
        quoted_cursor_column: str = self.render_identifier(cursor_column)
        delete_sql: str = (
            f"DELETE FROM {destination} WHERE {quoted_cursor_column} >= "
            f"{self._quote_sql_string(cursor_start)} "
            f"AND {quoted_cursor_column} < {self._quote_sql_string(cursor_end)}"
        )
        if unique_key:
            return (
                delete_sql,
                *self.render_delete_insert(
                    destination=destination, sql=sql, unique_key=unique_key, columns=columns
                ),
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
        del database
        query: str = (
            "SELECT DISTINCT dependent_namespace.nspname, dependent.relname, "
            "pg_get_viewdef(dependent.oid), "
            "array_to_string(dependent.reloptions, chr(31)) "
            "FROM pg_depend AS dependency "
            "JOIN pg_rewrite AS rewrite ON rewrite.oid = dependency.objid "
            "JOIN pg_class AS dependent ON dependent.oid = rewrite.ev_class "
            "JOIN pg_namespace AS dependent_namespace "
            "ON dependent_namespace.oid = dependent.relnamespace "
            "JOIN pg_class AS referenced ON referenced.oid = dependency.refobjid "
            "JOIN pg_namespace AS referenced_namespace "
            "ON referenced_namespace.oid = referenced.relnamespace "
            "WHERE dependency.classid = 'pg_rewrite'::regclass "
            "AND dependency.refclassid = 'pg_class'::regclass "
            "AND dependent.relkind = 'v' "
            "AND dependent.oid <> referenced.oid "
            f"AND referenced_namespace.nspname = {_quote_sql_string(schema)} "
            f"AND referenced.relname = {_quote_sql_string(name)} "
            "ORDER BY 1, 2"
        )
        cursor: Any = connection.execute(query)
        return tuple(
            render_postgres_view_rebind(
                view=f"{self.render_identifier(str(row[0]))}.{self.render_identifier(str(row[1]))}",
                definition=str(row[2]),
                reloptions=tuple(str(row[3]).split(chr(31))) if row[3] else (),
            )
            for row in cursor.fetchall()
        )

    def views_bind_to_relation_identity(self) -> bool:
        return True

    def list_dependent_view_names(
        self, *, connection: Any, database: str | None, schema: str, name: str
    ) -> tuple[str, ...]:
        del database
        query: str = (
            "SELECT DISTINCT dependent_namespace.nspname, dependent.relname "
            "FROM pg_depend AS dependency "
            "JOIN pg_rewrite AS rewrite ON rewrite.oid = dependency.objid "
            "JOIN pg_class AS dependent ON dependent.oid = rewrite.ev_class "
            "JOIN pg_namespace AS dependent_namespace "
            "ON dependent_namespace.oid = dependent.relnamespace "
            "JOIN pg_class AS referenced ON referenced.oid = dependency.refobjid "
            "JOIN pg_namespace AS referenced_namespace "
            "ON referenced_namespace.oid = referenced.relnamespace "
            "WHERE dependency.classid = 'pg_rewrite'::regclass "
            "AND dependency.refclassid = 'pg_class'::regclass "
            "AND dependent.relkind IN ('v', 'm') "
            "AND dependent.oid <> referenced.oid "
            f"AND referenced_namespace.nspname = {_quote_sql_string(schema)} "
            f"AND referenced.relname = {_quote_sql_string(name)} "
            "ORDER BY 1, 2"
        )
        cursor: Any = connection.execute(query)
        return tuple(f"{row[0]}.{row[1]}" for row in cursor.fetchall())

    def read_relation_grants(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str,
        name: str,
        relation_type: str,
    ) -> tuple[RelationGrant, ...]:
        del database, relation_type
        relation_filter: str = (
            "FROM pg_class AS relation "
            "JOIN pg_namespace AS namespace ON namespace.oid = relation.relnamespace "
        )
        where: str = (
            f"WHERE namespace.nspname = {_quote_sql_string(schema)} "
            f"AND relation.relname = {_quote_sql_string(name)} "
            "AND acl.grantee <> relation.relowner "
        )
        grantee: str = "CASE WHEN acl.grantee = 0 THEN NULL ELSE pg_get_userbyid(acl.grantee) END"
        relation_rows: list[tuple[Any, ...]] = connection.execute(
            f"SELECT acl.privilege_type, {grantee}, acl.is_grantable "
            + relation_filter
            + "CROSS JOIN LATERAL aclexplode(relation.relacl) AS acl "
            + where
            + "ORDER BY 2 NULLS FIRST, 1"
        ).fetchall()
        column_rows: list[tuple[Any, ...]] = connection.execute(
            f"SELECT attribute.attname, acl.privilege_type, {grantee}, acl.is_grantable "
            + relation_filter
            + "JOIN pg_attribute AS attribute ON attribute.attrelid = relation.oid "
            "AND attribute.attnum > 0 AND NOT attribute.attisdropped "
            "CROSS JOIN LATERAL aclexplode(attribute.attacl) AS acl "
            + where
            + "ORDER BY attribute.attnum, 3 NULLS FIRST, 2"
        ).fetchall()
        return postgres_relation_grants(relation_rows=relation_rows, column_rows=column_rows)

    def render_relation_grants(
        self, *, grants: tuple[RelationGrant, ...], destination: str, columns: tuple[str, ...]
    ) -> tuple[str, ...]:
        return render_postgres_grants(
            grants=grants,
            destination=destination,
            columns=columns,
            render_identifier=self.render_identifier,
        )

    def render_replace_view_keeping_grants(
        self, *, destination: str, sql: str
    ) -> tuple[str, ...] | None:
        return self.render_create_view_as(destination=destination, sql=sql)

    def read_view_definition(
        self, *, connection: Any, database: str | None, schema: str, name: str
    ) -> str | None:
        del database
        rows: list[tuple[Any, ...]] = connection.execute(
            "SELECT pg_get_viewdef(relation.oid) FROM pg_class AS relation "
            "JOIN pg_namespace AS namespace ON namespace.oid = relation.relnamespace "
            f"WHERE relation.relkind = 'v' AND namespace.nspname = {_quote_sql_string(schema)} "
            f"AND relation.relname = {_quote_sql_string(name)}"
        ).fetchall()
        return None if not rows else str(rows[0][0])

    def view_definition_matches(
        self, *, connection: Any, database: str | None, schema: str, name: str, sql: str
    ) -> bool:
        definition: str | None = self.read_view_definition(
            connection=connection, database=database, schema=schema, name=name
        )
        if definition is None:
            return False
        connection.execute("BEGIN")
        try:
            connection.execute(f"CREATE TEMPORARY VIEW _sqb_view_probe AS {sql}")
            probe: str = str(
                connection.execute("SELECT pg_get_viewdef('_sqb_view_probe'::regclass)").fetchone()[
                    0
                ]
            )
        finally:
            with execution_deadlines_suspended():
                connection.execute("ROLLBACK")
        return same_view_definition(definition=definition, sql=probe)

    def render_relation_revokes(
        self, *, grants: tuple[RelationGrant, ...], destination: str
    ) -> tuple[str, ...]:
        return render_postgres_revokes(
            grants=grants, destination=destination, render_identifier=self.render_identifier
        )

    def rename_view(
        self,
        *,
        connection: Any,
        origin: str,
        destination: str,
        statement_recorder: StatementRecorder,
    ) -> None:
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
        return self._render_seed_select_before_cursor_impl(
            origin=origin,
            cursor_column=cursor_column,
            cursor_end_exclusive=cursor_end_exclusive,
            cursor_type=cursor_type,
        )

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
                new_identifier=self.render_identifier(new_name.lower()),
            ),
        )

    def render_alter_column_types(
        self, *, destination: str, columns: tuple[ColumnInfo, ...]
    ) -> tuple[str, ...]:
        return tuple(
            f"ALTER TABLE {destination} ALTER COLUMN "
            f"{self.render_identifier(col.name)} TYPE {col.type}"
            for col in columns
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
        join_condition: str = " AND ".join(
            f"__target.{self.render_identifier(k)} = __source.{self.render_identifier(k)}"
            for k in unique_key
        )
        update_assignments: str = ", ".join(
            f"{self.render_identifier(col)} = __source.{self.render_identifier(col)}"
            for col in source_columns
            if col.lower() not in immutable_columns
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

    def render_current_timestamp(self) -> str:
        return "CURRENT_TIMESTAMP"

    def render_current_utc_timestamp(self) -> str:
        return "(now() AT TIME ZONE 'UTC')"

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
        origin_parent: str = (
            origin.rsplit(QUALIFIED_NAME_SEPARATOR, 1)[0]
            if QUALIFIED_NAME_SEPARATOR in origin
            else ""
        )
        destination_parent: str = (
            destination.rsplit(QUALIFIED_NAME_SEPARATOR, 1)[0]
            if QUALIFIED_NAME_SEPARATOR in destination
            else ""
        )
        if remove_origin and origin_parent == destination_parent:
            self.rename(
                connection=connection,
                origin=origin,
                destination=destination,
                statement_recorder=statement_recorder,
            )
            return
        origin_parts: list[str] = origin.split(".")
        destination_parts: list[str] = destination.split(".")
        schema_and_relation_part_count: int = 2
        if (
            remove_origin
            and len(origin_parts) >= schema_and_relation_part_count
            and len(destination_parts) >= schema_and_relation_part_count
            and origin_parts[:-2] == destination_parts[:-2]
        ):
            origin_name: str = origin_parts[-1]
            destination_schema: str = destination_parts[-2]
            destination_name: str = destination_parts[-1]
            statements: tuple[str, ...] = ()
            moved_origin: str = origin
            if origin_name != destination_name:
                statements = (
                    *statements,
                    *self.render_rename(origin=origin, destination=destination),
                )
                moved_origin = ".".join((*origin_parts[:-1], destination_name))
            statements = (
                *statements,
                f"ALTER TABLE {moved_origin} SET SCHEMA {destination_schema}",
            )
            statement_recorder.record_many(statements)
            stmt: str
            for stmt in statements:
                self.execute(connection=connection, sql=stmt)
            return
        if not allow_copy_fallback:
            raise AdapterUserError(message="Postgres relation move/copy requires --allow-copy")
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

    def append(
        self,
        *,
        connection: Any,
        destination: str,
        sql: str,
        columns: tuple[str, ...] | None = None,
        statement_recorder: StatementRecorder,
    ) -> None:
        statements: tuple[str, ...] = self.render_append(
            destination=destination, sql=sql, columns=columns
        )
        statement_recorder.record_many(statements)
        stmt: str
        for stmt in statements:
            self.execute(connection=connection, sql=stmt)

    def delete_insert(
        self,
        *,
        connection: Any,
        destination: str,
        sql: str,
        unique_key: str | tuple[str, ...],
        columns: tuple[str, ...] | None = None,
        statement_recorder: StatementRecorder,
    ) -> None:
        keys: tuple[str, ...] = (unique_key,) if isinstance(unique_key, str) else unique_key
        statements: tuple[str, ...] = self.render_delete_insert(
            destination=destination, sql=sql, unique_key=keys, columns=columns
        )
        statement_recorder.record_many(statements)
        with self.transaction(connection):
            stmt: str
            for stmt in statements:
                self.execute(connection=connection, sql=stmt)

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
    ) -> None:
        statements: tuple[str, ...] = self.render_delete_insert_cursor(
            destination=destination,
            sql=sql,
            cursor_column=cursor_column,
            cursor_start=cursor_start,
            cursor_end=cursor_end,
            columns=columns,
            cursor_type=cursor_type,
            unique_key=unique_key,
        )
        statement_recorder.record_many(statements)
        with self.transaction(connection):
            stmt: str
            for stmt in statements:
                self.execute(connection=connection, sql=stmt)

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

    def format_row_diff_decimal_sql(self, value: Decimal) -> str:
        return format(value, "f")

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

    def render_framework_type(self, type_name: FrameworkType) -> str:
        """Render one framework-internal logical type using generic SQL defaults."""

        match type_name:
            case FrameworkType.STRING:
                return "VARCHAR"
            case FrameworkType.INTEGER:
                return "BIGINT"
            case FrameworkType.TIMESTAMP:
                return "TIMESTAMP"

    def render_set_difference_operator(self) -> str:
        """Render the generic SQL set-difference operator."""

        return "EXCEPT"

    def render_create_fingerprint_table_sql(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> str:
        from sqlbuild._native import (
            build_create_table_sql,
        )

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

        table_name: str | None = self.render_qualified_name(
            database=database,
            schema=schema,
            name=FINGERPRINT_TABLE_NAME,
        )
        if table_name is None:
            return ()
        index_name: str = "_sqlbuild_fingerprints_latest_idx"
        return (
            "CREATE INDEX IF NOT EXISTS "
            f"{index_name} ON {table_name} (node_type, node_name, ts DESC, run_id DESC)",
        )

    def render_create_microbatch_state_index_sqls(
        self, *, database: str | None, schema: str
    ) -> tuple[str, ...]:
        from sqlbuild._native import build_microbatch_create_index_sqls

        return build_microbatch_create_index_sqls(
            database=database,
            schema=schema,
            render_identifier=self.render_identifier,
            render_qualified_name=self.render_qualified_name,
        )

    def render_read_latest_fingerprints_sql(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> str:
        from sqlbuild._native import (
            build_read_latest_sql,
        )

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

        table_name: str | None = self.render_qualified_name(
            database=database,
            schema=schema,
            name=FINGERPRINT_TABLE_NAME,
        )
        if table_name is None:
            return ""
        return (
            f"DELETE FROM {table_name} WHERE ctid IN ("
            "SELECT ctid FROM ("
            "SELECT ctid, ROW_NUMBER() OVER ("
            "PARTITION BY node_type, node_name "
            "ORDER BY ts DESC, run_id DESC"
            f") AS __sqlbuild_history_rank FROM {table_name}"
            ") AS __sqlbuild_ranked "
            f"WHERE __sqlbuild_history_rank > {retain_versions}"
            ")"
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

        table_name: str | None = self.render_qualified_name(
            database=database,
            schema=schema,
            name=SOURCE_FRESHNESS_TABLE_NAME,
        )
        if table_name is None:
            return ()
        index_name: str = "_sqlbuild_source_freshness_latest_idx"
        return (
            "CREATE INDEX IF NOT EXISTS "
            f"{index_name} ON {table_name} ("
            "source_name, target_database, target_schema, target_name, "
            "observed_at DESC, run_id DESC)",
        )

    def render_create_consumption_index_sqls(
        self, *, database: str | None, schema: str
    ) -> tuple[str, ...]:
        from sqlbuild._native import CONSUMPTION_TABLE_NAME

        return self._state_index_sqls(
            table=(database, schema, CONSUMPTION_TABLE_NAME),
            indexes=(
                ("_sqlbuild_consumption_relation_idx", "target_schema, target_name, recorded_at"),
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

        table_name: str | None = self.render_qualified_name(
            database=database,
            schema=schema,
            name=NODE_RESULTS_TABLE_NAME,
        )
        latest_index_name: str = "_sqlbuild_node_results_latest_idx"
        run_id_index_name: str = "_sqlbuild_node_results_run_id_idx"
        if table_name is None:
            return ()
        return (
            "CREATE INDEX IF NOT EXISTS "
            f"{latest_index_name} ON {table_name} ("
            "node_type, node_name, target_database, target_schema, target_name, status, "
            "ts DESC, run_id DESC)",
            "CREATE INDEX IF NOT EXISTS "
            f"{run_id_index_name} ON {table_name} ("
            "run_id, node_type, node_name, target_database, target_schema, target_name)",
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

        return self._state_index_sqls(
            table=(database, schema, AUDIT_RESULTS_TABLE_NAME),
            indexes=(
                (
                    "_sqlbuild_audit_results_latest_idx",
                    "audit_name, binding_key, execution_fingerprint, run_scope_phase, "
                    "occurred_at DESC, result_id DESC",
                ),
                ("_sqlbuild_audit_results_run_id_idx", "run_id, invocation_id, result_id"),
            ),
        )

    def _state_index_sqls(
        self, *, table: tuple[str | None, str, str], indexes: tuple[tuple[str, str], ...]
    ) -> tuple[str, ...]:
        """Create each named state-table index on its columns unless it already exists."""

        database, schema, name = table
        table_name: str | None = self.render_qualified_name(
            database=database, schema=schema, name=name
        )
        if table_name is None:
            return ()
        return tuple(
            f"CREATE INDEX IF NOT EXISTS {index_name} ON {table_name} ({columns})"
            for index_name, columns in indexes
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

        table_name: str | None = self.render_qualified_name(
            database=database,
            schema=schema,
            name=SOURCE_FRESHNESS_TABLE_NAME,
        )
        if table_name is None:
            return ""
        return (
            f"DELETE FROM {table_name} WHERE ctid IN ("
            "SELECT ctid FROM ("
            "SELECT ctid, ROW_NUMBER() OVER ("
            "PARTITION BY source_name, target_database, target_schema, target_name "
            "ORDER BY observed_at DESC, run_id DESC"
            f") AS __sqlbuild_history_rank FROM {table_name}"
            ") AS __sqlbuild_ranked "
            f"WHERE __sqlbuild_history_rank > {retain_versions}"
            ")"
        )

    def sql_analysis_dialect(self) -> str | None:
        """Return the configured SQL analysis dialect name, if any."""

        return self.sql_analysis_dialect_name

    def expression_inference_profile(self) -> ExpressionInferenceProfile:
        """Return portable static expression inference behavior by default."""

        return ExpressionInferenceProfile(sql_analysis_dialect=self.sql_analysis_dialect())

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

    def render_identifier(self, name: str) -> str:
        """Render one PostgreSQL identifier using double-quote escaping."""

        return '"' + name.replace('"', '""') + '"'

    def render_exact_identifier(self, name: str) -> str:
        """Render one explicitly quoted PostgreSQL identifier with its exact case."""

        return self.render_identifier(name)

    def render_loader_logical_type(
        self, *, type_name: LoaderLogicalType, precision: int = 38, scale: int = 0
    ) -> str:
        match type_name:
            case LoaderLogicalType.BOOLEAN:
                return "BOOLEAN"
            case LoaderLogicalType.INTEGER:
                return "BIGINT"
            case LoaderLogicalType.FLOAT:
                return "DOUBLE PRECISION"
            case LoaderLogicalType.DECIMAL:
                return f"NUMERIC({precision}, {scale})"
            case LoaderLogicalType.STRING:
                return "TEXT"
            case LoaderLogicalType.TIMESTAMP:
                return "TIMESTAMP"
            case LoaderLogicalType.DATE:
                return "DATE"
            case LoaderLogicalType.JSON:
                return "JSONB"

    def render_typed_scalar(self, *, value: SqlValue) -> str:
        return _render_ansi_typed_scalar(value=value)

    def render_typed_value_list(self, *, value: SqlValue) -> str:
        return _render_typed_value_list(value=value, render_scalar=self.render_typed_scalar)

    def render_typed_array(self, *, value: SqlValue) -> str:
        _validate_rectangular_typed_array(value=value, adapter_name=self.adapter_name)
        return "ARRAY[" + self._render_typed_array_items(value) + "]"

    def render_typed_object(self, *, value: SqlValue) -> str:
        return f"{self._quote_sql_string(_encode_typed_json(value))}::JSONB"

    def render_loader_value_literal(
        self, *, value: object, logical_type: LoaderLogicalType | None
    ) -> str:
        if value is None:
            return "NULL"
        if logical_type == LoaderLogicalType.JSON:
            return f"{self._quote_sql_string(json.dumps(value, sort_keys=True))}::JSONB"
        if isinstance(value, bool):
            return "TRUE" if value else "FALSE"
        if isinstance(value, Decimal):
            return format(value, "f")
        if isinstance(value, int | float):
            return str(value)
        if isinstance(value, datetime | date):
            return self._quote_sql_string(value.isoformat())
        return self._quote_sql_string(str(value))

    def render_loader_rows_select(
        self,
        *,
        rows: tuple[dict[str, object], ...],
        column_names: tuple[str, ...],
        column_sql_types: dict[str, str],
        inferred_types: dict[str, LoaderLogicalType],
    ) -> str:
        if not rows:
            projections: str = ", ".join(
                "CAST(NULL AS "
                f"{column_sql_types.get(column_name, 'TEXT')}) AS "
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
            value_rows.append(f"({', '.join(row_values)})")
        values_sql: str = ", ".join(value_rows)
        column_sql: str = ", ".join(
            self.render_identifier(column_name) for column_name in column_names
        )
        select_sql: str = ", ".join(
            (
                self.render_identifier(column_name)
                if column_name not in column_sql_types
                else "CAST("
                f"{self.render_identifier(column_name)} AS {column_sql_types[column_name]}) "
                f"AS {self.render_identifier(column_name)}"
            )
            for column_name in column_names
        )
        return f"SELECT {select_sql} FROM (VALUES {values_sql}) AS __loader_rows({column_sql})"

    def _quote_sql_string(self, value: str) -> str:
        return "'" + value.replace("'", "''") + "'"

    def render_source_expression_cast(
        self, *, expression: str, target_type: str, alias: str
    ) -> str:
        return f"CAST({expression} AS {target_type}) AS {alias}"

    def render_source_expression_relation(self, *, expression: str) -> str:
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
        exclude_list: str = ", ".join(self.render_identifier(name) for name in cast_column_names)
        return f"(SELECT * EXCLUDE ({exclude_list}), {cast_clause} FROM {source_relation})"

    def _render_source_relation_cast_subquery_with_columns(
        self,
        *,
        source_relation: str,
        cast_projections: tuple[str, ...],
        cast_column_names: tuple[str, ...],
        warehouse_column_names: tuple[str, ...],
        all_columns_cast: bool,
    ) -> str:
        """Render Postgres source casts without relying on unsupported star exclusion."""

        return _render_explicit_source_relation_cast_subquery(
            source_relation=source_relation,
            cast_projections=cast_projections,
            cast_column_names=cast_column_names,
            warehouse_column_names=warehouse_column_names,
            all_columns_cast=all_columns_cast,
            render_identifier=self.render_identifier,
        )

    def requires_derived_table_aliases(self) -> bool:
        """Postgres does not require aliases for derived table factors."""

        return False

    def connect(self, config: dict[str, Any]) -> _PostgresConnection:
        try:
            import psycopg
        except ImportError as error:
            raise AdapterUserError(
                message="Postgres adapter requires optional dependency psycopg. "
                "Install with: pip install 'psycopg[binary]' or sqlbuild[postgres]",
                code="A401",
            ) from error

        connect_config: dict[str, Any] = dict(config)
        self.temporary_test_functions = bool(
            connect_config.pop(TEMPORARY_TEST_FUNCTIONS_KEY, self.temporary_test_functions)
        )
        raw_connection: Any = psycopg.connect(**connect_config, autocommit=True)
        return _PostgresConnection(raw_connection)

    def _execute(self, *, connection: _PostgresConnection, sql: str) -> Any:
        log_sql(logger=logging.getLogger("sqlbuild.adapter.postgres"), sql=sql)
        return connection.execute(sql)

    def close(self, connection: _PostgresConnection) -> None:
        connection.close()

    def default_schema(self) -> str:
        return "public"

    def default_database(self) -> str | None:
        return None

    def describe_relation(self, *, connection: Any, relation: str) -> tuple[ColumnInfo, ...]:
        parts: list[str] = relation.split(".")
        name: str = parts[-1]
        schema_and_relation_part_count: int = 2
        schema: str | None = parts[-2] if len(parts) >= schema_and_relation_part_count else None
        cursor: Any = connection.execute(
            "SELECT column_name, data_type FROM information_schema.columns "
            f"WHERE table_name = {_quote_sql_string(name)}"
            + (f" AND table_schema = {_quote_sql_string(schema)}" if schema else "")
            + " ORDER BY ordinal_position"
        )
        return tuple(ColumnInfo(name=row[0], type=row[1]) for row in cursor.fetchall())

    def render_create_table_as(self, *, destination: str, sql: str) -> tuple[str, ...]:
        return (
            f"DROP TABLE IF EXISTS {destination}",
            f"CREATE TABLE {destination} AS {sql}",
        )

    def render_rename(self, *, origin: str, destination: str) -> tuple[str, ...]:
        destination_name: str = destination.split(".")[-1]
        return (f"ALTER TABLE {origin} RENAME TO {destination_name}",)

    def render_rename_view(self, *, origin: str, destination: str) -> tuple[str, ...]:
        destination_name: str = destination.split(".")[-1]
        return (f"ALTER VIEW {origin} RENAME TO {destination_name}",)

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
        if replace:
            self.drop(
                connection=connection,
                destination=destination,
                if_exists=True,
                statement_recorder=statement_recorder,
            )
        column_defs: str = ", ".join(f"{col.name} {col.type}" for col in columns)
        create_sql: str = f"CREATE TABLE {destination} ({column_defs})"
        statement_recorder.record(create_sql)
        self.execute(connection=connection, sql=create_sql)

        column_names: tuple[str, ...] = tuple(col.name for col in columns)
        placeholders: str = ", ".join(["%s"] * len(column_names))
        insert_sql: str = (
            f"INSERT INTO {destination} ({', '.join(column_names)}) VALUES ({placeholders})"
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
                if row is None:
                    continue
                rows.append(
                    tuple(
                        normalize_seed_csv_value(
                            value=row.get(col), column_name=col, csv_settings=csv_settings
                        )
                        for col in column_names
                    )
                )
        if not rows:
            return
        statement_recorder.record(insert_sql)
        cursor: Any = connection.cursor()
        try:
            cursor.executemany(insert_sql, rows)
        finally:
            cursor.close()

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
        immutable_columns: frozenset[str] = frozenset(
            column.lower() for column in (*keys, *exclude_columns)
        )
        update_columns: tuple[str, ...] = tuple(
            col for col in source_columns if col.lower() not in immutable_columns
        )
        col_list: str = ", ".join(self.render_identifier(column) for column in source_columns)
        key_match_sql: str = " AND ".join(
            f"__target.{self.render_identifier(key)} = __source.{self.render_identifier(key)}"
            for key in keys
        )
        source_select_sql: str = f"({sql}) AS __source"
        if update_columns:
            update_set: str = ", ".join(
                f"{self.render_identifier(col)} = __source.{self.render_identifier(col)}"
                for col in update_columns
            )
            update_sql: str = (
                f"UPDATE {destination} AS __target SET {update_set} "
                f"FROM {source_select_sql} WHERE {key_match_sql}"
            )
            statement_recorder.record(update_sql)
            self.execute(connection=connection, sql=update_sql)
        insert_sql: str = (
            f"INSERT INTO {destination} ({col_list}) "
            f"SELECT {col_list} FROM {source_select_sql} "
            f"WHERE NOT EXISTS (SELECT 1 FROM {destination} AS __target WHERE {key_match_sql})"
        )
        statement_recorder.record(insert_sql)
        insert_cursor: Any = self.execute(connection=connection, sql=insert_sql)
        return self.affected_row_count(cursor=insert_cursor)

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
        statements: tuple[str, ...] = self.render_alter_column_types(
            destination=destination, columns=columns
        )
        statement_recorder.record_many(statements)
        for stmt in statements:
            self.execute(connection=connection, sql=stmt)

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
        """Read the planner's ``reltuples`` estimate, treating unanalyzed data as unknown."""

        del database
        schema_filter: str = (
            "namespace.nspname = current_schema()"
            if schema is None
            else f"namespace.nspname = {_quote_sql_string(schema)}"
        )
        row: tuple[Any, ...] | None = self.execute(
            connection=connection,
            sql=(
                "SELECT relation.relkind, relation.reltuples, pg_relation_size(relation.oid) "
                "FROM pg_class AS relation "
                "JOIN pg_namespace AS namespace ON namespace.oid = relation.relnamespace "
                f"WHERE {schema_filter} AND relation.relname = {_quote_sql_string(name)}"
            ),
        ).fetchone()
        if row is None:
            return RelationRowCountEstimate(row_count=None, detail="relation not found")
        if str(row[0]) not in ROW_COUNT_RELATION_KINDS:
            return RelationRowCountEstimate(
                row_count=None, detail="no table statistics (view or partitioned table)"
            )
        estimated_rows: float = float(row[1])
        row_count: int | None = None
        if estimated_rows > 0:
            row_count = int(estimated_rows)
        elif int(row[2]) == 0:
            row_count = 0
        return RelationRowCountEstimate(
            row_count=row_count,
            detail="table statistics not collected; run ANALYZE" if row_count is None else None,
            byte_count=int(row[2]) if metadata_only else None,
            byte_count_basis="pg_relation_size_main_fork" if metadata_only else None,
        )

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
        for row in rows:
            sample: list[tuple[str, object]] = []
            for index, key in enumerate(keys):
                sample.append((key, row[index]))
            samples.append(tuple(sample))
        return tuple(samples)

    def normalize_row_diff_numeric_type(self, column_type: str) -> str | None:
        return normalize_numeric_family(type_sql=column_type, dialect=self.sql_analysis_dialect())

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
