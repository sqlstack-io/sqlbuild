"""Microsoft SQL Server adapter implementation."""

from __future__ import annotations

import json
import logging
import re
from datetime import date, datetime
from decimal import Decimal
from pathlib import Path
from typing import Any, ClassVar

from sqlbuild._native import (
    SeedCsvSettings,
    SourceFreshnessRecord,
    SqlLexicalSyntax,
    SqlValue,
    SqlValueKind,
    render_insert_source_freshness_records_sql,
)
from sqlbuild.adapter.contract.classes.base_adapter import (
    BaseAdapter,
    _build_names_filter,
    _build_schemas_filter,
    _encode_typed_json,
    _quote_sql_string,
    _render_ansi_typed_scalar,
    _render_typed_value_list,
    _typed_scalar_payload,
)
from sqlbuild.adapter.contract.classes.consumption import ConsumptionMixin
from sqlbuild.adapter.contract.classes.historical_check_snapshot_sql import (
    HistoricalCheckSnapshotSql,
)
from sqlbuild.adapter.contract.classes.historical_snapshot_sql import (
    HistoricalSnapshotSql,
)
from sqlbuild.adapter.contract.classes.historical_timestamp_snapshot_sql import (
    HistoricalTimestampSnapshotSql,
)
from sqlbuild.adapter.contract.classes.microbatch import MicrobatchMixin
from sqlbuild.adapter.contract.classes.snapshot_sql import SnapshotSql
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.classes.unkeyed_diff import UnkeyedDiffMixin
from sqlbuild.adapter.contract.constants import (
    CANCELLABLE_DURATION_LIMIT_SECONDS,
    DIFF_LEFT_SIDE,
    DIFF_RIGHT_SIDE,
)
from sqlbuild.adapter.contract.exceptions import (
    AdapterUserError,
    UnsupportedTypedSqlRenderingError,
)
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
from sqlbuild.adapters.sqlserver._helpers.grants import (
    render_sqlserver_view_grants,
    render_sqlserver_view_revokes,
    sqlserver_relation_grants,
)
from sqlbuild.adapters.sqlserver.classes.sqlserver_connection import _SqlServerConnection
from sqlbuild.adapters.sqlserver.constants import (
    BOOLEAN_RETURN_TYPE,
    DIFF_CHARACTER_LENGTH_TYPES,
    DIFF_DATETIME_PRECISION_TYPES,
    DIFF_NUMERIC_PRECISION_TYPES,
    DIFF_UNSUPPORTED_COMPARISON_TYPES,
    INFORMATION_SCHEMA_NULLABLE_VALUE,
    INTEGER_TYPE_TOKEN,
    ISO8601_CONVERT_STYLE,
    TABLE_FUNCTION_RETURN_TYPE,
    USER_TABLE_OBJECT_TYPE,
)
from sqlbuild.compiler.compile.types import FunctionLanguage
from sqlbuild.diagnostics.main.log_sql import log_sql
from sqlbuild.spec.contracts.constants import DEFAULT_SEED_CSV_SETTINGS


def _render_tsql_distinct_condition(*, left: str, right: str) -> str:
    return (
        f"({left} <> {right} "
        f"OR ({left} IS NULL AND {right} IS NOT NULL) "
        f"OR ({left} IS NOT NULL AND {right} IS NULL))"
    )


