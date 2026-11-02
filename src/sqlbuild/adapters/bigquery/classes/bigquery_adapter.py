"""BigQuery adapter implementation."""

from __future__ import annotations

import json
import logging
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
from sqlbuild.adapter.contract.classes.base_adapter import (
    _COLLECTION_SQL_VALUE_KINDS,
    BaseAdapter,
    _encode_typed_json,
    _render_ansi_typed_scalar,
    _render_typed_value_list,
    _typed_collection_items,
    _typed_scalar_payload,
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
from sqlbuild.adapter.contract.main.complete_table_freshness_results import (
    complete_table_freshness_results,
)
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
    RenderedRetentionChange,
    RetentionRequest,
    RetentionState,
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
    RetentionChangePhase,
    RetentionScope,
    SnapshotLatestVersionStyle,
    SnapshotUpdateStyle,
    StatementSizeLimit,
    TableFreshnessStatus,
    TablePromotionMode,
)
from sqlbuild.adapter.relations.main.get_columns_for_relations import (
    get_columns_for_relations_bulk,
)
from sqlbuild.adapter.relations.main.invalidate_cached_relations import invalidate_cached_relations
from sqlbuild.adapter.relations.main.relation_age_timestamp import relation_age_timestamp_utc
from sqlbuild.adapter.type_system.main.conditional_result_nullability import (
    conditional_result_nullability,
)
from sqlbuild.adapter.type_system.main.first_arg_nullability import first_arg_nullability
from sqlbuild.adapter.type_system.main.normalize_numeric_family import normalize_numeric_family
from sqlbuild.adapter.type_system.main.types_equal import types_equal
from sqlbuild.adapters.bigquery._helpers.clone_refusal import is_bigquery_clone_refusal
from sqlbuild.adapters.bigquery._helpers.grants import (
    bigquery_relation_grants,
    render_bigquery_view_grants,
    render_bigquery_view_move,
    render_bigquery_view_revokes,
)
from sqlbuild.adapters.bigquery._helpers.relation_usage import bigquery_relation_usage
from sqlbuild.adapters.bigquery._helpers.statement_telemetry import affected_rows
from sqlbuild.adapters.bigquery.classes.bigquery_connection import _BigQueryConnection
from sqlbuild.adapters.bigquery.classes.bigquery_cursor import _BigQueryCursor
from sqlbuild.adapters.bigquery.constants import (
    BOOLEAN_METADATA_TYPE_NAME,
    BOOLEAN_WIRE_TYPE_NAME,
    DATE_TYPE_NAME,
    DATETIME_TYPE_NAME,
    DECIMAL_METADATA_TYPE_NAMES,
    FLOAT_METADATA_TYPE_NAME,
    FLOAT_WIRE_TYPE_NAME,
    INTEGER_METADATA_TYPE_NAME,
    INTEGER_TYPE_TOKEN,
    INTEGER_WIRE_TYPE_NAME,
    NATIVE_TABLE_TYPE,
    NOT_FOUND_ERROR_CLASS_NAME,
    SELECT_STATEMENT_TYPE,
    TABLE_NAME_WILDCARD,
    TIMESTAMP_TYPE_TOKEN,
)
from sqlbuild.compiler.compile.types import FunctionLanguage
from sqlbuild.diagnostics.main.log_sql import log_sql
from sqlbuild.runtime.observability.classes.statement_lifecycle import StatementLifecycle
from sqlbuild.spec.contracts.constants import DEFAULT_SEED_CSV_SETTINGS


