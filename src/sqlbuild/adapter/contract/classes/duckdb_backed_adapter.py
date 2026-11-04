"""DuckDB-backed adapter infrastructure."""

from __future__ import annotations

import importlib.util
import json
import logging
import re
import time
from datetime import date, datetime
from decimal import Decimal
from importlib.machinery import ModuleSpec
from pathlib import Path
from types import ModuleType
from typing import Any, ClassVar

from sqlbuild._native import (
    SeedCsvSettings,
    SourceFreshnessRecord,
    SqlLexicalSyntax,
    SqlValue,
    render_insert_source_freshness_records_sql,
)
from sqlbuild.adapter.contract._helpers.seed_csv import get_seed_csv_null_values
from sqlbuild.adapter.contract.classes.base_adapter import (
    BaseAdapter,
    _encode_typed_json,
    _render_ansi_typed_scalar,
    _render_typed_value_list,
)
from sqlbuild.adapter.contract.classes.historical_check_snapshot_sql import (
    HistoricalCheckSnapshotSql,
)
from sqlbuild.adapter.contract.classes.historical_timestamp_snapshot_sql import (
    HistoricalTimestampSnapshotSql,
)
from sqlbuild.adapter.contract.classes.observed_connection import ObservedConnection
from sqlbuild.adapter.contract.classes.snapshot_sql import SnapshotSql
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.classes.unkeyed_diff import UnkeyedDiffMixin
from sqlbuild.adapter.contract.constants import (
    CANCELLABLE_DURATION_LIMIT_SECONDS,
    DIFF_LEFT_SIDE,
    DIFF_RIGHT_SIDE,
    DUCKDB_INTERNAL_ERROR_CLASS_NAME,
    DUCKDB_MISSING_OBJECT_MARKER,
    DUCKDB_NOT_FOUND_ERROR_CLASS_NAMES,
    DUCKDB_STORAGE_VERSION_KEY,
    LATEST_DUCKDB_STORAGE_VERSION,
    QUALIFIED_NAME_SEPARATOR,
)
from sqlbuild.adapter.contract.exceptions import AdapterUserError, ViewRenameRefusedError
from sqlbuild.adapter.contract.main.column_name_key import column_name_key
from sqlbuild.adapter.contract.main.is_quoted_identifier import is_quoted_identifier
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
    TableFreshnessMetadata,
    TableFreshnessRequest,
    TemporaryFunction,
)
from sqlbuild.adapter.contract.types import (
    CursorKind,
    FrameworkType,
    LoaderLogicalType,
    MigrationTransfer,
    PromotionStrategy,
    RelationReadStatus,
    StatementSizeLimit,
    TablePromotionMode,
)
from sqlbuild.adapter.relations.main.cached_view_definition import cached_view_definition
from sqlbuild.adapter.relations.main.get_columns_for_relations import (
    get_columns_for_relations_bulk,
)
from sqlbuild.adapter.type_system.main.first_arg_nullability import first_arg_nullability
from sqlbuild.adapter.type_system.main.normalize_numeric_family import normalize_numeric_family
from sqlbuild.adapter.type_system.main.types_equal import types_equal
from sqlbuild.compiler.compile.types import FunctionLanguage
from sqlbuild.diagnostics.classes.diagnostic_record_redactor import DiagnosticRecordRedactor
from sqlbuild.diagnostics.main.log_debug_event import log_debug_event
from sqlbuild.diagnostics.main.log_sql import log_sql
from sqlbuild.runtime.user_code.main.user_code_spec import user_code_spec
from sqlbuild.spec.contracts.constants import DEFAULT_SEED_CSV_SETTINGS