class SqlServerAdapter(MicrobatchMixin, ConsumptionMixin, UnkeyedDiffMixin, BaseAdapter):
    """Microsoft SQL Server adapter backed by pymssql."""

    def get_columns_for_relations(
        self,
        *,
        connection: Any,
        relations: tuple[RelationInfo, ...],
    ) -> dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]]:
        return get_columns_for_relations_bulk(
            adapter=self, connection=connection, relations=relations
        )

    execution_duration_limit_seconds: ClassVar[int | None] = CANCELLABLE_DURATION_LIMIT_SECONDS
    adapter_name: ClassVar[str] = BuiltinAdapter.SQLSERVER.value
    sql_analysis_dialect_name: ClassVar[str | None] = "tsql"
    sql_lexical_syntax: ClassVar[SqlLexicalSyntax] = SqlLexicalSyntax(nested_block_comments=True)
    max_identifier_length: ClassVar[int] = 128
    _snapshot_sql_dialect: ClassVar[SnapshotSqlDialect] = SnapshotSqlDialect(
        timestamp_type="DATETIME2",
        distinct_condition=_render_tsql_distinct_condition,
        update_style=SnapshotUpdateStyle.TSQL,
        latest_version=SnapshotLatestVersionStyle.ORDERED_CTE,
        historical_close=HistoricalSnapshotCloseStyle.TSQL,
        historical_insert=HistoricalSnapshotInsertStyle.TSQL,
    )

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
        invalid_object_name: int = 208
        not_found: bool = bool(error.args) and error.args[0] == invalid_object_name
        return RelationReadStatus.MISSING if not_found else None

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

    def _distinct_condition(self, *, left: str, right: str) -> str:
        return _render_tsql_distinct_condition(left=left, right=right)

    def _lag_expr(self, *, column: str, partition_sql: str, observed_at_column: str) -> str:
        return (
            f"LAG({column}) OVER (PARTITION BY {partition_sql} "
            f"ORDER BY {observed_at_column}) AS __prev_{column}"
        )

    def connect(self, config: dict[str, Any]) -> _SqlServerConnection:
        try:
            import pymssql
        except ImportError as error:
            raise AdapterUserError(
                message="SQL Server adapter requires optional dependency pymssql. "
                "Install with: pip install sqlbuild[sqlserver]",
                code="A401",
            ) from error

        raw_connection: Any = pymssql.connect(
            server=str(config.get("host", config.get("server", "localhost"))),
            port=str(config.get("port", 1433)),
            user=str(config.get("user", config.get("username", "sa"))),
            password=str(config.get("password", "")),
            database=str(config.get("database", config.get("dbname", "master"))),
            autocommit=True,
        )
        return _SqlServerConnection(raw_connection)

    def _execute(self, *, connection: _SqlServerConnection, sql: str) -> Any:
        log_sql(logger=logging.getLogger("sqlbuild.adapter.sqlserver"), sql=sql)
        return connection.execute(sql)

    def close(self, connection: _SqlServerConnection) -> None:
        connection.close()

    def begin(self, connection: Any) -> None:
        self.execute(connection=connection, sql="BEGIN TRANSACTION")

    def commit(self, connection: Any) -> None:
        self.execute(connection=connection, sql="COMMIT TRANSACTION")

    def rollback(self, connection: Any) -> None:
        self.execute(connection=connection, sql="IF @@TRANCOUNT > 0 ROLLBACK TRANSACTION")

    def default_schema(self) -> str:
        return "dbo"

    def default_database(self) -> str | None:
        return None

    def query_column_names(self, *, connection: Any, sql: str) -> tuple[str, ...]:
        try:
            cursor: Any = self.execute(
                connection=connection, sql=f"SELECT TOP 0 * FROM ({sql}) AS __describe_source"
            )
            description: Any | None = getattr(cursor, "description", None)
            if description is not None:
                return tuple(str(column[0]) for column in description)
        except Exception:
            pass
        escaped_sql: str = sql.replace("'", "''")
        cursor = self.execute(
            connection=connection,
            sql="EXEC sys.sp_describe_first_result_set "
            f"@tsql = N'{escaped_sql}', @params = NULL, @browse_information_mode = 0",
        )
        return tuple(str(row[2]) for row in cursor.fetchall() if row[2] is not None)

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

    def query(self, *, connection: Any, sql: str, limit: int | None) -> QueryResult:
        if limit is not None:
            column_names: tuple[str, ...] = self.query_column_names(connection=connection, sql=sql)
            if column_names:
                select_list: str = ", ".join(self.render_identifier(name) for name in column_names)
                sql = f"SELECT TOP {limit + 1} {select_list} FROM ({sql}) AS __query_source"
        cursor: Any = self.execute(connection=connection, sql=sql)
        description: Any | None = getattr(cursor, "description", None)
        if description is None:
            return QueryResult()
        columns: tuple[str, ...] = tuple(str(column[0]) for column in description)
        if limit is None:
            return QueryResult(columns=columns, rows=tuple(tuple(row) for row in cursor.fetchall()))
        fetched_rows: list[tuple[object, ...]] = [tuple(row) for row in cursor.fetchall()]
        return QueryResult(
            columns=columns,
            rows=tuple(fetched_rows[:limit]),
            truncated=len(fetched_rows) > limit,
        )

    def render_create_schema(self, *, database: str | None, schema: str) -> tuple[str, ...]:
        del database
        escaped_schema: str = schema.replace("'", "''")
        return (
            "IF NOT EXISTS (SELECT 1 FROM sys.schemas WHERE name = "
            f"'{escaped_schema}') EXEC('CREATE SCHEMA {self.render_identifier(schema)}')",
        )

    def render_drop_schema(self, *, database: str | None, schema: str) -> tuple[str, ...]:
        del database
        raise AdapterUserError(
            f"the {self.adapter_name} adapter cannot drop schema '{schema}' with its contents",
            code="A714",
            help="sqb review needs a warehouse that can drop a schema it created",
        )

    def render_create_fingerprint_table_sql(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> str:
        from sqlbuild._native import (
            COLUMN_DEFINITION_B64,
            COLUMN_DEFINITION_HASH,
            COLUMN_EVENT_ID,
            COLUMN_METADATA_JSON_B64,
            COLUMN_NODE_NAME,
            COLUMN_NODE_TYPE,
            COLUMN_RUN_ID,
            COLUMN_SCHEMA_FINGERPRINT,
            COLUMN_TARGET_DATABASE,
            COLUMN_TARGET_NAME,
            COLUMN_TARGET_SCHEMA,
            COLUMN_TIMESTAMP,
            COLUMN_VERSION_HASH,
            FINGERPRINT_TABLE_NAME,
        )

        table_name: str | None = self.render_qualified_name(
            database=database,
            schema=schema,
            name=FINGERPRINT_TABLE_NAME,
        )
        if table_name is None:
            return ""
        create_sql: str = (
            f"CREATE TABLE {table_name} ("
            f"{COLUMN_NODE_TYPE} NVARCHAR(450) NOT NULL, "
            f"{COLUMN_NODE_NAME} NVARCHAR(450) NOT NULL, "
            f"{COLUMN_TARGET_DATABASE} NVARCHAR(450), "
            f"{COLUMN_TARGET_SCHEMA} NVARCHAR(450), "
            f"{COLUMN_TARGET_NAME} NVARCHAR(450), "
            f"{COLUMN_RUN_ID} NVARCHAR(450) NOT NULL, "
            f"{COLUMN_DEFINITION_HASH} NVARCHAR(450) NOT NULL, "
            f"{COLUMN_VERSION_HASH} NVARCHAR(450) NOT NULL, "
            f"{COLUMN_SCHEMA_FINGERPRINT} NVARCHAR(450) NOT NULL, "
            f"{COLUMN_DEFINITION_B64} NVARCHAR(MAX) NOT NULL, "
            f"{COLUMN_METADATA_JSON_B64} NVARCHAR(MAX) NOT NULL, "
            f"{COLUMN_TIMESTAMP} DATETIME2 NOT NULL, "
            f"{COLUMN_EVENT_ID} NVARCHAR(MAX)"
            f")"
        )
        return self._create_table_if_missing_sql(
            create_sql=create_sql,
            database=database,
            schema=schema,
            table_name=FINGERPRINT_TABLE_NAME,
        )

    def render_create_microbatch_state_table_sql(self, *, database: str | None, schema: str) -> str:
        from sqlbuild._native import build_microbatch_create_table_sql
        from sqlbuild.microbatches.constants import MICROBATCH_TABLE_NAME

        create_sql: str = build_microbatch_create_table_sql(
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
            render_framework_type=self.render_framework_type,
        ).replace("CREATE TABLE IF NOT EXISTS ", "CREATE TABLE ", 1)
        return self._create_table_if_missing_sql(
            create_sql=create_sql,
            database=database,
            schema=schema,
            table_name=MICROBATCH_TABLE_NAME,
        )

    def render_create_fingerprint_index_sqls(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        from sqlbuild._native import FINGERPRINT_TABLE_NAME

        return self._state_index_sqls(
            table=(database, schema, FINGERPRINT_TABLE_NAME),
            index_name="_sqlbuild_fingerprints_latest_idx",
            columns="node_type, node_name, ts DESC, run_id DESC",
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
            "WITH __sqlbuild_ranked AS ("
            f"SELECT *, ROW_NUMBER() OVER ("
            "PARTITION BY node_type, node_name "
            "ORDER BY ts DESC, run_id DESC"
            f") AS __sqlbuild_history_rank FROM {table_name}"
            ") "
            f"DELETE FROM __sqlbuild_ranked WHERE __sqlbuild_history_rank > {retain_versions}"
        )

    def render_create_source_freshness_table_sql(self, *, database: str | None, schema: str) -> str:
        from sqlbuild._native import (
            SOURCE_FRESHNESS_TABLE_NAME,
            build_source_freshness_create_table_sql,
        )

        return self._keyed_state_table_sql(
            create_sql=build_source_freshness_create_table_sql(
                database=database,
                schema=schema,
                render_qualified_name=self.render_qualified_name,
                render_framework_type=self.render_framework_type,
            ),
            table=(database, schema, SOURCE_FRESHNESS_TABLE_NAME),
            key_columns=(
                "source_name",
                "target_database",
                "target_schema",
                "target_name",
                "run_id",
            ),
        )

    def render_create_source_freshness_index_sqls(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        from sqlbuild._native import SOURCE_FRESHNESS_TABLE_NAME

        return self._state_index_sqls(
            table=(database, schema, SOURCE_FRESHNESS_TABLE_NAME),
            index_name="_sqlbuild_source_freshness_latest_idx",
            columns="source_name, target_database, target_schema, target_name, "
            "observed_at DESC, run_id DESC",
        )

    def render_create_consumption_table_sql(self, *, database: str | None, schema: str) -> str:
        from sqlbuild._native import CONSUMPTION_TABLE_NAME

        return self._keyed_state_table_sql(
            create_sql=super().render_create_consumption_table_sql(
                database=database, schema=schema
            ),
            table=(database, schema, CONSUMPTION_TABLE_NAME),
            key_columns=("target_schema", "target_name"),
        )

    def render_create_consumption_index_sqls(
        self, *, database: str | None, schema: str
    ) -> tuple[str, ...]:
        from sqlbuild._native import CONSUMPTION_TABLE_NAME

        return self._state_index_sqls(
            table=(database, schema, CONSUMPTION_TABLE_NAME),
            index_name="_sqlbuild_consumption_relation_idx",
            columns="target_schema, target_name, recorded_at",
        )

    def _keyed_state_table_sql(
        self,
        *,
        create_sql: str,
        table: tuple[str | None, str, str],
        key_columns: tuple[str, ...],
    ) -> str:
        """Create a state table once, with indexable NVARCHAR(450) key columns."""

        database, schema, table_name = table
        create_sql = create_sql.replace("CREATE TABLE IF NOT EXISTS ", "CREATE TABLE ", 1)
        for column in key_columns:
            create_sql = create_sql.replace(f"{column} NVARCHAR(MAX)", f"{column} NVARCHAR(450)")
        return self._create_table_if_missing_sql(
            create_sql=create_sql, database=database, schema=schema, table_name=table_name
        )

    def _state_index_sqls(
        self, *, table: tuple[str | None, str, str], index_name: str, columns: str
    ) -> tuple[str, ...]:
        """Create a state-table index unless sys.indexes already has it."""

        database, schema, name = table
        table_name: str | None = self.render_qualified_name(
            database=database, schema=schema, name=name
        )
        if table_name is None:
            return ()
        escaped_index_name: str = index_name.replace("'", "''")
        escaped_table_name: str = table_name.replace("'", "''")
        return (
            "IF NOT EXISTS ("
            "SELECT 1 FROM sys.indexes "
            f"WHERE name = '{escaped_index_name}' "
            f"AND object_id = OBJECT_ID(N'{escaped_table_name}')"
            ") "
            f"CREATE INDEX {index_name} ON {table_name} ({columns})",
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
        from sqlbuild.executor.node_results.constants import NODE_RESULTS_TABLE_NAME
        from sqlbuild.executor.node_results.main.create_table_sql import (
            build_node_results_create_table_sql,
        )

        create_sql: str = build_node_results_create_table_sql(
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
            render_framework_type=self.render_framework_type,
        ).replace("CREATE TABLE IF NOT EXISTS", "CREATE TABLE", 1)
        for column in (
            "node_type",
            "node_name",
            "target_database",
            "target_schema",
            "target_name",
            "run_id",
            "status",
        ):
            create_sql = create_sql.replace(f"{column} NVARCHAR(MAX)", f"{column} NVARCHAR(450)")
        return self._create_table_if_missing_sql(
            create_sql=create_sql,
            database=database,
            schema=schema,
            table_name=NODE_RESULTS_TABLE_NAME,
        )

    def render_create_node_result_index_sqls(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        from sqlbuild.executor.node_results.constants import NODE_RESULTS_TABLE_NAME

        table: tuple[str | None, str, str] = (database, schema, NODE_RESULTS_TABLE_NAME)
        return self._state_index_sqls(
            table=table,
            index_name="_sqlbuild_node_results_latest_idx",
            columns="node_type, node_name, target_database, target_schema, target_name, status, "
            "ts DESC, run_id DESC",
        ) + self._state_index_sqls(
            table=table,
            index_name="_sqlbuild_node_results_run_id_idx",
            columns="run_id, node_type, node_name, target_database, target_schema, target_name",
        )

    def render_create_audit_result_table_sql(self, *, database: str | None, schema: str) -> str:
        from sqlbuild.executor.audit_results.constants import AUDIT_RESULTS_TABLE_NAME
        from sqlbuild.executor.audit_results.main.create_table_sql import (
            build_audit_results_create_table_sql,
        )

        create_sql: str = build_audit_results_create_table_sql(
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
            render_framework_type=self.render_framework_type,
        ).replace("CREATE TABLE IF NOT EXISTS", "CREATE TABLE", 1)
        for column in (
            "result_id",
            "invocation_id",
            "run_id",
            "audit_name",
            "binding_key",
            "definition_fingerprint",
            "execution_fingerprint",
            "evaluation_mode",
            "run_scope_phase",
            "attachment_kind",
            "severity",
            "outcome",
        ):
            create_sql = create_sql.replace(f"{column} NVARCHAR(MAX)", f"{column} NVARCHAR(450)")
        return self._create_table_if_missing_sql(
            create_sql=create_sql,
            database=database,
            schema=schema,
            table_name=AUDIT_RESULTS_TABLE_NAME,
        )

    def render_create_audit_result_index_sqls(
        self, *, database: str | None, schema: str
    ) -> tuple[str, ...]:
        from sqlbuild.executor.audit_results.constants import AUDIT_RESULTS_TABLE_NAME

        table_name: str | None = self.render_qualified_name(
            database=database, schema=schema, name=AUDIT_RESULTS_TABLE_NAME
        )
        if table_name is None:
            return ()
        latest_index_name: str = "_sqlbuild_audit_results_latest_idx"
        run_id_index_name: str = "_sqlbuild_audit_results_run_id_idx"
        escaped_table_name: str = table_name.replace("'", "''")
        escaped_latest_index_name: str = latest_index_name.replace("'", "''")
        escaped_run_id_index_name: str = run_id_index_name.replace("'", "''")
        return (
            "IF NOT EXISTS (SELECT 1 FROM sys.indexes "
            f"WHERE name = '{escaped_latest_index_name}' "
            f"AND object_id = OBJECT_ID(N'{escaped_table_name}')) "
            f"CREATE INDEX {latest_index_name} ON {table_name} "
            "(audit_name, binding_key, execution_fingerprint, run_scope_phase, "
            "occurred_at DESC, result_id DESC)",
            "IF NOT EXISTS (SELECT 1 FROM sys.indexes "
            f"WHERE name = '{escaped_run_id_index_name}' "
            f"AND object_id = OBJECT_ID(N'{escaped_table_name}')) "
            f"CREATE INDEX {run_id_index_name} ON {table_name} "
            "(run_id, invocation_id, result_id)",
        )

    def render_create_janitor_event_table_sql(self, *, database: str | None, schema: str) -> str:
        from sqlbuild.executor.janitor_events.constants import JANITOR_EVENTS_TABLE_NAME
        from sqlbuild.executor.janitor_events.main.create_table_sql import (
            build_janitor_events_create_table_sql,
        )

        create_sql: str = build_janitor_events_create_table_sql(
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
            render_framework_type=self.render_framework_type,
        ).replace("CREATE TABLE IF NOT EXISTS", "CREATE TABLE", 1)
        return self._create_table_if_missing_sql(
            create_sql=create_sql,
            database=database,
            schema=schema,
            table_name=JANITOR_EVENTS_TABLE_NAME,
        )

    def render_create_migration_state_table_sql(self, *, database: str | None, schema: str) -> str:
        from sqlbuild._native import MIGRATION_TABLE_NAME, render_migration_state_create_sql

        create_sql: str = render_migration_state_create_sql(
            kind="model",
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
            render_framework_type=self.render_framework_type,
        ).replace("CREATE TABLE IF NOT EXISTS", "CREATE TABLE", 1)
        return self._create_table_if_missing_sql(
            create_sql=create_sql,
            database=database,
            schema=schema,
            table_name=MIGRATION_TABLE_NAME,
        )

    def render_create_old_name_view_state_table_sql(
        self, *, database: str | None, schema: str
    ) -> str:
        from sqlbuild._native import OLD_NAME_VIEW_TABLE_NAME, render_migration_state_create_sql

        create_sql: str = render_migration_state_create_sql(
            kind="old_name",
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
            render_framework_type=self.render_framework_type,
        ).replace("CREATE TABLE IF NOT EXISTS", "CREATE TABLE", 1)
        return self._create_table_if_missing_sql(
            create_sql=create_sql,
            database=database,
            schema=schema,
            table_name=OLD_NAME_VIEW_TABLE_NAME,
        )

    def render_create_column_migration_state_table_sql(
        self, *, database: str | None, schema: str
    ) -> str:
        from sqlbuild._native import COLUMN_MIGRATION_TABLE_NAME, render_migration_state_create_sql

        create_sql: str = render_migration_state_create_sql(
            kind="column",
            database=database,
            schema=schema,
            render_qualified_name=self.render_qualified_name,
            render_framework_type=self.render_framework_type,
        ).replace("CREATE TABLE IF NOT EXISTS", "CREATE TABLE", 1)
        return self._create_table_if_missing_sql(
            create_sql=create_sql,
            database=database,
            schema=schema,
            table_name=COLUMN_MIGRATION_TABLE_NAME,
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
            "WITH __sqlbuild_ranked AS ("
            f"SELECT *, ROW_NUMBER() OVER ("
            "PARTITION BY source_name, target_database, target_schema, target_name "
            "ORDER BY observed_at DESC, run_id DESC"
            f") AS __sqlbuild_history_rank FROM {table_name}"
            ") "
            f"DELETE FROM __sqlbuild_ranked WHERE __sqlbuild_history_rank > {retain_versions}"
        )

    def render_create_table_as(self, *, destination: str, sql: str) -> tuple[str, ...]:
        return (
            f"DROP TABLE IF EXISTS {destination}",
            f"SELECT * INTO {destination} FROM ({sql}) AS __create_source",
        )

    def render_create_view_as(self, *, destination: str, sql: str) -> tuple[str, ...]:
        return (f"DROP VIEW IF EXISTS {destination}", f"CREATE VIEW {destination} AS {sql}")

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
        del return_columns, runtime_version, entry_point, packages
        if language != FunctionLanguage.SQL:
            raise AdapterUserError(
                message="Python functions require an engine-specific implementation"
            )
        arg_sql: str = ", ".join(f"@{arg.name} {arg.type}" for arg in arguments)
        body_query: str = body_sql.strip().rstrip(";")
        for arg in arguments:
            body_query = re.sub(rf"\b{re.escape(str(arg.name))}\b", f"@{arg.name}", body_query)
        if returns.upper() == TABLE_FUNCTION_RETURN_TYPE:
            return (
                f"CREATE OR ALTER FUNCTION {destination}({arg_sql})\n"
                "RETURNS TABLE\n"
                "AS\n"
                f"RETURN (\n{body_query}\n)",
            )
        body_expression: str = body_query.removeprefix("SELECT").strip()
        if returns.upper() == BOOLEAN_RETURN_TYPE and not body_expression.upper().startswith(
            ("CASE ", "CAST(")
        ):
            body_expression = f"CASE WHEN {body_expression} THEN 1 ELSE 0 END"
        return (
            f"CREATE OR ALTER FUNCTION {destination}({arg_sql})\n"
            f"RETURNS {returns}\n"
            "AS\nBEGIN\n"
            f"    RETURN ({body_expression})\n"
            "END",
        )

    def render_drop(self, *, destination: str, if_exists: bool = True) -> tuple[str, ...]:
        exists_clause: str = " IF EXISTS" if if_exists else ""
        return (f"DROP TABLE{exists_clause} {destination}",)

    def render_drop_view(self, *, destination: str, if_exists: bool = True) -> tuple[str, ...]:
        exists_clause: str = " IF EXISTS" if if_exists else ""
        return (f"DROP VIEW{exists_clause} {destination}",)

    def render_rename(self, *, origin: str, destination: str) -> tuple[str, ...]:
        origin_name: str = self._sp_rename_relation_name(origin)
        destination_name: str = self._unquote_relation_part(destination.split(".")[-1])
        return (f"EXEC sp_rename '{origin_name}', '{destination_name}'",)

    def render_rename_view(self, *, origin: str, destination: str) -> tuple[str, ...]:
        return self.render_rename(origin=origin, destination=destination)

    def _create_table_if_missing_sql(
        self, *, create_sql: str, database: str | None, schema: str, table_name: str
    ) -> str:
        escaped_schema: str = schema.replace("'", "''")
        escaped_table: str = table_name.replace("'", "''")
        exists_sql: str = (
            "SELECT 1 FROM information_schema.tables "
            f"WHERE table_schema = '{escaped_schema}' AND table_name = '{escaped_table}'"
        )
        if database is not None:
            escaped_database: str = database.replace("'", "''")
            exists_sql += f" AND table_catalog = '{escaped_database}'"
        return f"IF NOT EXISTS ({exists_sql}) {create_sql}"

    def render_add_columns(
        self, *, destination: str, columns: tuple[ColumnInfo, ...]
    ) -> tuple[str, ...]:
        return tuple(
            f"ALTER TABLE {destination} ADD {self.render_identifier(col.name)} {col.type}"
            for col in columns
        )

    def render_alter_column_types(
        self, *, destination: str, columns: tuple[ColumnInfo, ...]
    ) -> tuple[str, ...]:
        return tuple(
            f"ALTER TABLE {destination} ALTER COLUMN {self.render_identifier(col.name)} {col.type}"
            for col in columns
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
        immutable_columns: frozenset[str] = frozenset(
            column.lower() for column in (*keys, *exclude_columns)
        )
        update_columns: tuple[str, ...] = tuple(
            col for col in source_columns if col.lower() not in immutable_columns
        )
        key_match_sql: str = " AND ".join(
            f"__target.{self.render_identifier(key)} = __source.{self.render_identifier(key)}"
            for key in keys
        )
        if update_columns:
            update_set: str = ", ".join(
                f"__target.{self.render_identifier(col)} = __source.{self.render_identifier(col)}"
                for col in update_columns
            )
            update_sql: str = (
                f"UPDATE __target SET {update_set} FROM {destination} AS __target "
                f"JOIN ({sql}) AS __source ON {key_match_sql}"
            )
            statement_recorder.record(update_sql)
            self.execute(connection=connection, sql=update_sql)
        col_list: str = ", ".join(self.render_identifier(column) for column in source_columns)
        insert_sql: str = (
            f"INSERT INTO {destination} ({col_list}) "
            f"SELECT {col_list} FROM ({sql}) AS __source "
            f"WHERE NOT EXISTS (SELECT 1 FROM {destination} AS __target WHERE {key_match_sql})"
        )
        statement_recorder.record(insert_sql)
        insert_cursor: Any = self.execute(connection=connection, sql=insert_sql)
        return self.affected_row_count(cursor=insert_cursor)

    def render_identifier(self, name: str) -> str:
        return "[" + name.replace("]", "]]") + "]"

    def render_exact_identifier(self, name: str) -> str:
        return self.render_identifier(name)

    def render_framework_type(self, type_name: FrameworkType) -> str:
        match type_name:
            case FrameworkType.STRING:
                return "NVARCHAR(MAX)"
            case FrameworkType.INTEGER:
                return "BIGINT"
            case FrameworkType.TIMESTAMP:
                return "DATETIME2"

    def render_loader_logical_type(self, type_name: LoaderLogicalType) -> str:
        match type_name:
            case LoaderLogicalType.BOOLEAN:
                return "BIT"
            case LoaderLogicalType.INTEGER:
                return "BIGINT"
            case LoaderLogicalType.FLOAT:
                return "FLOAT"
            case LoaderLogicalType.STRING:
                return "NVARCHAR(MAX)"
            case LoaderLogicalType.TIMESTAMP:
                return "DATETIME2"
            case LoaderLogicalType.DATE:
                return "DATE"
            case LoaderLogicalType.JSON:
                return "NVARCHAR(MAX)"

    def render_typed_scalar(self, *, value: SqlValue) -> str:
        if value.kind == SqlValueKind.STRING:
            rendered: str = _render_ansi_typed_scalar(value=value)
            return f"N{rendered}"
        if value.kind == SqlValueKind.BOOLEAN:
            return "1" if _typed_scalar_payload(value) else "0"
        return _render_ansi_typed_scalar(value=value)

    def render_typed_value_list(self, *, value: SqlValue) -> str:
        return _render_typed_value_list(value=value, render_scalar=self.render_typed_scalar)

    def render_typed_array(self, *, value: SqlValue) -> str:
        del value
        raise UnsupportedTypedSqlRenderingError(adapter_name=self.adapter_name, rendering="array")

    def render_typed_object(self, *, value: SqlValue) -> str:
        return f"JSON_QUERY(N{self._quote_sql_string(_encode_typed_json(value))})"

    def render_loader_value_literal(
        self, *, value: object, logical_type: LoaderLogicalType | None
    ) -> str:
        if value is None:
            return "NULL"
        if logical_type == LoaderLogicalType.JSON:
            return self._quote_sql_string(json.dumps(value, sort_keys=True))
        if isinstance(value, bool):
            return "1" if value else "0"
        if isinstance(value, int | float | Decimal):
            return str(value)
        if isinstance(value, datetime | date):
            return self._quote_sql_string(value.isoformat())
        return self._quote_sql_string(str(value))

    def _quote_sql_string(self, value: str) -> str:
        return "'" + value.replace("'", "''") + "'"

    def render_cursor_text(self, *, expression: str, cursor_type: str) -> str:
        """Render a timestamp bound in ISO 8601 (style 126), not CAST's 'Apr  4 2026  2:30PM'."""

        if cursor_type == CursorKind.TIMESTAMP:
            return f"CONVERT(VARCHAR(33), {expression}, {ISO8601_CONVERT_STYLE})"
        return super().render_cursor_text(expression=expression, cursor_type=cursor_type)

    def render_cursor_bound_literal(self, *, value: str, cursor_type: str | None) -> str:
        if cursor_type == CursorKind.INTEGER:
            return value
        if cursor_type == CursorKind.TIMESTAMP:
            return f"CAST({self._quote_sql_string(value)} AS DATETIME2(6))"
        return self._quote_sql_string(value)

    def describe_relation(self, *, connection: Any, relation: str) -> tuple[ColumnInfo, ...]:
        parts: list[str] = [part.strip("[]") for part in relation.split(".")]
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

    def _describe_relation_for_diff(
        self, *, connection: Any, relation: str
    ) -> tuple[ColumnInfo, ...]:
        parts: list[str] = [part.strip("[]") for part in relation.split(".")]
        name: str = parts[-1]
        schema_and_relation_part_count: int = 2
        schema: str | None = parts[-2] if len(parts) >= schema_and_relation_part_count else None
        cursor: Any = self.execute(
            connection=connection,
            sql=(
                "SELECT column_name, data_type, character_maximum_length, "
                "numeric_precision, numeric_scale, datetime_precision, is_nullable "
                "FROM information_schema.columns "
                f"WHERE table_name = {_quote_sql_string(name)}"
                + (f" AND table_schema = {_quote_sql_string(schema)}" if schema else "")
                + " ORDER BY ordinal_position"
            ),
        )
        return tuple(
            ColumnInfo(name=str(row[0]), type=self._format_diff_column_type(row=row))
            for row in cursor.fetchall()
        )

    def _format_diff_column_type(self, *, row: Any) -> str:
        data_type: str = str(row[1]).lower()
        detail: str = ""
        if data_type in DIFF_CHARACTER_LENGTH_TYPES:
            length: int | None = row[2]
            if length is not None:
                detail = "max" if length == -1 else str(length)
        elif data_type in DIFF_NUMERIC_PRECISION_TYPES:
            precision: int | None = row[3]
            scale: int | None = row[4]
            if precision is not None and scale is not None:
                detail = f"{precision},{scale}"
        elif data_type in DIFF_DATETIME_PRECISION_TYPES:
            datetime_precision: int | None = row[5]
            if datetime_precision is not None:
                detail = str(datetime_precision)
        if detail:
            data_type += f"({detail})"
        nullability: str = (
            "NULL" if str(row[6]).upper() == INFORMATION_SCHEMA_NULLABLE_VALUE else "NOT NULL"
        )
        return f"{data_type} {nullability}"

    def add_columns(
        self,
        *,
        connection: Any,
        destination: str,
        columns: tuple[ColumnInfo, ...],
        statement_recorder: StatementRecorder,
    ) -> None:
        raise AdapterUserError(message="add_columns requires an engine-specific implementation")

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
        raise AdapterUserError(
            message="alter_column_types requires an engine-specific implementation"
        )

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
        quoted_cursor: str = self.render_identifier(cursor_column)
        start_literal: str = self.render_cursor_bound_literal(
            value=str(start_cursor.value), cursor_type=start_cursor.kind
        )
        clauses: list[str] = [f"{quoted_cursor} >= {start_literal}"]
        if end_cursor is not None:
            end_literal: str = self.render_cursor_bound_literal(
                value=str(end_cursor.value), cursor_type=end_cursor.kind
            )
            clauses.append(f"{quoted_cursor} < {end_literal}")
        return " AND ".join(clauses)

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
        normalized_type: str = column_info.type.lower().split("(", 1)[0].split(maxsplit=1)[0]
        if normalized_type in DIFF_UNSUPPORTED_COMPARISON_TYPES:
            raise AdapterUserError(
                message=(
                    f"row diff column '{column}' uses unsupported SQL Server comparison "
                    f"type '{column_info.type}'"
                )
            )
        left_expression: str = f"__left.{column}"
        right_expression: str = f"__right.{column}"
        if tolerance is None:
            return (
                f"(({left_expression} IS NULL AND {right_expression} IS NULL) OR "
                f"({left_expression} IS NOT NULL AND {right_expression} IS NOT NULL AND "
                f"{left_expression} = {right_expression}))"
            )
        threshold_parts: list[str] = []
        if tolerance.absolute is not None:
            threshold_parts.append(self.format_row_diff_decimal_sql(tolerance.absolute))
        if tolerance.relative is not None:
            threshold_parts.append(
                f"{self.format_row_diff_decimal_sql(tolerance.relative)} * "
                f"(CASE WHEN ABS({left_expression}) >= ABS({right_expression}) "
                f"THEN ABS({left_expression}) ELSE ABS({right_expression}) END)"
            )
        threshold_sql: str = threshold_parts[0]
        if len(threshold_parts) > 1:
            threshold_sql = (
                f"(CASE WHEN {threshold_parts[0]} >= {threshold_parts[1]} "
                f"THEN {threshold_parts[0]} ELSE {threshold_parts[1]} END)"
            )
        return (
            f"(({left_expression} IS NULL AND {right_expression} IS NULL) OR "
            f"({left_expression} IS NOT NULL AND {right_expression} IS NOT NULL AND "
            f"ABS({left_expression} - {right_expression}) <= {threshold_sql}))"
        )

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

    def default_promotion_strategy(self) -> PromotionStrategy:
        """Return atomic swap as the generic staged promotion strategy."""
        return PromotionStrategy.ATOMIC_SWAP

    def default_table_promotion_mode(self) -> TablePromotionMode:
        """Return staged as the generic default promotion mode."""
        return TablePromotionMode.STAGED

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
    ) -> None:
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
        with self.transaction(connection):
            stmt: str
            for stmt in statements:
                self.execute(connection=connection, sql=stmt)

    def estimate_relation_row_count(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str | None,
        name: str,
        metadata_only: bool = False,
    ) -> RelationRowCountEstimate:
        """Sum heap or clustered-index partition row counts from the catalog."""

        catalog: str = "" if database is None else f"{self.render_identifier(database)}."
        schema_filter: str = (
            "schemas.name = SCHEMA_NAME()"
            if schema is None
            else f"schemas.name = N{_quote_sql_string(schema)}"
        )
        row: tuple[Any, ...] | None = self.execute(
            connection=connection,
            sql=(
                "SELECT objects.type, SUM(partitions.rows) "
                f"FROM {catalog}sys.objects AS objects "
                f"JOIN {catalog}sys.schemas AS schemas ON schemas.schema_id = objects.schema_id "
                f"LEFT JOIN {catalog}sys.partitions AS partitions "
                "ON partitions.object_id = objects.object_id AND partitions.index_id IN (0, 1) "
                f"WHERE {schema_filter} AND objects.name = N{_quote_sql_string(name)} "
                "GROUP BY objects.type"
            ),
        ).fetchone()
        if row is None:
            return RelationRowCountEstimate(row_count=None, detail="relation not found")
        if str(row[0]).strip().upper() != USER_TABLE_OBJECT_TYPE or row[1] is None:
            return RelationRowCountEstimate(row_count=None, detail="no row count (view)")
        return RelationRowCountEstimate(row_count=int(row[1]))

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
        keys: tuple[str, ...] = (unique_key,) if isinstance(unique_key, str) else unique_key
        sampling: RowDiffSampling | None = tolerances.sampling if tolerances is not None else None
        left_columns: tuple[ColumnInfo, ...] = self.describe_relation(
            connection=connection, relation=left
        )
        compare_columns: tuple[str, ...] = tuple(
            column.name
            for column in left_columns
            if column.name not in keys and column.name not in excluded_columns
        )
        left_columns_by_name: dict[str, ColumnInfo] = {
            column.name: column for column in left_columns
        }
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
        join_condition: str = " AND ".join(f"__left.{key} = __right.{key}" for key in keys)
        column_equal_expressions: dict[str, str] = {
            column: self.build_row_diff_equal_expression(
                column=column,
                column_info=left_columns_by_name[column],
                tolerances=tolerances,
            )
            for column in compare_columns
        }
        column_tolerances: dict[str, RowDiffTolerance | None] = {
            column: self.resolve_row_diff_tolerance(
                column=column,
                column_type=left_columns_by_name[column].type,
                tolerances=tolerances,
            )
            for column in compare_columns
        }
        equal_condition: str = "1 = 1"
        if compare_columns:
            equal_condition = " AND ".join(column_equal_expressions.values())
        column_count_sql_parts: list[str] = [
            f"COUNT(CASE WHEN __left.{keys[0]} IS NOT NULL "
            f"AND __right.{keys[0]} IS NOT NULL "
            f"AND NOT ({column_equal_expressions[column]}) THEN 1 END) "
            f"AS __{column}_mismatch_count"
            for column in compare_columns
        ]
        column_count_sql: str = ""
        if column_count_sql_parts:
            column_count_sql = ", " + ", ".join(column_count_sql_parts)
        diff_sql: str = (
            f"WITH {prepared_relations.cte_sql} "
            "SELECT "
            f"COUNT(CASE WHEN __left.{keys[0]} IS NOT NULL THEN 1 END) AS left_count, "
            f"COUNT(CASE WHEN __right.{keys[0]} IS NOT NULL THEN 1 END) AS right_count, "
            "COUNT(*) AS joined, "
            f"COUNT(CASE WHEN __left.{keys[0]} IS NOT NULL "
            f"AND __right.{keys[0]} IS NOT NULL AND ({equal_condition}) "
            "THEN 1 END) AS equal, "
            f"COUNT(CASE WHEN __left.{keys[0]} IS NOT NULL "
            f"AND __right.{keys[0]} IS NOT NULL AND NOT ({equal_condition}) "
            "THEN 1 END) AS unequal, "
            f"COUNT(CASE WHEN __right.{keys[0]} IS NULL THEN 1 END) AS left_only, "
            f"COUNT(CASE WHEN __left.{keys[0]} IS NULL THEN 1 END) AS right_only"
            f"{column_count_sql} "
            f"FROM __left FULL OUTER JOIN __right ON {join_condition}"
        )
        row: tuple[Any, ...] = self.execute(connection=connection, sql=diff_sql).fetchone()
        column_results: tuple[RowDiffColumnResult, ...] = tuple(
            RowDiffColumnResult(
                name=column,
                mismatched_count=int(row[index]),
                tolerance=column_tolerances[column],
            )
            for index, column in enumerate(compare_columns, start=7)
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

    def diff_schema(
        self,
        *,
        connection: Any,
        left: str,
        right: str,
    ) -> SchemaDiffResult:
        left_columns: tuple[ColumnInfo, ...] = self._describe_relation_for_diff(
            connection=connection, relation=left
        )
        right_columns: tuple[ColumnInfo, ...] = self._describe_relation_for_diff(
            connection=connection, relation=right
        )
        left_map: dict[str, str] = {column.name: column.type for column in left_columns}
        right_map: dict[str, str] = {column.name: column.type for column in right_columns}
        added: list[ColumnInfo] = []
        removed: list[ColumnInfo] = []
        type_changed: list[tuple[ColumnInfo, ColumnInfo]] = []
        for column_name, column_type in right_map.items():
            if column_name not in left_map:
                added.append(ColumnInfo(name=column_name, type=column_type))
            elif left_map[column_name] != column_type:
                type_changed.append(
                    (
                        ColumnInfo(name=column_name, type=left_map[column_name]),
                        ColumnInfo(name=column_name, type=column_type),
                    )
                )
        for column_name, column_type in left_map.items():
            if column_name not in right_map:
                removed.append(ColumnInfo(name=column_name, type=column_type))
        return SchemaDiffResult(
            added_columns=tuple(added),
            removed_columns=tuple(removed),
            type_changed_columns=tuple(type_changed),
            left_column_count=len(left_columns),
            right_column_count=len(right_columns),
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

    def expression_inference_profile(self) -> ExpressionInferenceProfile:
        """Return portable static expression inference behavior by default."""

        return ExpressionInferenceProfile(sql_analysis_dialect=self.sql_analysis_dialect())

    def format_row_diff_decimal_sql(self, value: Decimal) -> str:
        return format(value, "f")

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

    def maximum_identifier_length(self) -> int:
        """Return the maximum unqualified identifier length supported by the adapter."""

        return self.max_identifier_length

    def normalize_row_diff_numeric_type(self, column_type: str) -> str | None:
        normalized: str = column_type.upper()
        if any(token in normalized for token in ("DOUBLE", "FLOAT", "REAL")):
            return "float"
        if any(token in normalized for token in ("DECIMAL", "NUMERIC")):
            return "decimal"
        if INTEGER_TYPE_TOKEN in normalized:
            return "integer"
        return None

    def persists_python_functions(self) -> bool:
        return True

    def python_functions_inherit_default_namespace(self) -> bool:
        return True

    def max_statement_size(self) -> StatementSizeLimit | None:
        """Defer 65,536 * negotiated packet bytes to SQL Server: https://learn.microsoft.com/en-us/sql/sql-server/maximum-capacity-specifications-for-sql-server."""

        return None

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

    def render_append(
        self, *, destination: str, sql: str, columns: tuple[str, ...] | None = None
    ) -> tuple[str, ...]:
        if columns is not None:
            col_list: str = ", ".join(self.render_identifier(column) for column in columns)
            return (f"INSERT INTO {destination} ({col_list}) {sql}",)
        return (f"INSERT INTO {destination} {sql}",)

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
            statements=(f"SELECT * INTO {stage} FROM {origin}",),
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
        del relation_type
        relation: str = ".".join(
            self.render_identifier(part) for part in (database, schema, name) if part
        ).replace("'", "''")
        query: str = (
            "SELECT permission.state_desc, permission.permission_name, grantee.name, "
            "COL_NAME(permission.major_id, NULLIF(permission.minor_id, 0)) "
            "FROM sys.database_permissions AS permission "
            "JOIN sys.database_principals AS grantee "
            "ON grantee.principal_id = permission.grantee_principal_id "
            "WHERE permission.class = 1 "
            f"AND permission.major_id = OBJECT_ID(N'{relation}') "
            "ORDER BY permission.minor_id, grantee.name, permission.permission_name"
        )
        rows: list[tuple[Any, ...]] = self.execute(connection=connection, sql=query).fetchall()
        return sqlserver_relation_grants(rows)

    def render_relation_grants(
        self, *, grants: tuple[RelationGrant, ...], destination: str, columns: tuple[str, ...]
    ) -> tuple[str, ...]:
        return render_sqlserver_view_grants(grants=grants, destination=destination, columns=columns)

    def render_replace_view_keeping_grants(
        self, *, destination: str, sql: str
    ) -> tuple[str, ...] | None:
        return (f"ALTER VIEW {destination} AS {sql}",)

    def read_view_definition(
        self, *, connection: Any, database: str | None, schema: str, name: str
    ) -> str | None:
        relation: str = ".".join(
            self.render_identifier(part) for part in (database, schema, name) if part
        ).replace("'", "''")
        rows: list[tuple[Any, ...]] = self.execute(
            connection=connection,
            sql=f"SELECT OBJECT_DEFINITION(OBJECT_ID(N'{relation}', N'V'))",
        ).fetchall()
        return None if not rows or rows[0][0] is None else str(rows[0][0])

    def view_definition_matches(
        self, *, connection: Any, database: str | None, schema: str, name: str, sql: str
    ) -> bool:
        definition: str | None = self.read_view_definition(
            connection=connection, database=database, schema=schema, name=name
        )
        return definition is not None and same_view_definition(definition=definition, sql=sql)

    def render_relation_revokes(
        self, *, grants: tuple[RelationGrant, ...], destination: str
    ) -> tuple[str, ...]:
        return render_sqlserver_view_revokes(grants=grants, destination=destination)

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
        if invalidate_hard_deletes:
            historical: HistoricalSnapshotSql = HistoricalSnapshotSql(
                origin=origin,
                unique_key=unique_key,
                observed_at_column=observed_at_column,
                valid_from_column=valid_from_column,
                valid_to_column=valid_to_column,
                check_columns=check_columns,
                distinct_condition=self._distinct_condition,
            )
            return (
                f"DROP TABLE IF EXISTS {destination}",
                f";WITH {historical.initial_ctes_sql()} "
                f"SELECT {historical.initial_select_list_sql(output_columns=output_columns)} "
                f"INTO {destination} FROM __versions",
            )
        partition_sql: str = ", ".join(unique_key)
        previous_columns_sql: str = ", ".join(
            self._lag_expr(
                column=column,
                partition_sql=partition_sql,
                observed_at_column=observed_at_column,
            )
            for column in check_columns
        )
        if previous_columns_sql:
            previous_columns_sql = f", {previous_columns_sql}"
        change_condition: str = " OR ".join(
            self._distinct_condition(left=column, right=f"__prev_{column}")
            for column in check_columns
        )
        output_select_sql: str = ", ".join(output_columns)
        hard_deleted_at_sql: str = "CAST(NULL AS DATETIME2)"
        valid_to_expr: str = (
            "CASE "
            "WHEN __next_change_at IS NULL THEN __hard_deleted_at "
            "WHEN __hard_deleted_at IS NULL THEN __next_change_at "
            "WHEN __hard_deleted_at < __next_change_at THEN __hard_deleted_at "
            "ELSE __next_change_at END"
        )
        historical_sql: str = (
            f";WITH __ordered AS (SELECT *, LAG({observed_at_column}) OVER ("
            f"PARTITION BY {partition_sql} ORDER BY {observed_at_column}"
            f") AS __prev_observed_at{previous_columns_sql} FROM {origin}"
            "), __changes AS ("
            f"SELECT * FROM __ordered WHERE __prev_observed_at IS NULL OR ({change_condition})"
            "), __versions AS ("
            f"SELECT __changes.*, LEAD({observed_at_column}) OVER ("
            f"PARTITION BY {partition_sql} ORDER BY {observed_at_column}"
            f") AS __next_change_at, {hard_deleted_at_sql} AS __hard_deleted_at FROM __changes"
            ") "
            f"SELECT {output_select_sql}, {observed_at_column} AS {valid_from_column}, "
            f"{valid_to_expr} AS {valid_to_column} INTO {destination} FROM __versions"
        )
        return (f"DROP TABLE IF EXISTS {destination}", historical_sql)

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
        partition_sql: str = ", ".join(unique_key)
        output_select_sql: str = ", ".join(output_columns)
        historical_sql: str = (
            f";WITH __ordered AS (SELECT *, LEAD({updated_at_column}) OVER ("
            f"PARTITION BY {partition_sql} ORDER BY {updated_at_column}"
            f") AS __next_updated_at FROM {origin}) "
            f"SELECT {output_select_sql}, {updated_at_column} AS {valid_from_column}, "
            f"__next_updated_at AS {valid_to_column} INTO {destination} FROM __ordered"
        )
        return (f"DROP TABLE IF EXISTS {destination}", historical_sql)

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
        if invalidate_hard_deletes:
            historical: HistoricalSnapshotSql = HistoricalSnapshotSql(
                origin=origin,
                unique_key=unique_key,
                observed_at_column=observed_at_column,
                valid_from_column=valid_from_column,
                valid_to_column=valid_to_column,
                updated_at_column=updated_at_column,
                distinct_condition=self._distinct_condition,
            )
            return (
                f"DROP TABLE IF EXISTS {destination}",
                f";WITH {historical.initial_ctes_sql()} "
                f"SELECT {historical.initial_select_list_sql(output_columns=output_columns)} "
                f"INTO {destination} FROM __versions",
            )
        partition_sql: str = ", ".join(unique_key)
        changed_condition: str = self._distinct_condition(
            left=updated_at_column, right="__prev_updated_at"
        )
        output_select_sql: str = ", ".join(output_columns)
        historical_sql: str = (
            f";WITH __ordered AS (SELECT *, LAG({updated_at_column}) OVER ("
            f"PARTITION BY {partition_sql} ORDER BY {observed_at_column}"
            f") AS __prev_updated_at FROM {origin}"
            "), __changes AS ("
            f"SELECT * FROM __ordered WHERE __prev_updated_at IS NULL OR {changed_condition}"
            ") "
            f"SELECT {output_select_sql}, {updated_at_column} AS {valid_from_column}, "
            f"LEAD({updated_at_column}) OVER (PARTITION BY {partition_sql} "
            f"ORDER BY {updated_at_column}) AS {valid_to_column} INTO {destination} FROM __changes"
        )
        return (f"DROP TABLE IF EXISTS {destination}", historical_sql)

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

    def render_current_timestamp(self) -> str:
        return "CURRENT_TIMESTAMP"

    def render_current_utc_timestamp(self) -> str:
        return "SYSUTCDATETIME()"

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
        start_literal: str = self.render_cursor_bound_literal(
            value=cursor_start, cursor_type=cursor_type
        )
        end_literal: str = self.render_cursor_bound_literal(
            value=cursor_end, cursor_type=cursor_type
        )
        delete_sql: str = (
            f"DELETE FROM {destination} "
            f"WHERE {self.render_identifier(cursor_column)} >= {start_literal} "
            f"AND {self.render_identifier(cursor_column)} < {end_literal}"
        )
        insert_stmts: tuple[str, ...] = self.render_append(
            destination=destination, sql=sql, columns=columns
        )
        return (delete_sql, *insert_stmts)

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
        column: str = (
            f"{self._sp_rename_relation_name(destination)}.{self.render_identifier(old_name)}"
        )
        escaped_column: str = column.replace("'", "''")
        escaped_name: str = new_name.replace("'", "''")
        return (f"EXEC sp_rename N'{escaped_column}', N'{escaped_name}', 'COLUMN'",)

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
            value_rows.append(f"({', '.join(row_values)})")
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

    def render_replace_table_from_relation(
        self, *, destination: str, origin: str
    ) -> tuple[str, ...]:
        return self.render_create_table_as(destination=destination, sql=f"SELECT * FROM {origin}")

    def render_set_difference_operator(self) -> str:
        """Render the generic SQL set-difference operator."""

        return "EXCEPT"

    def render_source_expression_cast(
        self, *, expression: str, target_type: str, alias: str
    ) -> str:
        """Render a generic cast projection for source expression type enforcement."""

        return f"CAST({expression} AS {target_type}) AS {alias}"

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
        relation: str = (
            f"{source_relation} AS __source_freshness" if source_is_subquery else source_relation
        )
        return (
            f"SELECT MAX({self.render_identifier(column)}) AS data_version "
            f"FROM {relation}{where_sql}"
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
        return f"(SELECT *, {cast_clause} FROM {source_relation})"

    def _render_source_relation_cast_subquery_with_columns(
        self,
        *,
        source_relation: str,
        cast_projections: tuple[str, ...],
        cast_column_names: tuple[str, ...],
        warehouse_column_names: tuple[str, ...],
        all_columns_cast: bool,
    ) -> str:
        """Render SQL Server source casts without duplicate projection names."""

        cast_clause: str = ", ".join(cast_projections)
        if all_columns_cast:
            return f"(SELECT {cast_clause} FROM {source_relation})"
        projection_names: set[str] = set(cast_column_names)
        passthrough_columns: tuple[str, ...] = tuple(
            column for column in warehouse_column_names if column not in projection_names
        )
        passthrough_clause: str = ", ".join(
            self.render_identifier(name) for name in passthrough_columns
        )
        projection_clause: str = cast_clause
        if passthrough_clause:
            projection_clause = f"{passthrough_clause}, {cast_clause}"
        return f"(SELECT {projection_clause} FROM {source_relation})"

    def requires_derived_table_aliases(self) -> bool:
        """SQL Server requires aliases for derived table factors."""

        return True

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
        return self._unquote_relation_part(relation.split(".")[-1])

    def _with_replaced_relation_name(self, *, relation: str, name: str) -> str:
        parts: list[str] = relation.split(".")
        parts[-1] = self.render_identifier(name)
        return ".".join(parts)

    def _sp_rename_relation_name(self, relation: str) -> str:
        parts: list[str] = relation.split(".")
        schema_and_relation_part_count: int = 2
        if len(parts) >= schema_and_relation_part_count:
            return ".".join(parts[-2:])
        return relation

    def _unquote_relation_part(self, part: str) -> str:
        return part.strip().strip('"').strip("`").removeprefix("[").removesuffix("]")

    def render_table_function_call(self, *, target: str, call_suffix_sql: str) -> str:
        return f"{target}{call_suffix_sql}"

    def render_udf_call(self, *, target: str, call_suffix_sql: str) -> str:
        return f"{target}{call_suffix_sql}"

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
        origin_parts: list[str] = origin.split(".")
        destination_parts: list[str] = destination.split(".")
        schema_and_relation_part_count: int = 2
        if (
            remove_origin
            and len(origin_parts) >= schema_and_relation_part_count
            and len(destination_parts) >= schema_and_relation_part_count
            and origin_parts[:-2] == destination_parts[:-2]
        ):
            origin_parent: str = ".".join(origin_parts[:-1])
            destination_parent: str = ".".join(destination_parts[:-1])
            if origin_parent == destination_parent:
                statements: tuple[str, ...] = self.render_rename(
                    origin=origin, destination=destination
                )
            else:
                destination_schema: str = destination_parts[-2]
                origin_name: str = origin_parts[-1]
                destination_name: str = destination_parts[-1]
                moved_origin: str = ".".join((*origin_parts[:-2], destination_schema, origin_name))
                statements = (f"ALTER SCHEMA {destination_schema} TRANSFER {origin}",)
                if origin_name != destination_name:
                    destination_name_unquoted: str = destination_name.strip("[]")
                    statements = (
                        *statements,
                        f"EXEC sp_rename '{moved_origin}', '{destination_name_unquoted}'",
                    )
            statement_recorder.record_many(statements)
            stmt: str
            for stmt in statements:
                self.execute(connection=connection, sql=stmt)
            return
        if not allow_copy_fallback:
            raise AdapterUserError(message="SQL Server relation move/copy requires --allow-copy")
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
        join_condition: str = " AND ".join(f"__left.{key} = __right.{key}" for key in keys)
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
            f"SELECT TOP {limit} {key_select_sql} "
            f"FROM __left FULL OUTER JOIN __right ON {join_condition} "
            f"WHERE {side_condition} "
            f"ORDER BY {', '.join(f'__key_{key}' for key in keys)}"
        )
        rows: list[tuple[Any, ...]] = self.execute(connection=connection, sql=sample_sql).fetchall()
        samples: list[tuple[tuple[str, object], ...]] = []
        for row in rows:
            samples.append(tuple((key, row[index]) for index, key in enumerate(keys)))
        return tuple(samples)

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
        sampling: RowDiffSampling | None = tolerances.sampling if tolerances is not None else None
        keys: tuple[str, ...] = (unique_key,) if isinstance(unique_key, str) else unique_key
        left_columns: tuple[ColumnInfo, ...] = self.describe_relation(
            connection=connection, relation=left
        )
        compare_columns: tuple[str, ...] = tuple(
            column.name
            for column in left_columns
            if column.name not in keys and column.name not in excluded_columns
        )
        left_columns_by_name: dict[str, ColumnInfo] = {
            column.name: column for column in left_columns
        }
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
            column: self.build_row_diff_equal_expression(
                column=column,
                column_info=left_columns_by_name[column],
                tolerances=tolerances,
            )
            for column in compare_columns
        }
        unequal_condition: str = "1 = 0"
        if compare_columns:
            unequal_condition = " OR ".join(
                f"NOT ({expression})" for expression in column_equal_expressions.values()
            )
        key_select_sql: str = ", ".join(
            f"COALESCE(__left.{key}, __right.{key}) AS __key_{key}" for key in keys
        )
        compare_select_sql: str = ", ".join(
            f"__left.{column} AS __left_{column}, "
            f"__right.{column} AS __right_{column}, "
            f"CASE WHEN {column_equal_expressions[column]} THEN 0 ELSE 1 END "
            f"AS __changed_{column}"
            for column in compare_columns
        )
        if compare_select_sql:
            compare_select_sql = ", " + compare_select_sql
        join_condition: str = " AND ".join(f"__left.{key} = __right.{key}" for key in keys)
        sample_sql: str = (
            f"WITH {prepared_relations.cte_sql} "
            f"SELECT TOP {limit} {key_select_sql}{compare_select_sql} "
            f"FROM __left FULL OUTER JOIN __right ON {join_condition} "
            f"WHERE __left.{keys[0]} IS NOT NULL AND __right.{keys[0]} IS NOT NULL "
            f"AND ({unequal_condition}) "
            f"ORDER BY {', '.join(f'__key_{key}' for key in keys)}"
        )
        rows: list[tuple[Any, ...]] = self.execute(connection=connection, sql=sample_sql).fetchall()
        samples: list[RowDiffSampleRow] = []
        for row in rows:
            key_values: tuple[tuple[str, object], ...] = tuple(
                (key, row[index]) for index, key in enumerate(keys)
            )
            changed_cells: list[RowDiffSampleCell] = []
            for column_index, column in enumerate(compare_columns):
                left_value_index: int = len(keys) + (column_index * 3)
                right_value_index: int = left_value_index + 1
                changed_flag_index: int = right_value_index + 1
                left_value: object = row[left_value_index]
                right_value: object = row[right_value_index]
                if bool(row[changed_flag_index]):
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

    def sql_analysis_dialect(self) -> str | None:
        """Return the configured SQL analysis dialect name, if any."""

        return self.sql_analysis_dialect_name

    def star_exclude_keyword(self) -> str:
        """Return the SQL keyword for SELECT * EXCLUDE/EXCEPT syntax."""
        return "EXCLUDE"

    def supports_python_functions(self) -> bool:
        return False

    def supports_relation_age_metadata(self) -> bool:
        """Return False because sys.objects.modify_date ignores DML, so written tables look old."""

        return False

    def supports_table_functions(self) -> bool:
        return False

    def supports_unqualified_function_fingerprints(self) -> bool:
        return False

    def interrupt_connection(self, connection: Any) -> bool:
        """Cancel the connection's running statement through the driver."""

        connection.cancel_active_statements()
        return True

    def lists_relations_authoritatively(self, *, database: str | None, schema: str) -> bool:
        """Return whether ``list_relations`` reports every relation in this scope."""

        del database, schema
        return True

    def supports_zero_copy_clone(self) -> bool:
        return False

    def supports_durable_clone(self) -> bool:
        return False

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

    def _render_row_diff_sample_keys_sql(
        self,
        *,
        key_union_sql: str,
        keys: tuple[str, ...],
        sampling: RowDiffSampling,
    ) -> str:
        """Render deterministic SQL Server top-N union-key selection."""

        key_list: str = ", ".join(keys)
        hash_expression: str = self._render_row_diff_key_hash(
            keys=keys,
            alias="__key_union",
            seed=sampling.seed,
        )
        tie_break: str = ", ".join(f"__key_union.{key}" for key in keys)
        return (
            f"SELECT TOP ({sampling.row_limit}) {key_list} "
            f"FROM ({key_union_sql}) AS __key_union "
            f"ORDER BY {hash_expression}, {tie_break}"
        )

    def _render_unkeyed_sample_sql(
        self, *, joined_sql: str, select_list: str, predicate: str, limit: int
    ) -> str:
        """Render the bounded SQL Server side-only sample query."""

        return f"{joined_sql} SELECT TOP ({limit}) {select_list} FROM __joined WHERE {predicate}"

    def _render_row_diff_key_hash(
        self,
        *,
        keys: tuple[str, ...],
        alias: str,
        seed: int,
    ) -> str:
        """Render a stable SQL Server hash over a canonical composite key."""

        key_parts: list[str] = []
        key: str
        for key in keys:
            value_sql: str = f"CAST({alias}.{key} AS NVARCHAR(MAX))"
            key_parts.append(f"CONCAT(DATALENGTH({value_sql}), ':', {value_sql})")
        payload_parts: str = ", ".join((f"'{seed}'", *key_parts))
        return f"CONVERT(VARCHAR(64), HASHBYTES('SHA2_256', CONCAT_WS('|', {payload_parts})), 2)"

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

    def validate_row_diff_tolerance(self, *, column: str, tolerance: RowDiffTolerance) -> None:
        if tolerance.absolute is None and tolerance.relative is None:
            raise AdapterUserError(
                message=f"row diff tolerance for column '{column}' must define absolute or relative"
            )

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
        import csv

        if replace:
            self.drop(
                connection=connection,
                destination=destination,
                if_exists=True,
                statement_recorder=statement_recorder,
            )
        column_defs: str = ", ".join(
            f"{self.render_identifier(col.name)} {col.type}" for col in columns
        )
        create_sql: str = f"CREATE TABLE {destination} ({column_defs})"
        statement_recorder.record(create_sql)
        self.execute(connection=connection, sql=create_sql)

        column_names: tuple[str, ...] = tuple(col.name for col in columns)
        placeholders: str = ", ".join(["%s"] * len(column_names))
        column_sql: str = ", ".join(self.render_identifier(col) for col in column_names)
        insert_sql: str = f"INSERT INTO {destination} ({column_sql}) VALUES ({placeholders})"
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