class BigQueryAdapter(MicrobatchMixin, UnkeyedDiffMixin, BaseAdapter):
    """BigQuery adapter backed by google-cloud-bigquery."""

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
    statement_cancellation_is_asynchronous: ClassVar[bool] = True
    adapter_name: ClassVar[str] = BuiltinAdapter.BIGQUERY.value
    sql_analysis_dialect_name: ClassVar[str | None] = "bigquery"
    sql_lexical_syntax: ClassVar[SqlLexicalSyntax] = SqlLexicalSyntax(
        backslash_escape_quotes=frozenset({"'", '"', "`"}),
        triple_quoted_strings=True,
        line_comment_prefixes=frozenset({"--", "#"}),
    )
    views_read_with_reader_access: ClassVar[bool] = True
    max_identifier_length: ClassVar[int] = 1024
    concurrent_ddl_limit: ClassVar[int] = 16
    _snapshot_sql_dialect: ClassVar[SnapshotSqlDialect] = SnapshotSqlDialect(
        timestamp_type="TIMESTAMP",
        distinct_condition=render_is_distinct_from,
        update_style=SnapshotUpdateStyle.UPDATE_FROM,
        latest_version=SnapshotLatestVersionStyle.QUALIFY,
        historical_close=HistoricalSnapshotCloseStyle.UPDATE_FROM,
        historical_insert=HistoricalSnapshotInsertStyle.INSERT_WITH,
    )

    def inspect_retention(
        self, *, connection: _BigQueryConnection, request: RetentionRequest
    ) -> RetentionState:
        dataset_id: str = self._retention_dataset_id(request=request)
        try:
            dataset: Any = connection.client.get_dataset(dataset_id)
        except Exception as error:
            if not self._is_google_not_found(error):
                raise
            return RetentionState(
                request_id=request.request_id,
                scope=request.scope,
                configured_days=None,
                effective_days=7,
                exists=False,
                max_time_travel_hours=168,
            )
        hours: object | None = getattr(dataset, "max_time_travel_hours", None)
        if not isinstance(hours, int):
            raise AdapterUserError(
                message=f"BigQuery dataset {dataset_id} has no max_time_travel_hours metadata"
            )
        return RetentionState(
            request_id=request.request_id,
            scope=request.scope,
            configured_days=hours // 24,
            effective_days=hours // 24,
            max_time_travel_hours=hours,
        )

    def render_retention_changes(
        self, *, request: RetentionRequest, state: RetentionState | None = None
    ) -> tuple[RenderedRetentionChange, ...]:
        del state
        dataset_id: str = self._retention_dataset_id(request=request)
        return (
            RenderedRetentionChange(
                phase=RetentionChangePhase.ALTER,
                statements=(
                    f"ALTER SCHEMA {self._quote_identifier_path(dataset_id)} SET OPTIONS "
                    f"(max_time_travel_hours = {request.desired_days * 24})",
                ),
            ),
        )

    def _retention_dataset_id(self, *, request: RetentionRequest) -> str:
        if request.scope != RetentionScope.NAMESPACE or request.name is not None:
            raise AdapterUserError(message="BigQuery retention requires namespace scope")
        minimum_days: int = 2
        maximum_days: int = 7
        if not minimum_days <= request.desired_days <= maximum_days:
            raise AdapterUserError(message="BigQuery retention days must be between 2 and 7")
        return self._build_dataset_id(database=request.database, schema=request.schema)

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
            f"DELETE FROM {table_name} AS target WHERE EXISTS ("
            "SELECT 1 FROM ("
            "SELECT node_type, node_name, ts, run_id, ROW_NUMBER() OVER ("
            "PARTITION BY node_type, node_name "
            "ORDER BY ts DESC, run_id DESC"
            f") AS __sqlbuild_history_rank FROM {table_name}"
            ") AS stale "
            f"WHERE __sqlbuild_history_rank > {retain_versions} "
            "AND target.node_type = stale.node_type "
            "AND target.node_name = stale.node_name "
            "AND target.ts = stale.ts "
            "AND target.run_id = stale.run_id"
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
            f"DELETE FROM {table_name} AS target WHERE EXISTS ("
            "SELECT 1 FROM ("
            "SELECT source_name, target_database, target_schema, target_name, observed_at, run_id, "
            "ROW_NUMBER() OVER ("
            "PARTITION BY source_name, target_database, target_schema, target_name "
            "ORDER BY observed_at DESC, run_id DESC"
            f") AS __sqlbuild_history_rank FROM {table_name}"
            ") AS stale "
            f"WHERE __sqlbuild_history_rank > {retain_versions} "
            "AND target.source_name = stale.source_name "
            "AND target.target_database IS NOT DISTINCT FROM stale.target_database "
            "AND target.target_schema IS NOT DISTINCT FROM stale.target_schema "
            "AND target.target_name IS NOT DISTINCT FROM stale.target_name "
            "AND target.observed_at = stale.observed_at "
            "AND target.run_id = stale.run_id"
            ")"
        )

    def supports_relation_age_metadata(self) -> bool:
        return True

    def supports_table_freshness_metadata(self) -> bool:
        return True

    def probe_relation_read(self, *, connection: Any, relation: str) -> RelationReadProbe:
        return run_relation_read_probe(
            execute=self.execute,
            connection=connection,
            relation=relation,
            classify_not_found=self._classify_relation_not_found,
        )

    @classmethod
    def _classify_relation_not_found(cls, error: BaseException) -> RelationReadStatus | None:
        return (
            RelationReadStatus.MISSING
            if isinstance(error, Exception) and cls._is_google_not_found(error)
            else None
        )

    def get_table_freshness_metadata(
        self,
        *,
        connection: _BigQueryConnection,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> TableFreshnessMetadata:
        request: TableFreshnessRequest = TableFreshnessRequest(
            database=database,
            schema=schema,
            name=name,
        )
        metadata: TableFreshnessMetadata = self.get_tables_freshness_metadata(
            connection=connection,
            requests=(request,),
        )[request]
        if metadata.status != TableFreshnessStatus.OBSERVED:
            raise AdapterUserError(message=metadata.message or f"freshness unavailable for {name}")
        return metadata

    def relation_usage(
        self,
        *,
        connection: Any,
        relations: tuple[RelationInfo, ...],
        window_seconds: int,
    ) -> RelationUsageReport:
        """Read candidate reads from the region's INFORMATION_SCHEMA.JOBS (180 days kept)."""

        return bigquery_relation_usage(
            connection=connection, relations=relations, window_seconds=window_seconds
        )

    def get_tables_freshness_metadata(
        self,
        *,
        connection: _BigQueryConnection,
        requests: tuple[TableFreshnessRequest, ...],
    ) -> dict[TableFreshnessRequest, TableFreshnessMetadata]:
        if not requests:
            return {}
        results: dict[TableFreshnessRequest, TableFreshnessMetadata] = {}
        lookup_requests: list[TableFreshnessRequest] = []
        request: TableFreshnessRequest
        for request in requests:
            if request.schema is None:
                results[request] = TableFreshnessMetadata.unavailable(
                    message="BigQuery table freshness metadata requires a dataset"
                )
            elif TABLE_NAME_WILDCARD in request.name:
                results[request] = TableFreshnessMetadata.unavailable(
                    message="BigQuery metadata freshness does not support wildcard tables; "
                    "configure a freshness column or query instead"
                )
            else:
                lookup_requests.append(request)

        requests_by_location_project: dict[tuple[str, str | None], list[TableFreshnessRequest]] = {}
        for request in lookup_requests:
            location: str = self._metadata_location(connection=connection, request=request)
            requests_by_location_project.setdefault((location, request.database), []).append(
                request
            )

        grouped_requests: list[TableFreshnessRequest]
        for (location, database), grouped_requests in requests_by_location_project.items():
            clauses: str = " OR ".join(
                "(UPPER(table_schema) = UPPER('"
                + self._escape_sql_string(str(request.schema))
                + "') AND UPPER(table_name) = UPPER('"
                + self._escape_sql_string(request.name)
                + "'))"
                for request in grouped_requests
            )
            information_schema: str = self._table_storage_information_schema(
                database=database,
                location=location,
            )
            try:
                cursor: _BigQueryCursor = self.execute(
                    connection=connection,
                    sql="SELECT table_schema, table_name, storage_last_modified_time "
                    f"FROM {information_schema} WHERE {clauses}",
                )
                rows: list[tuple[object, ...]] = cursor.fetchall()
            except AdapterUserError:
                rows = self._legacy_tables_freshness_rows(
                    connection=connection,
                    database=database,
                    requests=grouped_requests,
                )
            requests_by_key: dict[tuple[str, str], TableFreshnessRequest] = {
                (str(request.schema).lower(), request.name.lower()): request
                for request in grouped_requests
            }
            row: tuple[object, ...]
            for row in rows:
                matched_request: TableFreshnessRequest | None = requests_by_key.get(
                    (str(row[0]).lower(), str(row[1]).lower())
                )
                if matched_request is None:
                    continue
                if row[2] is None:
                    results[matched_request] = TableFreshnessMetadata.unavailable(
                        message="BigQuery table freshness metadata is missing "
                        f"storage_last_modified_time for {matched_request.name}"
                    )
                    continue
                results[matched_request] = TableFreshnessMetadata(
                    data_version=row[2],
                    value_kind="timestamp",
                    observed_at=row[2] if isinstance(row[2], datetime) else None,
                )
        return complete_table_freshness_results(
            requests=requests, results=results, adapter_label="BigQuery"
        )

    def _legacy_tables_freshness_rows(
        self,
        *,
        connection: _BigQueryConnection,
        database: str | None,
        requests: list[TableFreshnessRequest],
    ) -> list[tuple[object, ...]]:
        rows: list[tuple[object, ...]] = []
        requests_by_schema: dict[str, list[TableFreshnessRequest]] = {}
        request: TableFreshnessRequest
        for request in requests:
            requests_by_schema.setdefault(str(request.schema), []).append(request)
        schema: str
        schema_requests: list[TableFreshnessRequest]
        for schema, schema_requests in requests_by_schema.items():
            dataset_id: str = self._build_dataset_id(database=database, schema=schema)
            table_clauses: str = " OR ".join(
                "UPPER(table_id) = UPPER('" + self._escape_sql_string(request.name) + "')"
                for request in schema_requests
            )
            cursor: _BigQueryCursor = self.execute(
                connection=connection,
                sql="SELECT '"
                + self._escape_sql_string(schema)
                + "' AS table_schema, table_id AS table_name, "
                f"TIMESTAMP_MILLIS(last_modified_time) AS storage_last_modified_time "
                f"FROM `{dataset_id}.__TABLES__` WHERE {table_clauses}",
            )
            rows.extend(cursor.fetchall())
        return rows

    def persists_python_functions(self) -> bool:
        return True

    def python_functions_inherit_default_namespace(self) -> bool:
        return True

    def supports_unqualified_function_fingerprints(self) -> bool:
        return False

    def max_statement_size(self) -> StatementSizeLimit | None:
        """Unresolved GoogleSQL has a 1 MB cap: https://cloud.google.com/bigquery/quotas#query_jobs."""

        return (1_048_576, "bytes")

    def maximum_identifier_length(self) -> int:
        """Return the maximum unqualified identifier length supported by the adapter."""

        return self.max_identifier_length

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
        null_row: tuple[Any, ...] = cast(
            tuple[Any, ...], self.execute(connection=connection, sql=null_count_sql).fetchone()
        )
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
        duplicate_row: tuple[Any, ...] = cast(
            tuple[Any, ...], self.execute(connection=connection, sql=duplicate_count_sql).fetchone()
        )
        if int(duplicate_row[0]) > 0:
            raise AdapterUserError(
                message=f"row diff {relation_label} relation contains duplicate unique_key values"
            )

    def _render_row_diff_key_hash(
        self,
        *,
        keys: tuple[str, ...],
        alias: str,
        seed: int,
    ) -> str:
        """Render a stable BigQuery hash over a canonical composite key."""

        components: list[str] = [f"'{seed}'"]
        key: str
        for key in keys:
            value_sql: str = f"CAST({alias}.{key} AS STRING)"
            components.append(f"CONCAT(CAST(LENGTH({value_sql}) AS STRING), ':', {value_sql})")
        separated: list[str] = []
        component: str
        for component in components:
            if separated:
                separated.append("'|'")
            separated.append(component)
        return f"TO_HEX(MD5(CONCAT({', '.join(separated)})))"

    def _render_row_diff_union_operator(self) -> str:
        """Render BigQuery's explicit distinct union operator."""

        return "UNION DISTINCT"

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

    def sql_analysis_dialect(self) -> str | None:
        """Return the SQL analysis dialect name for this adapter, if any."""

        return self.sql_analysis_dialect_name

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

    def render_loader_logical_type(self, type_name: LoaderLogicalType) -> str:
        match type_name:
            case LoaderLogicalType.BOOLEAN:
                return "BOOL"
            case LoaderLogicalType.INTEGER:
                return "INT64"
            case LoaderLogicalType.FLOAT:
                return "FLOAT64"
            case LoaderLogicalType.STRING:
                return "STRING"
            case LoaderLogicalType.TIMESTAMP:
                return "TIMESTAMP"
            case LoaderLogicalType.DATE:
                return "DATE"
            case LoaderLogicalType.JSON:
                return "JSON"

    def render_typed_scalar(self, *, value: SqlValue) -> str:
        if value.kind == SqlValueKind.DECIMAL:
            return f"NUMERIC {self._quote_sql_string(str(_typed_scalar_payload(value)))}"
        return _render_ansi_typed_scalar(value=value)

    def render_typed_value_list(self, *, value: SqlValue) -> str:
        return _render_typed_value_list(value=value, render_scalar=self.render_typed_scalar)

    def render_typed_array(self, *, value: SqlValue) -> str:
        if any(item.kind in _COLLECTION_SQL_VALUE_KINDS for item in _typed_collection_items(value)):
            raise UnsupportedTypedSqlRenderingError(
                adapter_name=self.adapter_name,
                rendering="array",
                reason="does not support nested arrays",
            )
        return "[" + self._render_typed_array_items(value) + "]"

    def render_typed_object(self, *, value: SqlValue) -> str:
        return f"JSON {self._quote_sql_string(_encode_typed_json(value))}"

    def render_loader_value_literal(
        self, *, value: object, logical_type: LoaderLogicalType | None
    ) -> str:
        if value is None:
            return "NULL"
        if logical_type == LoaderLogicalType.JSON:
            return f"JSON {self._quote_sql_string(json.dumps(value, sort_keys=True))}"
        if isinstance(value, bool):
            return "TRUE" if value else "FALSE"
        if isinstance(value, int | float | Decimal):
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
                f"{self._loader_row_sql_type(column_sql_types.get(column_name))}) "
                f"AS {self.render_identifier(column_name)}"
                for column_name in column_names
            )
            return f"SELECT {projections} WHERE 1 = 0"
        selects: list[str] = []
        row: dict[str, object]
        for row in rows:
            projections = ", ".join(
                self._bigquery_loader_rows_projection_sql(
                    column_name=column_name,
                    literal=self.render_loader_value_literal(
                        value=row.get(column_name),
                        logical_type=inferred_types.get(column_name),
                    ),
                    column_sql_types=column_sql_types,
                )
                for column_name in column_names
            )
            selects.append(f"SELECT {projections}")
        return " UNION ALL ".join(selects)

    def _loader_row_sql_type(self, column_type: str | None) -> str:
        if column_type is None:
            return "STRING"
        return self._to_bigquery_type(column_type)

    def _bigquery_loader_rows_projection_sql(
        self,
        *,
        literal: str,
        column_name: str,
        column_sql_types: dict[str, str],
    ) -> str:
        sql_type: str | None = column_sql_types.get(column_name)
        quoted_column: str = self.render_identifier(column_name)
        if sql_type is None:
            return f"{literal} AS {quoted_column}"
        return f"CAST({literal} AS {self._loader_row_sql_type(sql_type)}) AS {quoted_column}"

    def render_identifier(self, name: str) -> str:
        return "`" + name.replace("`", "``") + "`"

    def render_exact_identifier(self, name: str) -> str:
        return self.render_identifier(name)

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

    def __init__(self) -> None:
        self._location: str | None = None

    def connect(self, config: dict[str, Any]) -> _BigQueryConnection:
        """Open a BigQuery client from ADC or an optional service account file."""

        project: object | None = config.get("project")
        if not isinstance(project, str) or not project.strip():
            raise AdapterUserError(
                message="BigQuery connection requires non-empty 'project'",
                code="A101",
                help=(
                    "set the Google Cloud project in sqlbuild_local.toml (or in the active "
                    "target's connection):\n"
                    "            [connection]\n"
                    '            project = "my-project"'
                ),
            )

        try:
            from google.cloud import bigquery
        except ImportError as error:
            raise AdapterUserError(
                message="BigQuery adapter requires optional dependency google-cloud-bigquery. "
                "Install with: sqlbuild[bigquery]",
                code="A102",
            ) from error

        location: object | None = config.get("location")
        credentials_path: object | None = config.get("credentials_path")
        project_name: str = project.strip()
        location_name: str | None = str(location) if location is not None else None
        if credentials_path is not None:
            try:
                from google.oauth2 import service_account
            except ImportError as error:
                raise AdapterUserError(
                    message=(
                        "BigQuery credentials_path requires google-auth service account support"
                    ),
                    code="A103",
                ) from error
            credentials: Any | None = service_account.Credentials.from_service_account_file(
                str(credentials_path)
            )
        else:
            credentials = None

        self._location = location_name
        return _BigQueryConnection(
            client=bigquery.Client(
                project=project_name,
                location=location_name,
                credentials=credentials,
            ),
            location=self._location,
        )

    def _execute(self, *, connection: _BigQueryConnection, sql: str) -> _BigQueryCursor:
        """Execute a SQL statement against BigQuery."""

        log_sql(logger=logging.getLogger("sqlbuild.adapter.bigquery"), sql=sql)
        try:
            return connection.execute(sql)
        except Exception as error:
            raise AdapterUserError(
                message=self._format_bigquery_error(error),
                code="A104",
            ) from error

    @staticmethod
    def _format_bigquery_error(error: Exception) -> str:
        message_parts: list[str] = []
        error_details: object | None = getattr(error, "errors", None)
        if isinstance(error_details, list):
            detail: object
            for detail in error_details:
                if not isinstance(detail, dict):
                    continue
                detail_message: object | None = detail.get("message")
                if detail_message is None:
                    continue
                detail_text: str = str(detail_message)
                if detail_text not in message_parts:
                    message_parts.append(detail_text)
        error_text: str = str(error)
        if error_text not in message_parts:
            message_parts.append(error_text)
        return "\n".join(message_parts)

    def query(self, *, connection: Any, sql: str, limit: int | None) -> QueryResult:
        """Execute SQL and return normalized rows for ad hoc query output."""

        log_sql(logger=logging.getLogger("sqlbuild.adapter.bigquery"), sql=sql)
        with StatementLifecycle(adapter="bigquery", sql=sql, intent="query") as lifecycle:
            job: Any = connection.query_job(sql=sql)
            raw_job_id: object | None = getattr(job, "job_id", None)
            job_id: str | None = raw_job_id if isinstance(raw_job_id, str) else None
            lifecycle.submitted(job_id=job_id)
            try:
                statement_type: str | None = getattr(job, "statement_type", None)
                if statement_type is not None and statement_type != SELECT_STATEMENT_TYPE:
                    job.result()
                    lifecycle.completed(job_id=job_id, affected_rows=affected_rows(job=job))
                    return QueryResult()
                cursor: Any = _BigQueryCursor(job.result())
                description: Any | None = getattr(cursor, "description", None)
                if description is None:
                    lifecycle.completed(job_id=job_id, row_count=0)
                    return QueryResult()
                columns: tuple[str, ...] = tuple(str(column[0]) for column in description)
                if limit is None:
                    rows: tuple[tuple[object, ...], ...] = tuple(cursor.fetchall())
                    lifecycle.completed(job_id=job_id, row_count=len(rows))
                    return QueryResult(columns=columns, rows=rows)
                fetched_rows: list[tuple[object, ...]] = cursor.fetchmany(limit + 1)
                returned_rows: tuple[tuple[object, ...], ...] = tuple(fetched_rows[:limit])
                lifecycle.completed(job_id=job_id, row_count=len(returned_rows))
                return QueryResult(
                    columns=columns,
                    rows=returned_rows,
                    truncated=len(fetched_rows) > limit,
                )
            except Exception as error:
                lifecycle.failed(error=error, job_id=job_id)
                raise

    def close(self, connection: _BigQueryConnection) -> None:
        """Close a BigQuery client."""

        connection.close()

    def supports_transactions(self) -> bool:
        return False

    def interrupt_connection(self, connection: Any) -> bool:
        """Request cancellation of the connection's jobs; BigQuery stops them asynchronously."""

        connection.cancel_active_jobs()
        return True

    def lists_relations_authoritatively(self, *, database: str | None, schema: str) -> bool:
        """Return whether ``list_relations`` reports every relation in this scope."""

        del database, schema
        return True

    def supports_zero_copy_clone(self) -> bool:
        return True

    def supports_durable_clone(self) -> bool:
        return True

    def star_exclude_keyword(self) -> str:
        """BigQuery uses EXCEPT for SELECT * EXCEPT."""

        return "EXCEPT"

    def default_schema(self) -> str | None:
        """BigQuery projects should provide dataset explicitly."""

        return None

    def default_database(self) -> str | None:
        """BigQuery projects should provide project explicitly."""

        return None

    def render_qualified_name(
        self,
        *,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> str | None:
        """Render BigQuery relation names with full-path backtick quoting."""

        if database is not None and schema is not None:
            return self._quote_identifier_path(f"{database}.{schema}.{name}")
        if schema is not None:
            return self._quote_identifier_path(f"{schema}.{name}")
        return None

    def render_framework_type(self, type_name: FrameworkType) -> str:
        """Render BigQuery internal framework types explicitly."""

        match type_name:
            case FrameworkType.STRING:
                return "STRING"
            case FrameworkType.INTEGER:
                return "INT64"
            case FrameworkType.TIMESTAMP:
                return "TIMESTAMP"

    def render_source_expression_cast(
        self, *, expression: str, target_type: str, alias: str
    ) -> str:
        """Render BigQuery source expression type-enforcement casts explicitly."""

        return f"CAST({expression} AS {self._to_bigquery_type(target_type)}) AS {alias}"

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
        return f"(SELECT * EXCEPT ({exclude_list}), {cast_clause} FROM {source_relation})"

    def requires_derived_table_aliases(self) -> bool:
        """BigQuery does not require aliases for derived table factors."""

        return False

    def render_set_difference_operator(self) -> str:
        """Render the BigQuery set-difference operator explicitly."""

        return "EXCEPT DISTINCT"

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

    def expression_inference_profile(self) -> ExpressionInferenceProfile:
        return ExpressionInferenceProfile(
            sql_analysis_dialect=self.sql_analysis_dialect(),
            function_nullability_rules={
                "IF": conditional_result_nullability,
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

    def default_table_promotion_mode(self) -> TablePromotionMode:
        return TablePromotionMode.STAGED

    def default_promotion_strategy(self) -> PromotionStrategy:
        return PromotionStrategy.ATOMIC_REPLACE

    def supports_python_functions(self) -> bool:
        return True

    def supports_table_functions(self) -> bool:
        return True

    def render_create_schema(self, *, database: str | None, schema: str) -> tuple[str, ...]:
        target: str = f"{database}.{schema}" if database is not None else schema
        sql: str = f"CREATE SCHEMA IF NOT EXISTS {self._quote_identifier_path(target)}"
        if self._location is not None:
            sql += f" OPTIONS(location = '{self._location}')"
        return (sql,)

    def render_drop_schema(self, *, database: str | None, schema: str) -> tuple[str, ...]:
        target: str = f"{database}.{schema}" if database is not None else schema
        return (f"DROP SCHEMA IF EXISTS {self._quote_identifier_path(target)} CASCADE",)

    def render_create_table_as(self, *, destination: str, sql: str) -> tuple[str, ...]:
        return (f"CREATE OR REPLACE TABLE {self._quote_identifier_path(destination)} AS {sql}",)

    def render_create_view_as(self, *, destination: str, sql: str) -> tuple[str, ...]:
        return (f"CREATE OR REPLACE VIEW {self._quote_identifier_path(destination)} AS {sql}",)

    def render_udf_call(self, *, target: str, call_suffix_sql: str) -> str:
        return f"{self._quote_identifier_path(target)}{call_suffix_sql}"

    def render_table_function_call(self, *, target: str, call_suffix_sql: str) -> str:
        return f"{self._quote_identifier_path(target)}{call_suffix_sql}"

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
        argument_sql: str = ", ".join(f"{argument.name} {argument.type}" for argument in arguments)
        if return_columns:
            if language != FunctionLanguage.SQL:
                raise AdapterUserError(message="BigQuery table functions must use SQL language")
            del returns, runtime_version, entry_point, packages
            return (
                f"CREATE OR REPLACE TABLE FUNCTION {self._quote_identifier_path(destination)}"
                f"({argument_sql})\n"
                f"AS (\n{body_sql}\n)",
            )
        if language == FunctionLanguage.PYTHON:
            if runtime_version is None or entry_point is None:
                raise AdapterUserError(
                    message="BigQuery Python UDFs require runtime_version and entry_point"
                )
            package_sql: str = ""
            if packages:
                package_values: str = ", ".join(f"'{package}'" for package in packages)
                package_sql = f",\n  packages = [{package_values}]"
            return (
                "CREATE OR REPLACE FUNCTION "
                f"{self._quote_identifier_path(destination)}({argument_sql})\n"
                f"RETURNS {returns}\n"
                "LANGUAGE python\n"
                "OPTIONS(\n"
                f'  runtime_version = "python-{runtime_version}",\n'
                f'  entry_point = "{entry_point}"'
                f"{package_sql}\n"
                ")\n"
                f"AS r'''\n{body_sql}\n'''",
            )
        return (
            "CREATE OR REPLACE FUNCTION "
            f"{self._quote_identifier_path(destination)}({argument_sql})\n"
            f"RETURNS {returns}\n"
            f"AS (\n{body_sql}\n)",
        )

    def render_append(
        self, *, destination: str, sql: str, columns: tuple[str, ...] | None = None
    ) -> tuple[str, ...]:
        quoted_target: str = self._quote_identifier_path(destination)
        if columns is not None:
            col_list: str = ", ".join(self.render_identifier(column) for column in columns)
            return (f"INSERT INTO {quoted_target} ({col_list}) {sql}",)
        return (f"INSERT INTO {quoted_target} {sql}",)

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
        insert_clause: str = "INSERT ROW"
        if columns is not None:
            column_list: str = ", ".join(self.render_identifier(column) for column in columns)
            values_list: str = ", ".join(
                f"__source.{self.render_identifier(column)}" for column in columns
            )
            insert_clause = f"INSERT ({column_list}) VALUES ({values_list})"
        cursor_filter: str = (
            f"__target.{self.render_identifier(cursor_column)} >= "
            f"{self._render_cursor_bound_string(cursor_start)} "
            f"AND __target.{self.render_identifier(cursor_column)} < "
            f"{self._render_cursor_bound_string(cursor_end)}"
        )
        return (
            f"MERGE {self._quote_identifier_path(destination)} AS __target "
            f"USING ({sql}) AS __source ON FALSE "
            f"WHEN NOT MATCHED BY TARGET THEN {insert_clause} "
            f"WHEN NOT MATCHED BY SOURCE AND {cursor_filter} THEN DELETE",
        )

    def render_delete_insert(
        self,
        *,
        destination: str,
        sql: str,
        unique_key: tuple[str, ...],
        columns: tuple[str, ...] | None = None,
    ) -> tuple[str, ...]:
        if columns is None:
            quoted_target: str = self._quote_identifier_path(destination)
            staged: str = f"{quoted_target}__delete_insert"
            key_condition: str = " AND ".join(
                f"{quoted_target}.{self.render_identifier(k)} = "
                f"{staged}.{self.render_identifier(k)}"
                for k in unique_key
            )
            create_staged: tuple[str, ...] = self.render_create_table_as(
                destination=staged, sql=sql
            )
            delete_sql: str = f"DELETE FROM {quoted_target} USING {staged} WHERE {key_condition}"
            insert_stmts: tuple[str, ...] = self.render_append(
                destination=quoted_target,
                sql=f"SELECT * FROM {staged}",
                columns=columns,
            )
            drop_staged: tuple[str, ...] = self.render_drop(destination=staged, if_exists=True)
            return (*create_staged, delete_sql, *insert_stmts, *drop_staged)
        return self.render_merge(
            destination=destination,
            sql=sql,
            unique_key=unique_key,
            source_columns=columns,
        )

    def render_drop(self, *, destination: str, if_exists: bool = True) -> tuple[str, ...]:
        exists_clause: str = " IF EXISTS" if if_exists else ""
        return (
            f"DROP TABLE{exists_clause} {self._quote_identifier_path(destination)}",
            f"DROP VIEW{exists_clause} {self._quote_identifier_path(destination)}",
        )

    def render_drop_view(self, *, destination: str, if_exists: bool = True) -> tuple[str, ...]:
        exists_clause: str = " IF EXISTS" if if_exists else ""
        return (f"DROP VIEW{exists_clause} {self._quote_identifier_path(destination)}",)

    def render_rename(self, *, origin: str, destination: str) -> tuple[str, ...]:
        destination_name: str = self._strip_identifier_quotes(destination).split(".")[-1]
        return (f"ALTER TABLE {self._quote_identifier_path(origin)} RENAME TO {destination_name}",)

    def render_rename_view(self, *, origin: str, destination: str) -> tuple[str, ...]:
        del origin, destination
        raise AdapterUserError(message="BigQuery does not support renaming views")

    def render_swap(self, *, left: str, right: str) -> tuple[str, ...]:
        raise AdapterUserError(message="BigQuery does not support atomic table swap")

    def render_clone(
        self,
        *,
        origin: str,
        destination: str,
        hard_copy: bool = False,
        origin_is_transient: bool = False,
    ) -> tuple[str, ...]:
        del origin_is_transient
        if not hard_copy:
            return (
                f"CREATE TABLE {self._quote_identifier_path(destination)} "
                f"CLONE {self._quote_identifier_path(origin)}",
            )
        return self.render_create_table_as(destination=destination, sql=f"SELECT * FROM {origin}")

    def render_durable_clone(
        self, *, origin: str, destination: str, origin_is_transient: bool = False
    ) -> tuple[str, ...]:
        del origin_is_transient
        return (
            f"CREATE TABLE {self._quote_identifier_path(destination)} "
            f"CLONE {self._quote_identifier_path(origin)}",
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
        quoted_stage: str = self._quote_identifier_path(stage)
        quoted_origin: str = self._quote_identifier_path(origin)
        return MigrationStagePlan(
            transfer=MigrationTransfer.CLONE,
            statements=(f"CREATE TABLE {quoted_stage} CLONE {quoted_origin}",),
            fallback_statements=(f"CREATE TABLE {quoted_stage} COPY {quoted_origin}",),
            is_clone_refusal=is_bigquery_clone_refusal,
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
        location: str = self._metadata_location(
            connection=connection,
            request=TableFreshnessRequest(database=database, schema=schema, name=name),
        )
        region: str = location.strip().lower()
        region = region if region.startswith("region-") else f"region-{region}"
        project: str = self._strip_identifier_quotes(database or str(connection.client.project))
        rows: list[tuple[Any, ...]] = self.execute(
            connection=connection,
            sql=(
                f"SELECT privilege_type, grantee FROM `{project}.{region}`"
                ".INFORMATION_SCHEMA.OBJECT_PRIVILEGES "
                f"WHERE object_schema = '{self._escape_sql_string(schema)}' "
                f"AND object_name = '{self._escape_sql_string(name)}' "
                "ORDER BY grantee, privilege_type"
            ),
        ).fetchall()
        return bigquery_relation_grants(rows)

    def render_relation_grants(
        self, *, grants: tuple[RelationGrant, ...], destination: str, columns: tuple[str, ...]
    ) -> tuple[str, ...]:
        del columns
        return render_bigquery_view_grants(
            grants=grants, destination=self._quote_identifier_path(destination)
        )

    def render_replace_view_keeping_grants(
        self, *, destination: str, sql: str
    ) -> tuple[str, ...] | None:
        del destination, sql
        return None

    def read_view_definition(
        self, *, connection: Any, database: str | None, schema: str, name: str
    ) -> str | None:
        dataset: str = self._build_dataset_id(database=database, schema=schema)
        view_name: str = self._escape_sql_string(self._strip_identifier_quotes(name))
        rows: list[tuple[Any, ...]] = self.execute(
            connection=connection,
            sql=(
                f"SELECT view_definition FROM `{dataset}`.INFORMATION_SCHEMA.VIEWS "
                f"WHERE table_name = '{view_name}'"
            ),
        ).fetchall()
        return None if not rows else str(rows[0][0])

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
        return render_bigquery_view_revokes(
            grants=grants, destination=self._quote_identifier_path(destination)
        )

    def rename_view(
        self,
        *,
        connection: Any,
        origin: str,
        destination: str,
        statement_recorder: StatementRecorder,
    ) -> None:
        parts: list[str] = self._strip_identifier_quotes(origin).split(".")
        dataset: str = ".".join(parts[:-1])
        rows: list[tuple[Any, ...]] = self.execute(
            connection=connection,
            sql=(
                f"SELECT view_definition FROM `{dataset}`.INFORMATION_SCHEMA.VIEWS "
                f"WHERE table_name = '{self._escape_sql_string(parts[-1])}'"
            ),
        ).fetchall()
        if not rows:
            raise AdapterUserError(message=f"BigQuery view {origin} does not exist")
        view_grants: tuple[RelationGrant, ...] = self.read_relation_grants(
            connection=connection,
            database=".".join(parts[:-2]) or None,
            schema=parts[-2],
            name=parts[-1],
            relation_type="view",
        )
        statements: tuple[str, ...] = render_bigquery_view_move(
            origin=self._quote_identifier_path(origin),
            destination=self._quote_identifier_path(destination),
            definition=str(rows[0][0]),
            grants=self.render_relation_grants(
                grants=view_grants, destination=destination, columns=()
            ),
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

    def render_replace_table_from_relation(
        self, *, destination: str, origin: str
    ) -> tuple[str, ...]:
        return (
            "-- BigQuery execution copies this relation to the destination table with "
            "WRITE_TRUNCATE for staged atomic replace.\n"
            f"CREATE OR REPLACE TABLE {self._quote_identifier_path(destination)} AS "
            f"SELECT * FROM {self._quote_identifier_path(origin)}",
        )

    def replace_table_from_relation(
        self,
        *,
        connection: _BigQueryConnection,
        destination: str,
        origin: str,
        statement_recorder: StatementRecorder,
    ) -> None:
        from google.cloud import bigquery

        origin_table: str = self._strip_identifier_quotes(origin)
        destination_table: str = self._strip_identifier_quotes(destination)
        job_config: Any = bigquery.CopyJobConfig(
            write_disposition=bigquery.WriteDisposition.WRITE_TRUNCATE,
        )
        statement_recorder.record(f"COPY WRITE_TRUNCATE {origin} TO {destination}")
        try:
            connection.client.copy_table(
                origin_table,
                destination_table,
                job_config=job_config,
                location=connection.location,
            ).result()
        finally:
            invalidate_cached_relations(adapter=self, qualified_names=(destination, origin))

    def move_or_copy_relation(
        self,
        *,
        connection: _BigQueryConnection,
        origin: str,
        destination: str,
        remove_origin: bool,
        allow_copy_fallback: bool,
        statement_recorder: StatementRecorder,
    ) -> None:
        if not allow_copy_fallback:
            raise AdapterUserError(message="BigQuery relation move/copy requires --allow-copy")
        self.replace_table_from_relation(
            connection=connection,
            origin=origin,
            destination=destination,
            statement_recorder=statement_recorder,
        )
        if remove_origin:
            self.drop(
                connection=connection, destination=origin, statement_recorder=statement_recorder
            )

    def relation_exists(
        self,
        *,
        connection: _BigQueryConnection,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> bool:
        if schema is None:
            return False
        try:
            connection.client.get_table(
                self._build_table_id(database=database, schema=schema, name=name)
            )
        except Exception as error:
            if self._is_google_not_found(error):
                return False
            raise
        return True

    def list_relations(
        self,
        *,
        connection: _BigQueryConnection,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[RelationInfo, ...]:
        if not schemas:
            return ()
        relations: list[RelationInfo] = []
        schema: str
        for schema in schemas:
            dataset_id: str = self._build_dataset_id(database=database, schema=schema)
            try:
                tables: Any = connection.client.list_tables(dataset_id)
                table: Any
                for table in tables:
                    table_name: str = str(table.table_id)
                    if names is not None and table_name not in names:
                        continue
                    relations.append(
                        RelationInfo(
                            database=database,
                            schema=schema,
                            name=table_name,
                            relation_type=str(getattr(table, "table_type", "TABLE")).lower(),
                        )
                    )
            except Exception as error:
                if not self._is_google_not_found(error):
                    raise
        return tuple(relations)

    def with_relation_age_metadata(
        self,
        *,
        connection: _BigQueryConnection,
        relations: tuple[RelationInfo, ...],
    ) -> tuple[RelationInfo, ...]:
        timestamps: dict[tuple[str | None, str, str], tuple[datetime | None, datetime | None]] = {}
        dataset_key: tuple[str | None, str]
        for dataset_key in sorted(
            {(relation.database, str(relation.schema)) for relation in relations},
            key=lambda key: (key[0] or "", key[1]),
        ):
            timestamps.update(
                self._dataset_relation_timestamps(
                    connection=connection, database=dataset_key[0], schema=dataset_key[1]
                )
            )
        return tuple(
            replace(
                relation,
                created_at=timestamps.get(self._age_key(relation), (None, None))[0],
                last_altered_at=timestamps.get(self._age_key(relation), (None, None))[1],
            )
            for relation in relations
        )

    def _dataset_relation_timestamps(
        self, *, connection: _BigQueryConnection, database: str | None, schema: str
    ) -> dict[tuple[str | None, str, str], tuple[datetime | None, datetime | None]]:
        dataset_id: str = self._build_dataset_id(database=database, schema=schema)
        try:
            cursor: _BigQueryCursor = self.execute(
                connection=connection,
                sql="SELECT table_id, TIMESTAMP_MILLIS(creation_time), "
                f"TIMESTAMP_MILLIS(last_modified_time) FROM `{dataset_id}.__TABLES__`",
            )
            rows: list[tuple[Any, ...]] = cursor.fetchall()
        except Exception as error:
            if not self._is_google_not_found(error):
                raise
            return {}
        return {
            (database, schema, str(row[0])): (
                relation_age_timestamp_utc(row[1]),
                relation_age_timestamp_utc(row[2]),
            )
            for row in rows
        }

    @staticmethod
    def _age_key(relation: RelationInfo) -> tuple[str | None, str, str]:
        return (relation.database, str(relation.schema), relation.name)

    def list_functions(
        self,
        *,
        connection: _BigQueryConnection,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[FunctionInfo, ...]:
        if not schemas:
            return ()
        functions: list[FunctionInfo] = []
        schema: str
        for schema in schemas:
            dataset_id: str = self._build_dataset_id(database=database, schema=schema)
            query: str = (
                "SELECT routine_name, routine_schema, routine_type "
                f"FROM `{dataset_id}`.INFORMATION_SCHEMA.ROUTINES WHERE 1=1"
            )
            if names:
                quoted_names: str = ", ".join(self._quote_sql_string(name) for name in names)
                query += f" AND routine_name IN ({quoted_names})"
            try:
                cursor: _BigQueryCursor = self.execute(connection=connection, sql=query)
                row: tuple[Any, ...]
                for row in cursor.fetchall():
                    functions.append(
                        FunctionInfo(
                            database=database,
                            schema=None if row[1] is None else str(row[1]),
                            name=str(row[0]),
                            function_type=str(row[2]),
                        )
                    )
            except Exception as error:
                if not self._is_google_not_found(error):
                    raise
        return tuple(functions)

    def get_columns(
        self,
        *,
        connection: _BigQueryConnection,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> tuple[ColumnInfo, ...]:
        if schema is None:
            return ()
        table: Any = connection.client.get_table(
            self._build_table_id(database=database, schema=schema, name=name)
        )
        return tuple(self._column_info_from_schema_field(field) for field in table.schema)

    def get_all_columns(
        self,
        *,
        connection: _BigQueryConnection,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> dict[str, tuple[ColumnInfo, ...]]:
        relations: tuple[RelationInfo, ...] = self.list_relations(
            connection=connection,
            database=database,
            schemas=schemas,
            names=names,
        )
        return {
            relation.name: self.get_columns(
                connection=connection,
                database=database,
                schema=relation.schema,
                name=relation.name,
            )
            for relation in relations
        }

    def describe_relation(
        self, *, connection: _BigQueryConnection, relation: str
    ) -> tuple[ColumnInfo, ...]:
        """Return BigQuery relation metadata using the tables API."""

        table: Any = connection.client.get_table(self._strip_identifier_quotes(relation))
        return tuple(self._column_info_from_schema_field(field) for field in table.schema)

    def query_column_names(self, *, connection: Any, sql: str) -> tuple[str, ...]:
        cursor: Any = self.execute(
            connection=connection, sql=f"SELECT * FROM ({sql}) AS __describe_source LIMIT 0"
        )
        description: Any | None = cursor.description
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
        """Build a BigQuery cursor filter clause with typed bound literals."""

        if cursor_column is None or start_cursor is None:
            return ""
        clauses: list[str] = [
            f"{cursor_column} >= {self._render_cursor_value_literal(start_cursor)}"
        ]
        if end_cursor is not None:
            clauses.append(f"{cursor_column} < {self._render_cursor_value_literal(end_cursor)}")
        return " AND ".join(clauses)

    def load_seed(
        self,
        *,
        connection: _BigQueryConnection,
        destination: str,
        file_path: Path,
        columns: tuple[ColumnInfo, ...],
        csv_settings: SeedCsvSettings = DEFAULT_SEED_CSV_SETTINGS,
        replace: bool = True,
        infer_types: bool = False,
        statement_recorder: StatementRecorder,
    ) -> None:
        del infer_types
        from google.cloud import bigquery

        write_disposition: str = "WRITE_TRUNCATE" if replace else "WRITE_APPEND"
        job_config: Any = bigquery.LoadJobConfig(
            schema=[
                bigquery.SchemaField(column.name, self._to_bigquery_type(column.type))
                for column in columns
            ],
            source_format=bigquery.SourceFormat.CSV,
            skip_leading_rows=1,
            write_disposition=write_disposition,
        )
        if csv_settings.delimiter is not None:
            job_config.field_delimiter = csv_settings.delimiter
        if csv_settings.quotechar is not None:
            job_config.quote_character = csv_settings.quotechar
        if csv_settings.encoding is not None:
            job_config.encoding = csv_settings.encoding
        statement_recorder.record(
            f"LOAD CSV {file_path} INTO {destination} ({', '.join(col.name for col in columns)})"
        )
        try:
            with file_path.open("rb") as seed_file:
                connection.client.load_table_from_file(
                    seed_file,
                    self._strip_identifier_quotes(destination),
                    job_config=job_config,
                    location=connection.location,
                ).result()
        finally:
            invalidate_cached_relations(adapter=self, qualified_names=(destination,))

    def add_columns(
        self,
        *,
        connection: _BigQueryConnection,
        destination: str,
        columns: tuple[ColumnInfo, ...],
        statement_recorder: StatementRecorder,
    ) -> None:
        statements: tuple[str, ...] = tuple(
            "ALTER TABLE "
            f"{self._quote_identifier_path(destination)} ADD COLUMN "
            f"{self.render_identifier(column.name)} {self._to_bigquery_type(column.type)}"
            for column in columns
        )
        statement_recorder.record_many(statements)
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

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
        statement: str
        for statement in statements:
            result: Any = self.execute(connection=connection, sql=statement)
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
        if columns is None:
            columns = self.query_column_names(connection=connection, sql=sql)
        statements: tuple[str, ...] = self.render_delete_insert(
            destination=destination,
            sql=sql,
            unique_key=keys,
            columns=columns,
        )
        statement_recorder.record_many(statements)
        affected: int | None = None
        statement: str
        for statement in statements:
            result: Any = self.execute(connection=connection, sql=statement)
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
        if columns is None:
            columns = self.query_column_names(connection=connection, sql=sql)
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
        statement: str
        for statement in statements:
            result: Any = self.execute(connection=connection, sql=statement)
            affected = self.affected_row_count(cursor=result)
        return affected

    def build_row_diff_equal_expression(
        self,
        *,
        column: str,
        column_info: ColumnInfo,
        tolerances: Any | None,
    ) -> str:
        tolerance: Any | None = self.resolve_row_diff_tolerance(
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

    def normalize_row_diff_numeric_type(self, column_type: str) -> str | None:
        return normalize_numeric_family(type_sql=column_type, dialect=self.sql_analysis_dialect())

    def format_row_diff_decimal_sql(self, value: Decimal) -> str:
        return format(value, "f")

    def estimate_relation_row_count(
        self,
        *,
        connection: _BigQueryConnection,
        database: str | None,
        schema: str | None,
        name: str,
        metadata_only: bool = False,
    ) -> RelationRowCountEstimate:
        """Read ``numRows`` from the tables API; only native tables report a count."""

        if schema is None:
            return RelationRowCountEstimate(row_count=None, detail="relation has no dataset")
        table: Any = connection.client.get_table(
            self._build_table_id(database=database, schema=schema, name=name)
        )
        table_type: str = str(getattr(table, "table_type", NATIVE_TABLE_TYPE)).upper()
        num_rows: object | None = getattr(table, "num_rows", None)
        if table_type != NATIVE_TABLE_TYPE:
            return RelationRowCountEstimate(
                row_count=None, detail=f"no row count ({table_type.lower()})"
            )
        byte_count: object | None = getattr(table, "num_bytes", None) if metadata_only else None
        return RelationRowCountEstimate(
            row_count=num_rows if isinstance(num_rows, int) else None,
            detail=f"no row count ({table_type.lower()})"
            if not isinstance(num_rows, int)
            else None,
            byte_count=byte_count if isinstance(byte_count, int) else None,
            byte_count_basis="tables_api_num_bytes" if isinstance(byte_count, int) else None,
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
        return self._inspect_row_diff_coverage(
            connection=connection,
            relation=relation,
            cursor_column=cursor_column,
            start_cursor=start_cursor,
            end_cursor=end_cursor,
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
        join_condition: str = " AND ".join(f"__left.{key} = __right.{key}" for key in keys)
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
            f"AND __right.{keys[0]} IS NOT NULL AND ({equal_condition}) THEN 1 END) AS equal, "
            f"COUNT(CASE WHEN __left.{keys[0]} IS NOT NULL "
            f"AND __right.{keys[0]} IS NOT NULL AND NOT ({equal_condition}) "
            f"THEN 1 END) AS unequal, "
            f"COUNT(CASE WHEN __right.{keys[0]} IS NULL THEN 1 END) AS left_only, "
            f"COUNT(CASE WHEN __left.{keys[0]} IS NULL THEN 1 END) AS right_only"
            f"{column_count_sql} FROM __left FULL OUTER JOIN __right ON {join_condition}"
        )
        row: tuple[Any, ...] | None = self.execute(connection=connection, sql=diff_sql).fetchone()
        if row is None:
            raise AdapterUserError(message="BigQuery row diff query returned no result")
        column_results: tuple[RowDiffColumnResult, ...] = tuple(
            RowDiffColumnResult(
                name=col,
                mismatched_count=self._to_int(row[index]),
                tolerance=column_tolerances[col],
            )
            for index, col in enumerate(compare_columns, start=7)
        )
        return RowDiffResult(
            left_count=self._to_int(row[0]),
            right_count=self._to_int(row[1]),
            joined_count=self._to_int(row[2]),
            equal_count=self._to_int(row[3]),
            unequal_count=self._to_int(row[4]),
            left_only_count=self._to_int(row[5]),
            right_only_count=self._to_int(row[6]),
            column_results=column_results,
            population_count=prepared_relations.population_count or self._to_int(row[2]),
            compared_count=self._to_int(row[2]),
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
        join_condition: str = " AND ".join(f"__left.{key} = __right.{key}" for key in keys)
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
            inspect_population=False,
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
            f"SELECT {key_select_sql} "
            f"FROM __left FULL OUTER JOIN __right ON {join_condition} "
            f"WHERE {side_condition} "
            f"ORDER BY {', '.join(f'__key_{key}' for key in keys)} LIMIT {limit}"
        )
        rows: list[tuple[Any, ...]] = self.execute(connection=connection, sql=sample_sql).fetchall()
        samples: list[tuple[tuple[str, Any], ...]] = []
        for row in rows:
            sample: list[tuple[str, Any]] = []
            for index, key in enumerate(keys):
                sample.append((key, row[index]))
            samples.append(tuple(sample))
        return tuple(samples)

    def schema_exists(
        self,
        *,
        connection: _BigQueryConnection,
        database: str | None,
        schema: str,
    ) -> bool:
        try:
            connection.client.get_dataset(self._build_dataset_id(database=database, schema=schema))
        except Exception as error:
            if self._is_google_not_found(error):
                return False
            raise
        return True

    @staticmethod
    def _column_info_from_schema_field(field: Any) -> ColumnInfo:
        normalized_type: str = str(field.field_type).upper()
        if normalized_type == INTEGER_METADATA_TYPE_NAME:
            normalized_type = INTEGER_WIRE_TYPE_NAME
        elif normalized_type == FLOAT_METADATA_TYPE_NAME:
            normalized_type = FLOAT_WIRE_TYPE_NAME
        elif normalized_type == BOOLEAN_METADATA_TYPE_NAME:
            normalized_type = BOOLEAN_WIRE_TYPE_NAME
        elif normalized_type in DECIMAL_METADATA_TYPE_NAMES:
            precision: Any | None = getattr(field, "precision", None)
            scale: Any | None = getattr(field, "scale", None)
            if precision is not None and scale is not None:
                normalized_type = f"{normalized_type}({precision},{scale})"
        return ColumnInfo(name=str(field.name).lower(), type=normalized_type)

    @classmethod
    def _build_dataset_id(cls, *, database: str | None, schema: str) -> str:
        if database is None:
            return schema
        return f"{cls._strip_identifier_quotes(database)}.{cls._strip_identifier_quotes(schema)}"

    @classmethod
    def _build_table_id(cls, *, database: str | None, schema: str, name: str) -> str:
        return (
            f"{cls._build_dataset_id(database=database, schema=schema)}."
            f"{cls._strip_identifier_quotes(name)}"
        )

    def _metadata_location(
        self, *, connection: _BigQueryConnection, request: TableFreshnessRequest
    ) -> str:
        if connection.location is not None:
            return connection.location
        if request.schema is None:
            raise AdapterUserError(message="BigQuery table freshness metadata requires a dataset")
        dataset: Any = connection.client.get_dataset(
            self._build_dataset_id(database=request.database, schema=request.schema)
        )
        location: object | None = getattr(dataset, "location", None)
        if location is None:
            raise AdapterUserError(
                message="BigQuery table freshness metadata could not determine location "
                f"for {request.schema}"
            )
        return str(location)

    @classmethod
    def _table_storage_information_schema(cls, *, database: str | None, location: str) -> str:
        normalized_location: str = location.strip().lower()
        region_name: str = (
            normalized_location
            if normalized_location.startswith("region-")
            else f"region-{normalized_location}"
        )
        if database is None:
            return f"`{region_name}`.INFORMATION_SCHEMA.TABLE_STORAGE"
        project_name: str = cls._strip_identifier_quotes(database)
        return f"`{project_name}.{region_name}`.INFORMATION_SCHEMA.TABLE_STORAGE"

    @staticmethod
    def _escape_sql_string(value: str) -> str:
        return value.replace("'", "''")

    @staticmethod
    def _strip_identifier_quotes(value: str) -> str:
        return value.replace("`", "")

    @classmethod
    def _quote_identifier_path(cls, value: str) -> str:
        stripped: str = cls._strip_identifier_quotes(value)
        return f"`{stripped}`"

    @staticmethod
    def _is_google_not_found(error: Exception) -> bool:
        not_found_status_code: int = 404
        return (
            error.__class__.__name__ == NOT_FOUND_ERROR_CLASS_NAME
            or getattr(error, "code", None) == not_found_status_code
        )

    @staticmethod
    def _to_bigquery_type(column_type: str) -> str:
        normalized: str = column_type.upper()
        if any(token in normalized for token in ("CHAR", "TEXT", "STRING", "VARCHAR")):
            return "STRING"
        if BOOLEAN_WIRE_TYPE_NAME in normalized:
            return BOOLEAN_WIRE_TYPE_NAME
        if TIMESTAMP_TYPE_TOKEN in normalized:
            return TIMESTAMP_TYPE_TOKEN
        if normalized == DATE_TYPE_NAME:
            return DATE_TYPE_NAME
        if normalized == DATETIME_TYPE_NAME:
            return DATETIME_TYPE_NAME
        if any(token in normalized for token in ("FLOAT", "DOUBLE", "REAL")):
            return FLOAT_WIRE_TYPE_NAME
        if any(token in normalized for token in ("DECIMAL", "NUMERIC", "NUMBER")):
            return "NUMERIC"
        if INTEGER_TYPE_TOKEN in normalized:
            return INTEGER_WIRE_TYPE_NAME
        return normalized

    @staticmethod
    def _render_cursor_value_literal(cursor_value: CursorValue) -> str:
        if cursor_value.kind == CursorKind.INTEGER:
            return str(cursor_value.value)
        return f"TIMESTAMP '{cursor_value.value}'"

    @staticmethod
    def _to_int(value: object) -> int:
        if isinstance(value, int):
            return value
        return int(str(value))

    @staticmethod
    def _render_cursor_bound_string(value: str) -> str:
        try:
            int(value)
        except ValueError:
            return f"TIMESTAMP '{value}'"
        return value