class DuckDbBackedAdapter(UnkeyedDiffMixin, BaseAdapter):
    """Shared adapter implementation for DuckDB-backed connections."""

    def get_columns_for_relations(
        self,
        *,
        connection: Any,
        relations: tuple[RelationInfo, ...],
    ) -> dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]]:
        return get_columns_for_relations_bulk(
            adapter=self, connection=connection, relations=relations
        )

    sql_analysis_dialect_name: ClassVar[str | None] = "duckdb"
    sql_lexical_syntax: ClassVar[SqlLexicalSyntax] = SqlLexicalSyntax(
        escape_string_prefix=True, nested_block_comments=True
    )
    relation_grants_supported: ClassVar[bool] = False

    execution_duration_limit_seconds: ClassVar[int | None] = CANCELLABLE_DURATION_LIMIT_SECONDS

    def create_table_as_replaces(self) -> bool:
        return True

    def create_sql_test_output(self, *, connection: Any, sql: str, name: str) -> str | None:
        """Store a unit test's output in a temporary table; compare inline on an engine bug."""

        try:
            _ = self.execute(connection=connection, sql=f"CREATE TEMPORARY TABLE {name} AS {sql}")
        except Exception as error:
            if type(error).__name__ != DUCKDB_INTERNAL_ERROR_CLASS_NAME:
                raise
            log_debug_event(
                logger=logging.getLogger("sqlbuild.adapter.duckdb"),
                message="DuckDB could not store a SQL test output; comparing inline",
                sqlbuild_error=DiagnosticRecordRedactor.text(str(error)),
            )
            return None
        return name

    def drop_sql_test_output(self, *, connection: Any, relation: str) -> None:
        """Drop the temporary table `create_sql_test_output` created."""

        _ = self.execute(connection=connection, sql=f"DROP TABLE IF EXISTS {relation}")

    def interrupt_connection(self, connection: Any) -> bool:
        """Cancel a connection's running statement from another thread; False if unsupported."""

        connection.interrupt()
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
        """Return False because the DuckDB and MotherDuck catalogs record no relation timestamps."""

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
        return (
            RelationReadStatus.MISSING
            if type(error).__name__ in DUCKDB_NOT_FOUND_ERROR_CLASS_NAMES
            and DUCKDB_MISSING_OBJECT_MARKER in str(error)
            else None
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

    def maximum_identifier_length(self) -> int:
        """Return the maximum unqualified identifier length supported by the adapter."""

        return self.max_identifier_length

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
            f"WHERE schema_name = {_duckdb_string_literal(schema)}"
        )
        if database is not None:
            query += f" AND catalog_name = {_duckdb_string_literal(database)}"
        cursor: Any = self.execute(connection=connection, sql=query)
        return cursor.fetchone() is not None

    def render_create_schema(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        target: str = f"{database}.{schema}" if database is not None else schema
        return (f"CREATE SCHEMA IF NOT EXISTS {target}",)

    def render_drop_schema(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        target: str = f"{database}.{schema}" if database is not None else schema
        return (f"DROP SCHEMA IF EXISTS {target} CASCADE",)

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
        if not allow_copy_fallback:
            raise AdapterUserError(message="DuckDB relation move/copy requires --allow-copy")
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

    def render_drop_view(self, *, destination: str, if_exists: bool = True) -> tuple[str, ...]:
        exists_clause: str = " IF EXISTS" if if_exists else ""
        return (f"DROP VIEW{exists_clause} {destination}",)

    def render_replace_table_from_relation(
        self, *, destination: str, origin: str
    ) -> tuple[str, ...]:
        return self.render_create_table_as(destination=destination, sql=f"SELECT * FROM {origin}")

    def render_current_timestamp(self) -> str:
        return "CURRENT_TIMESTAMP"

    def render_current_utc_timestamp(self) -> str:
        return "CAST(get_current_timestamp() AT TIME ZONE 'UTC' AS TIMESTAMP)"

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

    def default_database(self) -> str | None:
        """Return None; DuckDB-backed adapters default only schema."""

        return None

    def star_exclude_keyword(self) -> str:
        """Return the SQL keyword for SELECT * EXCLUDE/EXCEPT syntax."""

        return "EXCLUDE"

    def sql_analysis_dialect(self) -> str | None:
        """Return the SQL analysis dialect name for this adapter, if any."""

        return self.sql_analysis_dialect_name

    def default_table_promotion_mode(self) -> TablePromotionMode:
        """Return staged as the generic default promotion mode."""

        return TablePromotionMode.STAGED

    def default_promotion_strategy(self) -> PromotionStrategy:
        """Return atomic swap as the generic staged promotion strategy."""

        return PromotionStrategy.ATOMIC_SWAP

    def render_identifier(self, name: str) -> str:
        """Render one DuckDB identifier using double-quote escaping."""

        if is_quoted_identifier(name):
            return name
        return '"' + name.replace('"', '""') + '"'

    def render_exact_identifier(self, name: str) -> str:
        """Render one explicitly quoted DuckDB identifier with its exact case."""

        return self.render_identifier(name)

    def render_loader_logical_type(self, type_name: LoaderLogicalType) -> str:
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
        return _render_ansi_typed_scalar(value=value)

    def render_typed_value_list(self, *, value: SqlValue) -> str:
        return _render_typed_value_list(value=value, render_scalar=self.render_typed_scalar)

    def render_typed_array(self, *, value: SqlValue) -> str:
        return "[" + self._render_typed_array_items(value) + "]"

    def render_typed_object(self, *, value: SqlValue) -> str:
        return f"json({self._quote_sql_string(_encode_typed_json(value))})"

    def render_loader_value_literal(
        self, *, value: object, logical_type: LoaderLogicalType | None
    ) -> str:
        if value is None:
            return "NULL"
        if isinstance(value, bool):
            return "TRUE" if value else "FALSE"
        if isinstance(value, int | float | Decimal):
            return str(value)
        if isinstance(value, datetime | date):
            return self._quote_sql_string(value.isoformat())
        if isinstance(value, dict | list):
            return self._quote_sql_string(json.dumps(value, sort_keys=True))
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
        return self.render_create_table_as(
            destination=destination,
            sql=HistoricalTimestampSnapshotSql(
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
            ),
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
        return self.render_create_table_as(
            destination=destination,
            sql=HistoricalTimestampSnapshotSql.changes_initial_select_sql(
                origin=origin,
                unique_key=unique_key,
                updated_at_column=updated_at_column,
                valid_from_column=valid_from_column,
                valid_to_column=valid_to_column,
                output_columns=output_columns,
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
        return self.render_create_table_as(
            destination=destination,
            sql=HistoricalCheckSnapshotSql(dialect=self._snapshot_sql_dialect).initial_select_sql(
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

    def max_statement_size(self) -> StatementSizeLimit | None:
        """No practical SQL text cap: https://duckdb.org/docs/stable/operations_manual/limits."""

        return None

    def default_schema(self) -> str:
        """DuckDB uses 'main' as its default schema."""
        return "main"

    def render_qualified_name(
        self,
        *,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> str | None:
        """Render DuckDB relation names using generic dot qualification."""

        if database is not None and schema is not None:
            return f"{database}.{schema}.{name}"
        if schema is not None:
            return f"{schema}.{name}"
        return None

    def render_framework_type(self, type_name: FrameworkType) -> str:
        """Render DuckDB internal framework types explicitly."""

        match type_name:
            case FrameworkType.STRING:
                return "VARCHAR"
            case FrameworkType.INTEGER:
                return "BIGINT"
            case FrameworkType.TIMESTAMP:
                return "TIMESTAMP"

    def render_source_expression_cast(
        self, *, expression: str, target_type: str, alias: str
    ) -> str:
        """Render DuckDB source expression type-enforcement casts explicitly."""

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
        del source_is_subquery
        return (
            f"SELECT MAX({self.render_identifier(column)}) AS data_version "
            f"FROM {source_relation}{where_sql}"
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

    def requires_derived_table_aliases(self) -> bool:
        """DuckDB does not require aliases for derived table factors."""

        return False

    def render_set_difference_operator(self) -> str:
        """Render DuckDB set-difference operator explicitly."""

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

    def render_create_fingerprint_index_sqls(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        del database, schema
        return ()

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
        del database, schema
        return ()

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
        del database, schema
        return ()

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
        del database, schema
        return ()

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
            f"DELETE FROM {table_name} WHERE rowid IN ("
            "SELECT rowid FROM ("
            "SELECT rowid, ROW_NUMBER() OVER ("
            "PARTITION BY node_type, node_name "
            "ORDER BY ts DESC, run_id DESC"
            f") AS __sqlbuild_history_rank FROM {table_name}"
            ") AS __sqlbuild_ranked "
            f"WHERE __sqlbuild_history_rank > {retain_versions}"
            ")"
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
            f"DELETE FROM {table_name} WHERE rowid IN ("
            "SELECT rowid FROM ("
            "SELECT rowid, ROW_NUMBER() OVER ("
            "PARTITION BY source_name, target_database, target_schema, target_name "
            "ORDER BY observed_at DESC, run_id DESC"
            f") AS __sqlbuild_history_rank FROM {table_name}"
            ") AS __sqlbuild_ranked "
            f"WHERE __sqlbuild_history_rank > {retain_versions}"
            ")"
        )

    def expression_inference_profile(self) -> ExpressionInferenceProfile:
        return ExpressionInferenceProfile(
            sql_analysis_dialect=self.sql_analysis_dialect(),
            function_nullability_rules={
                "LOWER": first_arg_nullability,
                "UPPER": first_arg_nullability,
            },
        )

    def render_cursor_bound_literal(self, *, value: str, cursor_type: str | None) -> str:
        if cursor_type == CursorKind.INTEGER:
            return value
        if cursor_type == CursorKind.TIMESTAMP:
            return f"TIMESTAMP '{value}'"
        return f"'{value}'"

    def connect(self, config: dict[str, Any]) -> Any:
        """Open DuckDB, waiting up to five seconds for another process's database file lock."""

        import duckdb

        database: str = str(config.get("database", ":memory:"))
        deadline: float = time.monotonic() + 5
        while True:
            try:
                raw_connection: duckdb.DuckDBPyConnection = duckdb.connect(
                    database=database, read_only=bool(config.get("read_only", False))
                )
                break
            except duckdb.IOException as error:
                if (
                    not any(
                        marker in str(error)
                        for marker in (
                            ": Conflicting lock is held in ",
                            "\nFile is already open in ",
                        )
                    )
                    or time.monotonic() >= deadline
                ):
                    raise
                time.sleep(0.05)
        return self._configured_connection(raw_connection=raw_connection, config=config)

    def connect_local_replay(self, database: Path) -> Any:
        """Open sqlbuild's own local replay database at the latest DuckDB storage version."""

        import duckdb

        raw_connection: duckdb.DuckDBPyConnection = duckdb.connect(
            database=str(database),
            config={DUCKDB_STORAGE_VERSION_KEY: LATEST_DUCKDB_STORAGE_VERSION},
        )
        return self._configured_connection(raw_connection=raw_connection, config={})

    def _configured_connection(self, *, raw_connection: Any, config: dict[str, Any]) -> Any:
        """Wrap a DuckDB connection and apply the config's extensions, settings and attachments."""

        connection: ObservedConnection = ObservedConnection(
            raw_connection=raw_connection, adapter=self.adapter_name
        )

        extensions: list[str] | tuple[str, ...] = config.get("extensions", ())
        extension_name: str
        for extension_name in extensions:
            self.execute(connection=connection, sql=f"INSTALL '{extension_name}'")
            self.execute(connection=connection, sql=f"LOAD '{extension_name}'")

        settings: dict[str, object] = config.get("settings", {})
        setting_key: str
        setting_value: object
        for setting_key, setting_value in settings.items():
            self.execute(connection=connection, sql=f"SET {setting_key} = '{setting_value}'")

        attach_entries: list[dict[str, object]] = config.get("attach", [])
        attach_entry: dict[str, object]
        for attach_entry in attach_entries:
            self.execute(connection=connection, sql=self.duckdb_build_attach_sql(attach_entry))

        return connection

    def _execute(self, *, connection: Any, sql: str) -> Any:
        """Execute a SQL statement against a DuckDB connection."""

        log_sql(logger=logging.getLogger("sqlbuild.adapter.duckdb"), sql=sql)
        return connection.execute(sql)

    def query(self, *, connection: Any, sql: str, limit: int | None) -> QueryResult:
        """Execute SQL and return normalized rows for ad hoc query output."""

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
                columns=columns,
                rows=tuple(tuple(row) for row in cursor.fetchall()),
                column_types=column_types,
            )
        fetched_rows: list[tuple[object, ...]] = [tuple(row) for row in cursor.fetchmany(limit + 1)]
        return QueryResult(
            columns=columns,
            rows=tuple(fetched_rows[:limit]),
            truncated=len(fetched_rows) > limit,
            column_types=column_types,
        )

    def describe_relation(self, *, connection: Any, relation: str) -> tuple[ColumnInfo, ...]:
        """Return column metadata for a relation using DuckDB DESCRIBE."""

        cursor: Any = self.execute(connection=connection, sql=f"DESCRIBE {relation}")
        return tuple(ColumnInfo(name=row[0], type=row[1]) for row in cursor.fetchall())

    def query_column_names(self, *, connection: Any, sql: str) -> tuple[str, ...]:
        """Return DuckDB query column names using DESCRIBE SELECT."""

        cursor: Any = self.execute(connection=connection, sql=f"DESCRIBE {sql}")
        return tuple(str(row[0]) for row in cursor.fetchall())

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
            f"CAST('{maximum_allowed}' AS DATE)"
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
        """Build a DuckDB cursor filter clause."""

        if cursor_column is None or start_cursor is None:
            return ""
        clauses: list[str] = [f"{cursor_column} >= '{start_cursor.value}'"]
        if end_cursor is not None:
            clauses.append(f"{cursor_column} < '{end_cursor.value}'")
        return " AND ".join(clauses)

    def duckdb_build_attach_sql(self, attach_entry: dict[str, object]) -> str:
        """Build a DuckDB ATTACH statement from one attach config entry."""

        path: str = str(attach_entry["path"])
        sql: str = f"ATTACH IF NOT EXISTS '{path}'"
        alias: object | None = attach_entry.get("alias")
        if alias is not None:
            sql += f" AS {alias}"
        options: list[str] = []
        attach_type: object | None = attach_entry.get("type")
        if attach_type is not None:
            options.append(f"TYPE {attach_type}")
        read_only: object | None = attach_entry.get("read_only")
        if read_only is True:
            options.append("READ_ONLY")
        if options:
            sql += f" ({', '.join(options)})"
        return sql

    def close(self, connection: Any) -> None:
        """Close a DuckDB connection."""

        connection.close()

    def relation_exists(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> bool:
        query: str = (
            "SELECT 1 FROM information_schema.tables "
            f"WHERE table_name = {_duckdb_string_literal(name)}"
        )
        if schema is not None:
            query += f" AND table_schema = {_duckdb_string_literal(schema)}"
        else:
            query += (
                " AND lower(table_schema) IN "
                "(SELECT lower(search_schema) FROM "
                "(SELECT UNNEST(current_schemas(true)) AS search_schema))"
            )
        if database is not None:
            query += f" AND table_catalog = {_duckdb_string_literal(database)}"
        result: Any = self.execute(connection=connection, sql=query).fetchone()
        return result is not None

    def list_relations(
        self,
        *,
        connection: Any,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[RelationInfo, ...]:
        query: str = (
            "SELECT table_name, table_schema, table_type FROM information_schema.tables WHERE 1=1"
        )
        if schemas is not None:
            quoted: str = ", ".join(_duckdb_string_literal(schema) for schema in schemas)
            query += f" AND table_schema IN ({quoted})"
        if names:
            quoted_names: str = ", ".join(_duckdb_string_literal(name) for name in names)
            query += f" AND table_name IN ({quoted_names})"
        if database is not None:
            query += f" AND table_catalog = {_duckdb_string_literal(database)}"
        rows: list[tuple[Any, ...]] = self.execute(connection=connection, sql=query).fetchall()
        return tuple(
            RelationInfo(
                database=database,
                schema=row[1],
                name=row[0],
                relation_type=row[2],
            )
            for row in rows
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
            "SELECT function_name, schema_name, function_type FROM duckdb_functions() WHERE 1=1"
        )
        if schemas is not None:
            quoted: str = ", ".join(_duckdb_string_literal(schema) for schema in schemas)
            query += f" AND schema_name IN ({quoted})"
        if names:
            quoted_names: str = ", ".join(_duckdb_string_literal(name) for name in names)
            query += f" AND function_name IN ({quoted_names})"
        if database is not None:
            query += f" AND database_name = {_duckdb_string_literal(database)}"
        rows: list[tuple[Any, ...]] = self.execute(connection=connection, sql=query).fetchall()
        return tuple(
            FunctionInfo(
                database=database,
                schema=row[1],
                name=row[0],
                function_type=row[2],
            )
            for row in rows
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
            f"WHERE table_name = {_duckdb_string_literal(name)}"
        )
        if schema is not None:
            query += f" AND table_schema = {_duckdb_string_literal(schema)}"
        if database is not None:
            query += f" AND table_catalog = {_duckdb_string_literal(database)}"
        query += " ORDER BY ordinal_position"
        rows: list[tuple[Any, ...]] = self.execute(connection=connection, sql=query).fetchall()
        return tuple(ColumnInfo(name=row[0], type=row[1]) for row in rows)

    def get_all_columns(
        self,
        *,
        connection: Any,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> dict[str, tuple[ColumnInfo, ...]]:
        query: str = (
            "SELECT table_name, column_name, data_type FROM information_schema.columns WHERE 1=1"
        )
        if schemas is not None:
            quoted: str = ", ".join(_duckdb_string_literal(schema) for schema in schemas)
            query += f" AND table_schema IN ({quoted})"
        if names:
            quoted_names: str = ", ".join(_duckdb_string_literal(name) for name in names)
            query += f" AND table_name IN ({quoted_names})"
        if database is not None:
            query += f" AND table_catalog = {_duckdb_string_literal(database)}"
        query += " ORDER BY table_name, ordinal_position"
        rows: list[tuple[Any, ...]] = self.execute(connection=connection, sql=query).fetchall()
        result: dict[str, list[ColumnInfo]] = {}
        row: tuple[Any, ...]
        for row in rows:
            table_name: str = row[0]
            if table_name not in result:
                result[table_name] = []
            result[table_name].append(ColumnInfo(name=row[1], type=row[2]))
        return {k: tuple(v) for k, v in result.items()}

    def render_create_table_as(self, *, destination: str, sql: str) -> tuple[str, ...]:
        return (f"CREATE OR REPLACE TABLE {destination} AS {sql}",)

    def render_create_view_as(self, *, destination: str, sql: str) -> tuple[str, ...]:
        return (f"CREATE OR REPLACE VIEW {destination} AS {sql}",)

    def render_udf_call(self, *, target: str, call_suffix_sql: str) -> str:
        return f"{target}{call_suffix_sql}"

    def render_table_function_call(self, *, target: str, call_suffix_sql: str) -> str:
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
        if language == FunctionLanguage.PYTHON:
            if return_columns:
                raise AdapterUserError(message="DuckDB table functions must use SQL language")
            parameter_types: str = ", ".join(str(arg.type) for arg in arguments)
            return (f"REGISTER PYTHON FUNCTION {destination}({parameter_types}) RETURNS {returns}",)
        del returns
        argument_sql: str = ", ".join(str(arg.name) for arg in arguments)
        if return_columns:
            return (f"CREATE OR REPLACE MACRO {destination}({argument_sql}) AS TABLE\n{body_sql}",)
        return (f"CREATE OR REPLACE MACRO {destination}({argument_sql}) AS (\n{body_sql}\n)",)

    def render_create_temporary_function(
        self, *, name: str, definition: FunctionDefinition
    ) -> TemporaryFunction | None:
        """Copy a SQL function into a temporary macro; Python functions register per connection."""

        if definition.language != FunctionLanguage.SQL:
            return None
        identifier: str = self.render_identifier(name)
        argument_sql: str = ", ".join(str(arg.name) for arg in definition.arguments)
        body: str = (
            f"AS TABLE\n{definition.body_sql}"
            if definition.return_columns
            else f"AS (\n{definition.body_sql}\n)"
        )
        return TemporaryFunction(
            target=f"temp.main.{identifier}",
            statements=(f"CREATE OR REPLACE TEMP MACRO {identifier}({argument_sql}) {body}",),
        )

    def supports_python_functions(self) -> bool:
        return True

    def persists_python_functions(self) -> bool:
        return False

    def python_functions_inherit_default_namespace(self) -> bool:
        return False

    def supports_unqualified_function_fingerprints(self) -> bool:
        return True

    def supports_table_functions(self) -> bool:
        return True

    def create_function(
        self,
        *,
        connection: Any,
        definition: FunctionDefinition,
        statement_recorder: StatementRecorder,
    ) -> None:
        destination: str = definition.destination
        arguments: tuple[Any, ...] = definition.arguments
        returns: str = definition.returns
        body_sql: str = definition.body_sql
        return_columns: tuple[Any, ...] = definition.return_columns
        language: FunctionLanguage = definition.language
        runtime_version: str | None = definition.runtime_version
        entry_point: str | None = definition.entry_point
        packages: tuple[str, ...] = definition.packages
        source_file_path: Path | None = definition.source_file_path
        if language != FunctionLanguage.PYTHON:
            del source_file_path
            statements: tuple[str, ...] = self.render_create_function(
                destination=destination,
                arguments=arguments,
                returns=returns,
                body_sql=body_sql,
                return_columns=return_columns,
                language=language,
                runtime_version=runtime_version,
                entry_point=entry_point,
                packages=packages,
            )
            statement_recorder.record_many(statements)
            stmt: str
            for stmt in statements:
                self.execute(connection=connection, sql=stmt)
            return
        del body_sql, runtime_version, packages
        if source_file_path is None or entry_point is None:
            raise AdapterUserError(
                message="DuckDB Python UDFs require source_file_path and entry_point"
            )
        function_name: str = destination.split(".")[-1]
        if destination not in {function_name, f"main.{function_name}"}:
            raise AdapterUserError(
                message=(
                    f"DuckDB Python UDF '{function_name}' cannot set database or schema because "
                    "DuckDB registers Python UDFs as connection-scoped functions. Remove "
                    "database/schema from the UDF decorator or use SQL UDFs for schema-qualified "
                    "DuckDB functions."
                )
            )
        callable_function: Any = self._load_python_udf_callable(
            source_file_path=source_file_path,
            entry_point=entry_point,
        )
        parameter_types: list[str] = [argument.type for argument in arguments]
        registration_sql: str = (
            f"REGISTER PYTHON FUNCTION {function_name}({', '.join(parameter_types)}) "
            f"RETURNS {returns}"
        )
        statement_recorder.record(registration_sql)
        self.register_python_function(
            connection=connection,
            name=function_name,
            function=callable_function,
            parameter_types=parameter_types,
            returns=returns,
        )

    def register_python_function(
        self,
        *,
        connection: Any,
        name: str,
        function: Any,
        parameter_types: list[str],
        returns: str,
    ) -> None:
        """Register a Python function after finishing a partly read result, which DuckDB 2 needs."""

        connection.complete()
        connection.create_function(name, function, parameter_types, returns)

    def _load_python_udf_callable(self, *, source_file_path: Path, entry_point: str) -> Any:
        module_name: str = "sqlbuild_python_udf_" + "_".join(
            source_file_path.with_suffix("").parts[-4:]
        ).replace("-", "_")
        spec: ModuleSpec | None = user_code_spec(
            name=module_name, file_path=source_file_path, root=source_file_path.parent
        )
        if spec is None or spec.loader is None:
            raise AdapterUserError(message=f"Could not load Python UDF from '{source_file_path}'")
        module: ModuleType = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        udf_function: object = getattr(module, entry_point, None)
        if not callable(udf_function):
            raise AdapterUserError(
                message=(
                    f"Python UDF entry_point '{entry_point}' was not found in '{source_file_path}'"
                )
            )
        return udf_function

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
            f"WHERE {self.render_identifier(cursor_column)} >= '{cursor_start}' "
            f"AND {self.render_identifier(cursor_column)} < '{cursor_end}'"
        )
        insert_stmts: tuple[str, ...] = self.render_append(
            destination=destination, sql=sql, columns=columns
        )
        return (delete_sql, *insert_stmts)

    def render_drop(self, *, destination: str, if_exists: bool = True) -> tuple[str, ...]:
        exists_clause: str = " IF EXISTS" if if_exists else ""
        return (f"DROP TABLE{exists_clause} {destination}",)

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

    def render_rename(self, *, origin: str, destination: str) -> tuple[str, ...]:
        unqualified_destination: str = destination.rsplit(".", 1)[-1]
        return (f"ALTER TABLE {origin} RENAME TO {unqualified_destination}",)

    def render_rename_view(self, *, origin: str, destination: str) -> tuple[str, ...]:
        unqualified_destination: str = destination.rsplit(".", 1)[-1]
        return (f"ALTER VIEW {origin} RENAME TO {unqualified_destination}",)

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
        return _unquote_duckdb_identifier(relation.split(".")[-1])

    def _with_replaced_relation_name(self, *, relation: str, name: str) -> str:
        parts: list[str] = relation.split(".")
        parts[-1] = self.render_identifier(name)
        return ".".join(parts)

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
        if self._is_view(connection=connection, relation=left):
            raise ViewRenameRefusedError(message=f"DuckDB cannot swap the view {left} with a table")
        statements: tuple[str, ...] = self.render_swap(left=left, right=right)
        statement_recorder.record_many(statements)
        with self.transaction(connection):
            stmt: str
            for stmt in statements:
                self.execute(connection=connection, sql=stmt)

    def _is_view(self, *, connection: Any, relation: str) -> bool:
        """Whether a relation is a view; DuckDB 2 lets ALTER TABLE rename a view, so swap checks."""

        *qualifiers, name = [_unquote_duckdb_identifier(part) for part in relation.split(".")]
        schema_sql: str = (
            _duckdb_string_literal(qualifiers[-1]) if qualifiers else "current_schema()"
        )
        database_sql: str = (
            _duckdb_string_literal(qualifiers[-2]) if len(qualifiers) > 1 else "current_database()"
        )
        query: str = (
            "SELECT 1 FROM duckdb_views() WHERE NOT internal "
            f"AND view_name = {_duckdb_string_literal(name)} "
            f"AND schema_name = {schema_sql} AND database_name = {database_sql}"
        )
        return self.execute(connection=connection, sql=query).fetchone() is not None

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
        return False

    def list_dependent_view_names(
        self, *, connection: Any, database: str | None, schema: str, name: str
    ) -> tuple[str, ...]:
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
        prefetched: tuple[bool, str | None] = cached_view_definition(
            adapter=self, connection=connection, database=database, schema=schema, name=name
        )
        if prefetched[0]:
            return prefetched[1]
        database_filter: str = (
            "" if database is None else f" AND database_name = {_duckdb_string_literal(database)}"
        )
        rows: list[tuple[Any, ...]] = self.execute(
            connection=connection,
            sql=(
                "SELECT sql FROM duckdb_views() WHERE schema_name = "
                f"{_duckdb_string_literal(schema)} AND view_name = {_duckdb_string_literal(name)}"
                + database_filter
            ),
        ).fetchall()
        return None if not rows else str(rows[0][0])

    def read_view_definitions(
        self, *, connection: Any, database: str | None, schema: str, names: tuple[str, ...]
    ) -> dict[str, str]:
        """Read the stored definitions of several views in one schema with one query."""

        if not names:
            return {}
        database_filter: str = (
            "" if database is None else f" AND database_name = {_duckdb_string_literal(database)}"
        )
        rows: list[tuple[Any, ...]] = self.execute(
            connection=connection,
            sql=(
                "SELECT view_name, sql FROM duckdb_views() WHERE schema_name = "
                f"{_duckdb_string_literal(schema)} AND view_name IN ("
                + ", ".join(_duckdb_string_literal(name) for name in names)
                + ")"
                + database_filter
            ),
        ).fetchall()
        definitions: dict[str, str] = {}
        row: tuple[Any, ...]
        for row in rows:
            definitions.setdefault(str(row[0]), str(row[1]))
        return definitions

    def view_definition_matches(
        self, *, connection: Any, database: str | None, schema: str, name: str, sql: str
    ) -> bool:
        definition: str | None = self.read_view_definition(
            connection=connection, database=database, schema=schema, name=name
        )
        if definition is None:
            return False
        if same_view_definition(definition=definition, sql=sql):
            return True
        catalog: str = database if database is not None else self._current_database(connection)
        return same_view_definition(
            definition=_without_catalog_qualifier(text=definition, catalog=catalog),
            sql=_without_catalog_qualifier(text=sql, catalog=catalog),
        )

    def _current_database(self, connection: Any) -> str:
        row: Any = self.execute(connection=connection, sql="SELECT current_database()").fetchone()
        return str(row[0])

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
        """Load a seed CSV into a DuckDB table using read_csv."""

        if replace:
            self.drop(
                connection=connection,
                destination=destination,
                if_exists=True,
                statement_recorder=statement_recorder,
            )
        read_csv_options: str = self._render_seed_csv_options(csv_settings)
        if infer_types:
            stmt: str = (
                f"CREATE TABLE {destination} AS SELECT * FROM read_csv('{file_path}', "
                f"auto_detect=true{read_csv_options})"
            )
            statement_recorder.record(stmt)
            self.execute(connection=connection, sql=stmt)
            return
        column_defs: str = ", ".join(f"{col.name} {col.type}" for col in columns)
        type_map: str = ", ".join(f"'{col.name}': '{col.type}'" for col in columns)
        statements: tuple[str, ...] = (
            f"CREATE TABLE {destination} ({column_defs})",
            f"INSERT INTO {destination} SELECT * FROM read_csv('{file_path}', "
            f"columns={{{type_map}}}{read_csv_options})",
        )
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
    ) -> int | None:
        statements: tuple[str, ...] = self.render_append(
            destination=destination, sql=sql, columns=columns
        )
        statement_recorder.record_many(statements)
        affected: int | None = None
        stmt: str
        for stmt in statements:
            result: Any = self.execute(connection=connection, sql=stmt)
            affected = self.affected_row_count(cursor=result)
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
                result: Any = self.execute(connection=connection, sql=stmt)
                affected = self.affected_row_count(cursor=result)
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
                result: Any = self.execute(connection=connection, sql=stmt)
                affected = self.affected_row_count(cursor=result)
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
        affected: int | None = None
        stmt: str
        for stmt in statements:
            result: Any = self.execute(connection=connection, sql=stmt)
            affected = self.affected_row_count(cursor=result)
        return affected

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
                new_identifier=self.render_identifier(new_name),
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
        stmt: str
        for stmt in statements:
            self.execute(connection=connection, sql=stmt)

    def inspect_row_diff_coverage(
        self,
        *,
        connection: Any,
        relation: str,
        cursor_column: str | None = None,
        start_cursor: CursorValue | None = None,
        end_cursor: CursorValue | None = None,
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
        """Read DuckDB's estimated table size from the catalog."""

        database_filter: str = (
            "database_name = current_database()"
            if database is None
            else f"lower(database_name) = lower({_duckdb_string_literal(database)})"
        )
        schema_filter: str = (
            "schema_name = current_schema()"
            if schema is None
            else f"lower(schema_name) = lower({_duckdb_string_literal(schema)})"
        )
        row: tuple[Any, ...] | None = self.execute(
            connection=connection,
            sql=(
                "SELECT estimated_size FROM duckdb_tables() WHERE "
                f"{database_filter} AND {schema_filter} "
                f"AND lower(table_name) = lower({_duckdb_string_literal(name)})"
            ),
        ).fetchone()
        if row is None or row[0] is None:
            return RelationRowCountEstimate(
                row_count=None, detail="no table size metadata (view or missing table)"
            )
        return RelationRowCountEstimate(row_count=int(row[0]))

    def diff_schema(
        self,
        *,
        connection: Any,
        left: str,
        right: str,
    ) -> SchemaDiffResult:
        """Compare column metadata between two DuckDB relations."""

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

        col_name: str
        col_type: str
        for col_name, col_type in right_map.items():
            if col_name not in left_map:
                added.append(ColumnInfo(name=col_name, type=col_type))
            elif not types_equal(
                left=left_map[col_name],
                right=col_type,
                dialect=self.sql_analysis_dialect(),
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
        start_cursor: CursorValue | None = None,
        end_cursor: CursorValue | None = None,
    ) -> RowDiffResult:
        """Compare row-level data between two DuckDB relations."""

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
            connection=connection,
            relation_sql=left_cte,
            relation_label="left",
            keys=keys,
        )
        self.validate_row_diff_keys(
            connection=connection,
            relation_sql=right_cte,
            relation_label="right",
            keys=keys,
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
            f"AND NOT ({column_equal_expressions[col]}) THEN 1 END) "
            f"AS __{col}_mismatch_count"
            for col in compare_columns
        ]
        column_count_sql: str = ""
        if column_count_sql_parts:
            column_count_sql = ", " + ", ".join(column_count_sql_parts)

        diff_sql: str = (
            f"WITH {prepared_relations.cte_sql} "
            f"SELECT "
            f"COUNT(CASE WHEN __left.{keys[0]} IS NOT NULL THEN 1 END) AS left_count, "
            f"COUNT(CASE WHEN __right.{keys[0]} IS NOT NULL THEN 1 END) AS right_count, "
            f"COUNT(*) AS joined, "
            f"COUNT(CASE WHEN __left.{keys[0]} IS NOT NULL "
            f"AND __right.{keys[0]} IS NOT NULL AND ({equal_condition}) "
            f"THEN 1 END) AS equal, "
            f"COUNT(CASE WHEN __left.{keys[0]} IS NOT NULL "
            f"AND __right.{keys[0]} IS NOT NULL AND NOT ({equal_condition}) "
            f"THEN 1 END) AS unequal, "
            f"COUNT(CASE WHEN __right.{keys[0]} IS NULL THEN 1 END) AS left_only, "
            f"COUNT(CASE WHEN __left.{keys[0]} IS NULL THEN 1 END) AS right_only"
            f"{column_count_sql} "
            f"FROM __left FULL OUTER JOIN __right ON {join_condition}"
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
        start_cursor: CursorValue | None = None,
        end_cursor: CursorValue | None = None,
        limit: int = 20,
    ) -> tuple[RowDiffSampleRow, ...]:
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
            connection=connection,
            relation_sql=left_cte,
            relation_label="left",
            keys=keys,
        )
        prepared_relations: RowDiffPreparedRelations = self._build_row_diff_relation_ctes(
            connection=connection,
            left_sql=left_cte,
            right_sql=right_cte,
            keys=keys,
            sampling=sampling,
            inspect_population=False,
        )
        self.validate_row_diff_keys(
            connection=connection,
            relation_sql=right_cte,
            relation_label="right",
            keys=keys,
        )
        column_equal_expressions: dict[str, str] = {
            col: self.build_row_diff_equal_expression(
                column=col,
                column_info=left_columns_by_name[col],
                tolerances=tolerances,
            )
            for col in compare_columns
        }
        unequal_condition: str = "FALSE"
        if compare_columns:
            unequal_condition = " OR ".join(
                f"NOT ({expression})" for expression in column_equal_expressions.values()
            )
        key_select_sql: str = ", ".join(
            f"COALESCE(__left.{key}, __right.{key}) AS __key_{key}" for key in keys
        )
        compare_select_sql: str = ", ".join(
            f"__left.{column} AS __left_{column}, __right.{column} AS __right_{column}"
            for column in compare_columns
        )
        if compare_select_sql:
            compare_select_sql = ", " + compare_select_sql
        join_condition: str = " AND ".join(f"__left.{k} = __right.{k}" for k in keys)
        sample_sql: str = (
            f"WITH {prepared_relations.cte_sql} "
            f"SELECT {key_select_sql}{compare_select_sql} "
            f"FROM __left FULL OUTER JOIN __right ON {join_condition} "
            f"WHERE __left.{keys[0]} IS NOT NULL AND __right.{keys[0]} IS NOT NULL "
            f"AND ({unequal_condition}) "
            f"ORDER BY {', '.join(f'__key_{key}' for key in keys)} LIMIT {limit}"
        )
        rows: list[tuple[Any, ...]] = self.execute(connection=connection, sql=sample_sql).fetchall()
        samples: list[RowDiffSampleRow] = []
        row: tuple[Any, ...]
        for row in rows:
            key_values: tuple[tuple[str, object], ...] = tuple(
                (key, row[index]) for index, key in enumerate(keys)
            )
            changed_cells: list[RowDiffSampleCell] = []
            column_index: int
            column: str
            for column_index, column in enumerate(compare_columns):
                left_value_index: int = len(keys) + (column_index * 2)
                right_value_index: int = left_value_index + 1
                left_value: object = row[left_value_index]
                right_value: object = row[right_value_index]
                if left_value != right_value:
                    changed_cells.append(
                        RowDiffSampleCell(
                            name=column,
                            left_value=left_value,
                            right_value=right_value,
                        )
                    )
            samples.append(
                RowDiffSampleRow(
                    key_values=key_values,
                    changed_cells=tuple(changed_cells),
                )
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
        start_cursor: CursorValue | None = None,
        end_cursor: CursorValue | None = None,
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
            connection=connection,
            relation_sql=left_cte,
            relation_label="left",
            keys=keys,
        )
        prepared_relations: RowDiffPreparedRelations = self._build_row_diff_relation_ctes(
            connection=connection,
            left_sql=left_cte,
            right_sql=right_cte,
            keys=keys,
            sampling=sampling,
            inspect_population=False,
        )
        self.validate_row_diff_keys(
            connection=connection,
            relation_sql=right_cte,
            relation_label="right",
            keys=keys,
        )
        join_condition: str = " AND ".join(f"__left.{k} = __right.{k}" for k in keys)
        key_select_sql: str = ", ".join(
            f"COALESCE(__left.{key}, __right.{key}) AS __key_{key}" for key in keys
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
            f"ORDER BY {', '.join(f'__key_{key}' for key in keys)} LIMIT {limit}"
        )
        rows: list[tuple[Any, ...]] = self.execute(connection=connection, sql=sample_sql).fetchall()
        samples: list[tuple[tuple[str, object], ...]] = []
        for row in rows:
            sample: list[tuple[str, object]] = []
            for index, key in enumerate(keys):
                sample.append((key, row[index]))
            samples.append(tuple(sample))
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
        return normalize_numeric_family(type_sql=column_type, dialect=self.sql_analysis_dialect())

    def format_row_diff_decimal_sql(self, value: Decimal) -> str:
        return format(value, "f")

    def _render_seed_csv_options(self, csv_settings: SeedCsvSettings) -> str:
        options: list[str] = []
        string_options: dict[str, str | None] = {
            "delim": csv_settings.delimiter,
            "quote": csv_settings.quotechar,
            "escape": csv_settings.escapechar,
            "encoding": csv_settings.encoding,
            "new_line": csv_settings.lineterminator,
        }
        option_name: str
        option_value: str | None
        for option_name, option_value in string_options.items():
            if option_value is not None:
                options.append(f"{option_name}='{self._duckdb_string_literal(option_value)}'")
        seed_null_values: tuple[str, ...] = get_seed_csv_null_values(csv_settings)
        if seed_null_values:
            null_values: str = ", ".join(
                f"'{self._duckdb_string_literal(value)}'" for value in seed_null_values
            )
            options.append(f"nullstr=[{null_values}]")
        if not options:
            return ""
        return ", " + ", ".join(options)

    def _duckdb_string_literal(self, value: str) -> str:
        return value.replace("'", "''")


def _without_catalog_qualifier(*, text: str, catalog: str) -> str:
    """Drop a view's own catalog prefix, which DuckDB 2 leaves out of stored view SQL."""

    quoted_catalog: str = re.escape('"' + catalog.replace('"', '""') + '"')
    pattern: re.Pattern[str] = re.compile(
        rf"""(?<![\w."])(?:{quoted_catalog}|{re.escape(catalog)})\.(?=[\w"])""",
        re.IGNORECASE,
    )
    return pattern.sub("", text)


def _duckdb_string_literal(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"


def _unquote_duckdb_identifier(part: str) -> str:
    stripped: str = part.strip()
    quoted_identifier_character_count: int = 2
    if (
        stripped.startswith('"')
        and stripped.endswith('"')
        and len(stripped) >= quoted_identifier_character_count
    ):
        return stripped[1:-1].replace('""', '"')
    return stripped
