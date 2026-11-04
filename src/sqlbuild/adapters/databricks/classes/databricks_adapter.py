"""Databricks adapter implementation."""

from __future__ import annotations

import ast
import csv
import json
import logging
import re
from dataclasses import replace
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
from sqlbuild.adapter.contract.classes.base_adapter import (
    BaseAdapter,
    _encode_typed_json,
    _render_ansi_typed_scalar,
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
from sqlbuild.adapter.contract.classes.snapshot_sql import SnapshotSql
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.classes.unkeyed_diff import UnkeyedDiffMixin
from sqlbuild.adapter.contract.constants import (
    CANCELLABLE_DURATION_LIMIT_SECONDS,
    DIFF_LEFT_SIDE,
    DIFF_RIGHT_SIDE,
)
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.contract.main.complete_table_freshness_results import (
    complete_table_freshness_results,
)
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
    RelationType,
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
from sqlbuild.adapter.relations.main.relation_age_timestamp import relation_age_timestamp_utc
from sqlbuild.adapter.type_system.main.conditional_result_nullability import (
    conditional_result_nullability,
)
from sqlbuild.adapter.type_system.main.first_arg_nullability import first_arg_nullability
from sqlbuild.adapter.type_system.main.normalize_numeric_family import normalize_numeric_family
from sqlbuild.adapter.type_system.main.types_equal import types_equal
from sqlbuild.adapters.databricks._helpers.grants import (
    databricks_relation_grants,
    render_databricks_view_grants,
    render_databricks_view_revokes,
    show_grants_object_kind,
)
from sqlbuild.adapters.databricks._helpers.relation_usage import read_relation_usage
from sqlbuild.adapters.databricks.classes.databricks_connection import _DatabricksConnection
from sqlbuild.adapters.databricks.constants import (
    CURRENT_CATALOG_INFORMATION_SCHEMA,
    DELTA_COLUMN_MAPPING_NAME_MODE,
    DELTA_COLUMN_MAPPING_PROPERTY,
    DELTA_DEFAULT_DELETED_FILE_RETENTION_DAYS,
    DELTA_DEFAULT_LOG_RETENTION_DAYS,
    DELTA_RELATION_FORMAT,
    DESCRIBE_DETAIL_ROW_WIDTH,
    DETAILED_TABLE_INFORMATION_MARKER,
    NON_ROW_RESULT_COLUMN_NAMES,
    RELATION_NOT_FOUND_ERROR_CLASSES,
    TABLE_RELATION_METADATA_TYPES,
    TEMPORARY_FUNCTION_PREFIX,
    UNDEFINED_TABLE_SQLSTATE,
)
from sqlbuild.compiler.compile.types import FunctionLanguage
from sqlbuild.diagnostics.main.log_sql import log_sql
from sqlbuild.spec.contracts.constants import DEFAULT_SEED_CSV_SETTINGS


class DatabricksAdapter(MicrobatchMixin, ConsumptionMixin, UnkeyedDiffMixin, BaseAdapter):
    """Databricks adapter backed by databricks-sql-connector."""

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
    adapter_name: ClassVar[str] = BuiltinAdapter.DATABRICKS.value
    sql_analysis_dialect_name: ClassVar[str | None] = "databricks"
    sql_lexical_syntax: ClassVar[SqlLexicalSyntax] = SqlLexicalSyntax(
        backslash_escape_quotes=frozenset({"'", '"'}),
        raw_string_prefix=True,
        nested_block_comments=True,
    )
    max_identifier_length: ClassVar[int] = 255
    concurrent_ddl_limit: ClassVar[int] = 16
    _snapshot_sql_dialect: ClassVar[SnapshotSqlDialect] = SnapshotSqlDialect(
        timestamp_type="TIMESTAMP",
        distinct_condition=render_is_distinct_from,
        update_style=SnapshotUpdateStyle.MERGE,
        latest_version=SnapshotLatestVersionStyle.QUALIFY,
        historical_close=HistoricalSnapshotCloseStyle.MERGE_HARD_DELETES,
        historical_insert=HistoricalSnapshotInsertStyle.WITH_INSERT,
    )

    def inspect_retention(self, *, connection: Any, request: RetentionRequest) -> RetentionState:
        target: str = self._retention_relation_target(request=request)
        cursor: Any = connection.cursor()
        try:
            cursor.execute(f"DESCRIBE DETAIL {target}")
            row: tuple[Any, ...] | None = cursor.fetchone()
            description: tuple[Any, ...] | None = cursor.description
        finally:
            cursor.close()
        if row is None or description is None:
            raise AdapterUserError(message="Databricks retention metadata is missing")
        detail: dict[str, object] = {
            str(column[0]).lower(): value for column, value in zip(description, row, strict=True)
        }
        relation_format: str = str(detail.get("format", "missing"))
        if relation_format.lower() != DELTA_RELATION_FORMAT:
            raise AdapterUserError(
                message=f"Databricks retention requires a Delta relation; found {relation_format}"
            )
        properties: dict[str, object] = self._delta_properties(detail.get("properties"))
        log_days: int = self._parse_delta_retention_days(
            value=properties.get(
                "delta.logRetentionDuration",
                f"interval {DELTA_DEFAULT_LOG_RETENTION_DAYS} days",
            ),
            property_name="log",
        )
        deleted_days: int = self._parse_delta_retention_days(
            value=properties.get(
                "delta.deletedFileRetentionDuration",
                f"interval {DELTA_DEFAULT_DELETED_FILE_RETENTION_DAYS} days",
            ),
            property_name="deleted file",
        )
        return RetentionState(
            request_id=request.request_id,
            scope=request.scope,
            configured_days=None,
            effective_days=min(log_days, deleted_days),
            relation_kind="DELTA",
            delta_log_retention_days=log_days,
            delta_deleted_file_retention_days=deleted_days,
        )

    def render_retention_changes(
        self, *, request: RetentionRequest, state: RetentionState | None = None
    ) -> tuple[RenderedRetentionChange, ...]:
        target: str = self._retention_relation_target(request=request)
        days: int = request.desired_days
        if state is not None:
            log_days: int = state.delta_log_retention_days or state.effective_days
            deleted_days: int = state.delta_deleted_file_retention_days or state.effective_days
            changes: list[RenderedRetentionChange] = []
            required_log_days: int = max(days, deleted_days)
            if log_days < required_log_days:
                changes.append(
                    self._render_delta_retention_property(
                        target=target,
                        property_name="log",
                        days=required_log_days,
                        increase=True,
                    )
                )
                log_days = required_log_days
            if deleted_days < days:
                changes.append(
                    self._render_delta_retention_property(
                        target=target, property_name="deletedFile", days=days, increase=True
                    )
                )
                deleted_days = days
            if deleted_days > days:
                changes.append(
                    self._render_delta_retention_property(
                        target=target, property_name="deletedFile", days=days, increase=False
                    )
                )
            if log_days > days:
                changes.append(
                    self._render_delta_retention_property(
                        target=target, property_name="log", days=days, increase=False
                    )
                )
            return tuple(changes)
        return (
            RenderedRetentionChange(
                phase=RetentionChangePhase.ALTER,
                statements=(
                    f"ALTER TABLE {target} SET TBLPROPERTIES "
                    f"('delta.logRetentionDuration' = 'interval {days} days', "
                    f"'delta.deletedFileRetentionDuration' = 'interval {days} days')",
                ),
            ),
        )

    @staticmethod
    def _render_delta_retention_property(
        *, target: str, property_name: str, days: int, increase: bool
    ) -> RenderedRetentionChange:
        return RenderedRetentionChange(
            phase=(RetentionChangePhase.PREPARE if increase else RetentionChangePhase.FINALIZE),
            statements=(
                f"ALTER TABLE {target} SET TBLPROPERTIES "
                f"('delta.{property_name}RetentionDuration' = 'interval {days} days')",
            ),
        )

    def _retention_relation_target(self, *, request: RetentionRequest) -> str:
        if request.scope != RetentionScope.RELATION or request.name is None:
            raise AdapterUserError(message="Databricks retention requires relation scope")
        if request.desired_days < 0:
            raise AdapterUserError(message="Databricks retention days must be non-negative")
        target: str | None = self.render_qualified_name(
            database=request.database,
            schema=request.schema,
            name=request.name,
        )
        if target is None:
            raise AdapterUserError(
                message="Databricks retention requires catalog, schema, and relation"
            )
        return target

    @staticmethod
    def _parse_delta_retention_days(*, value: object, property_name: str) -> int:
        match: re.Match[str] | None = re.fullmatch(
            r"\s*interval\s+(\d+)\s+(hours?|days?|weeks?)\s*",
            str(value),
            flags=re.IGNORECASE,
        )
        if match is None:
            raise AdapterUserError(
                message=f"Databricks Delta {property_name} retention is missing or unparseable"
            )
        amount: int = int(match.group(1))
        unit: str = match.group(2).lower()
        if unit.startswith("week"):
            return amount * 7
        if unit.startswith("hour"):
            if amount % 24 != 0:
                raise AdapterUserError(
                    message=(
                        f"Databricks Delta {property_name} retention is not a whole-day interval: "
                        f"{value!r}"
                    )
                )
            return amount // 24
        return amount

    @staticmethod
    def _delta_properties(value: object) -> dict[str, object]:
        if isinstance(value, dict):
            return {str(key): item for key, item in value.items()}
        if isinstance(value, str):
            try:
                parsed: object = json.loads(value)
            except json.JSONDecodeError:
                parsed = ast.literal_eval(value)
            if isinstance(parsed, dict):
                return {str(key): item for key, item in parsed.items()}
        raise AdapterUserError(message="Databricks Delta retention properties are missing")

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

    def supports_table_freshness_metadata(self) -> bool:
        return True

    def probe_relation_read(self, *, connection: Any, relation: str) -> RelationReadProbe:
        return run_relation_read_probe(
            execute=self.execute,
            connection=connection,
            relation=relation,
            classify_not_found=self._classify_relation_not_found,
        )

    @staticmethod
    def _classify_relation_not_found(error: BaseException) -> RelationReadStatus | None:
        message: str = str(error)
        not_found: bool = (
            any(error_class in message for error_class in RELATION_NOT_FOUND_ERROR_CLASSES)
            or getattr(error, "sqlstate", None) == UNDEFINED_TABLE_SQLSTATE
        )
        return RelationReadStatus.MISSING if not_found else None

    def get_table_freshness_metadata(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> TableFreshnessMetadata:
        if database is None or schema is None:
            raise AdapterUserError(
                message="Databricks table freshness metadata requires catalog and schema"
            )
        request: TableFreshnessRequest = TableFreshnessRequest(
            database=database,
            schema=schema,
            name=name,
        )
        metadata: TableFreshnessMetadata = self.get_tables_freshness_metadata(
            connection=connection, requests=(request,)
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
        """Read candidate reads from Unity Catalog's system.access.table_lineage."""

        return read_relation_usage(
            connection=connection, relations=relations, window_seconds=window_seconds
        )

    def get_tables_freshness_metadata(
        self,
        *,
        connection: Any,
        requests: tuple[TableFreshnessRequest, ...],
    ) -> dict[TableFreshnessRequest, TableFreshnessMetadata]:
        if not requests:
            return {}
        results: dict[TableFreshnessRequest, TableFreshnessMetadata] = {
            request: TableFreshnessMetadata.unavailable(
                message="Databricks table freshness metadata requires catalog and schema"
            )
            for request in requests
            if request.database is None or request.schema is None
        }
        lookup_requests: tuple[TableFreshnessRequest, ...] = tuple(
            request for request in requests if request not in results
        )
        if not lookup_requests:
            return results
        try:
            results.update(
                self._get_delta_history_freshness_metadata(
                    connection=connection,
                    requests=lookup_requests,
                )
            )
        except Exception:
            results.update(
                self._get_unity_catalog_freshness_metadata(
                    connection=connection,
                    requests=lookup_requests,
                )
            )
        return results

    def _get_unity_catalog_freshness_metadata(
        self,
        *,
        connection: Any,
        requests: tuple[TableFreshnessRequest, ...],
    ) -> dict[TableFreshnessRequest, TableFreshnessMetadata]:
        clauses: str = " OR ".join(
            "(table_catalog = "
            + self._string_literal(str(request.database).lower())
            + " AND table_schema = "
            + self._string_literal(str(request.schema).lower())
            + " AND table_name = "
            + self._string_literal(request.name.lower())
            + ")"
            for request in requests
        )
        query: str = (
            "SELECT table_catalog, table_schema, table_name, table_type, last_altered "
            "FROM `system`.`information_schema`.`tables` WHERE "
            f"{clauses}"
        )
        cursor: Any = connection.cursor()
        try:
            cursor.execute(query)
            rows: list[tuple[Any, ...]] = list(cursor.fetchall())
        finally:
            cursor.close()

        requests_by_key: dict[tuple[str, str, str], TableFreshnessRequest] = {
            (
                str(request.database).lower(),
                str(request.schema).lower(),
                request.name.lower(),
            ): request
            for request in requests
        }
        results: dict[TableFreshnessRequest, TableFreshnessMetadata] = {}
        row: tuple[Any, ...]
        for row in rows:
            matched_request: TableFreshnessRequest | None = requests_by_key.get(
                (str(row[0]).lower(), str(row[1]).lower(), str(row[2]).lower())
            )
            if matched_request is None:
                continue
            table_type: str = str(row[3])
            if self._normalize_relation_type(table_type) != RelationType.TABLE:
                results[matched_request] = TableFreshnessMetadata.unavailable(
                    message="Databricks table freshness metadata only supports Delta tables; "
                    f"found {table_type} for {matched_request.name}"
                )
                continue
            if row[4] is None:
                results[matched_request] = TableFreshnessMetadata.unavailable(
                    message="Databricks table freshness metadata is missing LAST_ALTERED "
                    f"for {matched_request.name}"
                )
                continue
            results[matched_request] = TableFreshnessMetadata(
                data_version=row[4],
                value_kind="timestamp",
                observed_at=row[4] if isinstance(row[4], datetime) else None,
            )
        return complete_table_freshness_results(
            requests=requests, results=results, adapter_label="Databricks"
        )

    def _get_delta_history_freshness_metadata(
        self,
        *,
        connection: Any,
        requests: tuple[TableFreshnessRequest, ...],
    ) -> dict[TableFreshnessRequest, TableFreshnessMetadata]:
        selects: list[str] = []
        request: TableFreshnessRequest
        for request in requests:
            target: str | None = self.render_qualified_name(
                database=request.database,
                schema=request.schema,
                name=request.name,
            )
            if target is None:
                raise AdapterUserError(
                    message="Databricks table freshness metadata requires catalog and schema"
                )
            selects.append(
                "SELECT "
                + self._string_literal(str(request.database))
                + " AS catalog, "
                + self._string_literal(str(request.schema))
                + " AS schema, "
                + self._string_literal(request.name)
                + " AS identifier, max(timestamp) AS last_modified "
                f"FROM (DESCRIBE HISTORY {target})"
            )
        cursor: Any = connection.cursor()
        try:
            cursor.execute(" UNION ALL ".join(selects))
            rows: list[tuple[Any, ...]] = list(cursor.fetchall())
        except Exception as error:
            raise AdapterUserError(
                message="Databricks table freshness metadata requires Unity Catalog table metadata "
                "or Delta history"
            ) from error
        finally:
            cursor.close()

        requests_by_key: dict[tuple[str, str, str], TableFreshnessRequest] = {
            (str(request.database), str(request.schema), request.name): request
            for request in requests
        }
        results: dict[TableFreshnessRequest, TableFreshnessMetadata] = {}
        row: tuple[Any, ...]
        for row in rows:
            matched_request: TableFreshnessRequest | None = requests_by_key.get(
                (str(row[0]), str(row[1]), str(row[2]))
            )
            if matched_request is None:
                continue
            if row[3] is None:
                raise AdapterUserError(
                    message=f"Databricks Delta history not found for {matched_request.name}"
                )
            results[matched_request] = TableFreshnessMetadata(
                data_version=row[3],
                value_kind="timestamp",
                observed_at=row[3] if isinstance(row[3], datetime) else None,
            )
        return complete_table_freshness_results(
            requests=requests, results=results, adapter_label="Databricks Delta history"
        )

    def maximum_identifier_length(self) -> int:
        """Return the maximum unqualified identifier length supported by the adapter."""

        return self.max_identifier_length

    def persists_python_functions(self) -> bool:
        return True

    def python_functions_inherit_default_namespace(self) -> bool:
        return True

    def supports_unqualified_function_fingerprints(self) -> bool:
        return False

    def render_drop_view(self, *, destination: str, if_exists: bool = True) -> tuple[str, ...]:
        exists_clause: str = " IF EXISTS" if if_exists else ""
        return (f"DROP VIEW{exists_clause} {destination}",)

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

    def render_current_timestamp(self) -> str:
        return "CURRENT_TIMESTAMP"

    def render_current_utc_timestamp(self) -> str:
        return "current_timestamp()"

    def sql_analysis_dialect(self) -> str | None:
        """Return the SQL analysis dialect name for this adapter, if any."""

        return self.sql_analysis_dialect_name

    def render_identifier(self, name: str) -> str:
        return "`" + name.replace("`", "``") + "`"

    def render_exact_identifier(self, name: str) -> str:
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
                return "STRING"
            case LoaderLogicalType.TIMESTAMP:
                return "TIMESTAMP"
            case LoaderLogicalType.DATE:
                return "DATE"
            case LoaderLogicalType.JSON:
                return "STRING"

    def render_typed_scalar(self, *, value: SqlValue) -> str:
        return _render_ansi_typed_scalar(value=value)

    def render_typed_value_list(self, *, value: SqlValue) -> str:
        return _render_typed_value_list(value=value, render_scalar=self.render_typed_scalar)

    def render_typed_array(self, *, value: SqlValue) -> str:
        return "array(" + self._render_typed_array_items(value) + ")"

    def render_typed_object(self, *, value: SqlValue) -> str:
        return f"parse_json({self._quote_sql_string(_encode_typed_json(value))})"

    def render_loader_value_literal(
        self, *, value: object, logical_type: LoaderLogicalType | None
    ) -> str:
        del logical_type
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
                f"{self._loader_row_sql_type(column_sql_types.get(column_name))}) AS "
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
            (
                self.render_identifier(column_name)
                if column_name not in column_sql_types
                else "CAST("
                f"{self.render_identifier(column_name)} AS "
                f"{self._loader_row_sql_type(column_sql_types[column_name])}) "
                f"AS {self.render_identifier(column_name)}"
            )
            for column_name in column_names
        )
        return f"SELECT {select_sql} FROM (VALUES {values_sql}) AS __loader_rows({column_sql})"

    def _loader_row_sql_type(self, column_type: str | None) -> str:
        if column_type is None:
            return "STRING"
        return self._to_databricks_type(column_type)

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

    def interrupt_connection(self, connection: Any) -> bool:
        """Request cancellation of the connection's running statements."""

        connection.cancel_active_statements()
        return True

    def lists_relations_authoritatively(self, *, database: str | None, schema: str) -> bool:
        """Return whether ``list_relations`` reports every relation in this scope."""

        del schema
        return database is not None

    def supports_zero_copy_clone(self) -> bool:
        return True

    def supports_durable_clone(self) -> bool:
        return True

    def supports_relation_age_metadata(self) -> bool:
        return True

    def max_statement_size(self) -> StatementSizeLimit | None:
        """16 MiB query text cap: https://docs.databricks.com/api/statement-execution/v1/execute-statement."""

        return (16 * 1_048_576, "bytes")

    def connect(self, config: dict[str, Any]) -> _DatabricksConnection:
        """Open a Databricks SQL connection from the resolved config."""

        server_hostname: object | None = config.get("server_hostname")
        http_path: object | None = config.get("http_path")
        token: object | None = config.get("token")
        catalog: object | None = config.get("catalog")
        if not isinstance(server_hostname, str) or not server_hostname.strip():
            raise AdapterUserError(
                message="Databricks connection requires non-empty 'server_hostname'",
                code="A201",
            )
        if not isinstance(http_path, str) or not http_path.strip():
            raise AdapterUserError(
                message="Databricks connection requires non-empty 'http_path'",
                code="A202",
            )
        if not isinstance(token, str) or not token.strip():
            raise AdapterUserError(
                message="Databricks connection requires non-empty 'token'", code="A203"
            )
        if not isinstance(catalog, str) or not catalog.strip():
            raise AdapterUserError(
                message="Databricks connection requires non-empty 'catalog'",
                code="A204",
            )

        try:
            from databricks import sql
        except ImportError as error:
            raise AdapterUserError(
                message="Databricks adapter requires optional dependency databricks-sql-connector. "
                "Install with: sqlbuild[databricks]",
                code="A205",
            ) from error

        raw_connection: Any = sql.connect(
            server_hostname=server_hostname.strip(),
            http_path=http_path.strip(),
            access_token=token.strip(),
        )
        connection: _DatabricksConnection = _DatabricksConnection(raw_connection)
        self._initialize_session(
            connection=connection,
            catalog=catalog,
            schema=config.get("schema"),
        )
        return connection

    def _execute(self, *, connection: _DatabricksConnection, sql: str) -> Any:
        """Execute a SQL statement against a Databricks connection."""

        log_sql(logger=logging.getLogger("sqlbuild.adapter.databricks"), sql=sql)
        return connection.execute(sql)

    def query(self, *, connection: Any, sql: str, limit: int | None) -> QueryResult:
        """Execute SQL and return normalized rows for ad hoc query output."""

        cursor: Any = self.execute(connection=connection, sql=sql)
        try:
            description: Any | None = getattr(cursor, "description", None)
            if description is None:
                return QueryResult()
            columns: tuple[str, ...] = tuple(str(column[0]) for column in description)
            if limit is None:
                rows: tuple[tuple[object, ...], ...] = tuple(
                    tuple(row) for row in cursor.fetchall()
                )
                if self._is_status_only_non_row_result(columns=columns, rows=rows):
                    return QueryResult()
                return QueryResult(columns=columns, rows=rows)
            fetched_rows: list[tuple[object, ...]] = [
                tuple(row) for row in cursor.fetchmany(limit + 1)
            ]
            rows = tuple(fetched_rows[:limit])
            if self._is_status_only_non_row_result(columns=columns, rows=rows):
                return QueryResult()
            return QueryResult(
                columns=columns,
                rows=rows,
                truncated=len(fetched_rows) > limit,
            )
        finally:
            cursor.close()

    @staticmethod
    def _is_status_only_non_row_result(
        *, columns: tuple[str, ...], rows: tuple[tuple[object, ...], ...]
    ) -> bool:
        if len(columns) != 1 or columns[0].lower() not in NON_ROW_RESULT_COLUMN_NAMES:
            return False
        if not rows:
            return True
        if len(rows) != 1:
            return False
        status_value: object = rows[0][0]
        return isinstance(status_value, str)

    def relation_exists(
        self,
        *,
        connection: _DatabricksConnection,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> bool:
        if database is None or schema is None:
            return False
        query: str = (
            f"SELECT 1 FROM {self._information_schema(database)}.tables "
            f"WHERE table_schema = {self._string_literal(schema)} "
            f"AND table_name = {self._string_literal(name)}"
        )
        cursor: Any = connection.cursor()
        try:
            cursor.execute(query)
            return cursor.fetchone() is not None
        finally:
            cursor.close()

    def list_relations(
        self,
        *,
        connection: _DatabricksConnection,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[RelationInfo, ...]:
        if database is None or not schemas:
            return ()
        information_schema: str = self._information_schema(database)
        query: str = (
            "SELECT table_name, table_schema, table_type, created, last_altered "
            f"FROM {information_schema}.tables WHERE 1=1"
        )
        query += self._build_in_filter(column="table_schema", values=schemas)
        if names:
            query += self._build_in_filter(column="table_name", values=names)
        cursor: Any = connection.cursor()
        try:
            cursor.execute(query)
            rows: list[tuple[Any, ...]] = cursor.fetchall()
        finally:
            cursor.close()
        return tuple(self._listed_relation(database=database, row=row) for row in rows)

    def _listed_relation(self, *, database: str, row: tuple[Any, ...]) -> RelationInfo:
        relation_type: str = self._normalize_relation_type(str(row[2]))
        is_view: bool = relation_type == RelationType.VIEW
        return RelationInfo(
            database=database,
            schema=str(row[1]).lower(),
            name=str(row[0]).lower(),
            relation_type=relation_type,
            created_at=relation_age_timestamp_utc(row[3]) if is_view else None,
            last_altered_at=relation_age_timestamp_utc(row[4]) if is_view else None,
        )

    def with_relation_age_metadata(
        self,
        *,
        connection: Any,
        relations: tuple[RelationInfo, ...],
    ) -> tuple[RelationInfo, ...]:
        """Read data-write ages for non-view relations; information_schema tracks only DDL."""

        return tuple(
            relation
            if relation.relation_type == RelationType.VIEW
            else self._with_delta_detail_ages(connection=connection, relation=relation)
            for relation in relations
        )

    def _with_delta_detail_ages(self, *, connection: Any, relation: RelationInfo) -> RelationInfo:
        unknown_age: RelationInfo = replace(relation, created_at=None, last_altered_at=None)
        if relation.database is None or relation.schema is None:
            return unknown_age
        target: str = ".".join(
            self.render_identifier(part)
            for part in (relation.database, relation.schema, relation.name)
        )
        cursor: Any = connection.cursor()
        try:
            cursor.execute(f"DESCRIBE DETAIL {target}")
            row: tuple[Any, ...] | None = cursor.fetchone()
            description: tuple[Any, ...] | None = cursor.description
        except Exception:
            return unknown_age
        finally:
            cursor.close()
        if row is None or description is None:
            return unknown_age
        detail: dict[str, object] = {
            str(column[0]).lower(): value for column, value in zip(description, row, strict=False)
        }
        last_modified: datetime | None = relation_age_timestamp_utc(detail.get("lastmodified"))
        if last_modified is None:
            return unknown_age
        return replace(
            relation,
            created_at=relation_age_timestamp_utc(detail.get("createdat")),
            last_altered_at=last_modified,
        )

    def list_functions(
        self,
        *,
        connection: _DatabricksConnection,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[FunctionInfo, ...]:
        if database is None or not schemas:
            return ()
        information_schema: str = self._information_schema(database)
        query: str = (
            f"SELECT routine_name, routine_schema, routine_type FROM {information_schema}.routines "
            "WHERE 1=1"
        )
        query += self._build_in_filter(column="routine_schema", values=schemas)
        if names:
            query += self._build_in_filter(column="routine_name", values=names)
        cursor: Any = connection.cursor()
        try:
            cursor.execute(query)
            rows: list[tuple[Any, ...]] = cursor.fetchall()
        finally:
            cursor.close()
        return tuple(
            FunctionInfo(
                database=database,
                schema=str(row[1]).lower(),
                name=str(row[0]).lower(),
                function_type=str(row[2]).lower(),
            )
            for row in rows
        )

    def get_columns(
        self,
        *,
        connection: _DatabricksConnection,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> tuple[ColumnInfo, ...]:
        if database is None or schema is None:
            return ()
        query: str = (
            "SELECT column_name, full_data_type "
            f"FROM {self._information_schema(database)}.columns "
            f"WHERE table_schema = {self._string_literal(schema)} "
            f"AND table_name = {self._string_literal(name)} "
            "ORDER BY ordinal_position"
        )
        cursor: Any = connection.cursor()
        try:
            cursor.execute(query)
            rows: list[tuple[Any, ...]] = cursor.fetchall()
        finally:
            cursor.close()
        return tuple(ColumnInfo(name=str(row[0]).lower(), type=str(row[1]).upper()) for row in rows)

    def get_all_columns(
        self,
        *,
        connection: _DatabricksConnection,
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

    def close(self, connection: _DatabricksConnection) -> None:
        """Close a Databricks connection."""

        connection.close()

    def supports_transactions(self) -> bool:
        return False

    def star_exclude_keyword(self) -> str:
        """Databricks uses EXCEPT for SELECT * EXCEPT."""

        return "EXCEPT"

    def default_schema(self) -> str | None:
        return None

    def default_database(self) -> str | None:
        return None

    def default_table_promotion_mode(self) -> TablePromotionMode:
        return TablePromotionMode.STAGED

    def render_qualified_name(
        self,
        *,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> str | None:
        if database is not None and schema is not None:
            return f"`{database}`.`{schema}`.`{name}`"
        if schema is not None:
            return f"`{schema}`.`{name}`"
        return None

    def render_framework_type(self, type_name: FrameworkType) -> str:
        match type_name:
            case FrameworkType.STRING:
                return "STRING"
            case FrameworkType.INTEGER:
                return "BIGINT"
            case FrameworkType.TIMESTAMP:
                return "TIMESTAMP"

    def render_source_expression_cast(
        self, *, expression: str, target_type: str, alias: str
    ) -> str:
        return f"CAST({expression} AS {self._to_databricks_type(target_type)}) AS {alias}"

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
        """Databricks does not require aliases for derived table factors."""

        return False

    def render_set_difference_operator(self) -> str:
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

    def expression_inference_profile(self) -> ExpressionInferenceProfile:
        return ExpressionInferenceProfile(
            sql_analysis_dialect=self.sql_analysis_dialect(),
            function_nullability_rules={
                "IF": conditional_result_nullability,
                "LOWER": first_arg_nullability,
                "UPPER": first_arg_nullability,
            },
        )

    def default_promotion_strategy(self) -> PromotionStrategy:
        return PromotionStrategy.ATOMIC_REPLACE

    def render_create_schema(self, *, database: str | None, schema: str) -> tuple[str, ...]:
        target: str = f"{database}.{schema}" if database is not None else schema
        return (f"CREATE SCHEMA IF NOT EXISTS {target}",)

    def render_drop_schema(self, *, database: str | None, schema: str) -> tuple[str, ...]:
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
        statements: tuple[str, ...] = self.render_create_schema(database=database, schema=schema)
        statement_recorder.record_many(statements)
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

    def render_create_table_as(self, *, destination: str, sql: str) -> tuple[str, ...]:
        return (f"CREATE OR REPLACE TABLE {destination} AS {sql}",)

    def render_create_view_as(self, *, destination: str, sql: str) -> tuple[str, ...]:
        return (f"CREATE OR REPLACE VIEW {destination} AS {sql}",)

    def supports_table_functions(self) -> bool:
        return True

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
        if language == FunctionLanguage.PYTHON:
            if return_columns:
                raise AdapterUserError(message="Databricks table functions must use SQL language")
            del runtime_version
            return self._render_create_python_function(
                destination=destination,
                arguments=arguments,
                returns=returns,
                body_sql=body_sql,
                entry_point=entry_point,
                packages=packages,
            )
        del runtime_version, entry_point, packages
        argument_sql: str = ", ".join(f"{argument.name} {argument.type}" for argument in arguments)
        if return_columns:
            del returns
            return (
                f"CREATE OR REPLACE FUNCTION {destination}({argument_sql})\n"
                "RETURNS TABLE\n"
                f"RETURN {body_sql}",
            )
        return (
            f"CREATE OR REPLACE FUNCTION {destination}({argument_sql})\n"
            f"RETURNS {returns}\n"
            f"RETURN {body_sql}",
        )

    def render_create_temporary_function(
        self, *, name: str, definition: FunctionDefinition
    ) -> TemporaryFunction | None:
        """Copy a SQL function into a prefixed session TEMPORARY function, called unqualified."""

        if definition.language != FunctionLanguage.SQL:
            return None
        target: str = self.render_identifier(f"{TEMPORARY_FUNCTION_PREFIX}{name}")
        statements: tuple[str, ...] = self.render_create_function(
            destination=target,
            arguments=definition.arguments,
            returns=definition.returns,
            body_sql=definition.body_sql,
            return_columns=definition.return_columns,
        )
        return TemporaryFunction(
            target=target,
            statements=tuple(
                statement.replace(
                    "CREATE OR REPLACE FUNCTION ", "CREATE OR REPLACE TEMPORARY FUNCTION ", 1
                )
                for statement in statements
            ),
        )

    def _render_create_python_function(
        self,
        *,
        destination: str,
        arguments: tuple[Any, ...],
        returns: str,
        body_sql: str,
        entry_point: str | None,
        packages: tuple[str, ...],
    ) -> tuple[str, ...]:
        argument_sql: str = ", ".join(f"{argument.name} {argument.type}" for argument in arguments)
        environment_sql: str = self._render_python_environment(packages=packages)
        body: str = self._databricks_python_body(body_sql=body_sql, entry_point=entry_point)
        return (
            f"CREATE OR REPLACE FUNCTION {destination}({argument_sql})\n"
            f"RETURNS {returns}\n"
            "LANGUAGE PYTHON\n"
            f"{environment_sql}"
            "AS $$\n"
            f"{body}\n"
            "$$",
        )

    @staticmethod
    def _render_python_environment(*, packages: tuple[str, ...]) -> str:
        if not packages:
            return ""
        dependencies: str = json.dumps(list(packages))
        return (
            "ENVIRONMENT (\n"
            f"  dependencies = '{dependencies}',\n"
            "  environment_version = 'None'\n"
            ")\n"
        )

    @staticmethod
    def _databricks_python_body(*, body_sql: str, entry_point: str | None) -> str:
        if entry_point is None:
            return body_sql.strip()
        try:
            module: ast.Module = ast.parse(body_sql)
        except SyntaxError:
            return body_sql.strip()
        body: list[ast.stmt] = []
        found_entry_point: bool = False
        node: ast.stmt
        for node in module.body:
            if (
                isinstance(node, ast.FunctionDef | ast.AsyncFunctionDef)
                and node.name == entry_point
            ):
                body.extend(node.body)
                found_entry_point = True
                continue
            body.append(node)
        if not found_entry_point:
            return body_sql.strip()
        return ast.unparse(ast.Module(body=body, type_ignores=[])).strip()

    def supports_python_functions(self) -> bool:
        return True

    def render_append(
        self, *, destination: str, sql: str, columns: tuple[str, ...] | None = None
    ) -> tuple[str, ...]:
        if columns is not None:
            return (
                f"INSERT INTO {destination} "
                f"({', '.join(self.render_identifier(column) for column in columns)}) {sql}",
            )
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
            f"{destination}.{self.render_identifier(key)} = __source.{self.render_identifier(key)}"
            for key in unique_key
        )
        delete_sql: str = (
            f"DELETE FROM {destination} WHERE EXISTS "
            f"(SELECT 1 FROM ({sql}) AS __source WHERE {key_condition})"
        )
        insert_statements: tuple[str, ...] = self.render_append(
            destination=destination,
            sql=sql,
            columns=columns,
        )
        return (delete_sql, *insert_statements)

    def render_cursor_bound_literal(self, *, value: str, cursor_type: str | None) -> str:
        if cursor_type == CursorKind.INTEGER:
            return value
        if cursor_type == CursorKind.TIMESTAMP:
            return f"TIMESTAMP '{value}'"
        return f"'{value}'"

    def render_drop(self, *, destination: str, if_exists: bool = True) -> tuple[str, ...]:
        exists_clause: str = " IF EXISTS" if if_exists else ""
        return (
            f"DROP TABLE{exists_clause} {destination}",
            f"DROP VIEW{exists_clause} {destination}",
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
    ) -> tuple[str, ...]:
        column_list: str = ""
        if columns is not None:
            column_list = f" ({', '.join(self.render_identifier(column) for column in columns)})"
        quoted_cursor_column: str = self.render_identifier(cursor_column)
        start_bound: str = self._render_cursor_bound_string(cursor_start)
        end_bound: str = self._render_cursor_bound_string(cursor_end)
        replace_where: str = (
            f"{quoted_cursor_column} >= {start_bound} AND {quoted_cursor_column} < {end_bound}"
        )
        return (f"INSERT INTO {destination}{column_list} REPLACE WHERE {replace_where} {sql}",)

    def render_rename(self, *, origin: str, destination: str) -> tuple[str, ...]:
        return (f"ALTER TABLE {origin} RENAME TO {destination}",)

    def render_rename_view(self, *, origin: str, destination: str) -> tuple[str, ...]:
        return (f"ALTER VIEW {origin} RENAME TO {destination}",)

    def render_swap(self, *, left: str, right: str) -> tuple[str, ...]:
        raise AdapterUserError(message="Databricks does not support atomic table swap")

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
            return (f"CREATE TABLE {destination} SHALLOW CLONE {origin}",)
        return self.render_create_table_as(destination=destination, sql=f"SELECT * FROM {origin}")

    def render_durable_clone(
        self, *, origin: str, destination: str, origin_is_transient: bool = False
    ) -> tuple[str, ...]:
        del origin_is_transient
        return (f"CREATE TABLE {destination} DEEP CLONE {origin}",)

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
            statements=(f"CREATE TABLE {stage} DEEP CLONE {origin}",),
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
        relation: str | None = self.render_qualified_name(
            database=database, schema=schema, name=name
        )
        kind: str = show_grants_object_kind(relation_type)
        rows: list[tuple[Any, ...]] = self.execute(
            connection=connection, sql=f"SHOW GRANTS ON {kind} {relation}"
        ).fetchall()
        return databricks_relation_grants(rows)

    def render_relation_grants(
        self, *, grants: tuple[RelationGrant, ...], destination: str, columns: tuple[str, ...]
    ) -> tuple[str, ...]:
        del columns
        return render_databricks_view_grants(grants=grants, destination=destination)

    def render_replace_view_keeping_grants(
        self, *, destination: str, sql: str
    ) -> tuple[str, ...] | None:
        return (f"ALTER VIEW {destination} AS {sql}",)

    def read_view_definition(
        self, *, connection: Any, database: str | None, schema: str, name: str
    ) -> str | None:
        views: str = (
            "information_schema.views"
            if database is None
            else (f"{database}.information_schema.views")
        )
        rows: list[tuple[Any, ...]] = self.execute(
            connection=connection,
            sql=(
                f"SELECT view_definition FROM {views} WHERE lower(table_schema) = lower("
                + "'"
                + schema.replace("'", "''")
                + "'"
                + ") AND lower(table_name) = lower("
                + "'"
                + name.replace("'", "''")
                + "'"
                + ")"
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
        return render_databricks_view_revokes(grants=grants, destination=destination)

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

    def render_replace_table_from_relation(
        self, *, destination: str, origin: str
    ) -> tuple[str, ...]:
        return (f"CREATE OR REPLACE TABLE {destination} AS SELECT * FROM {origin}",)

    def render_add_columns(
        self,
        *,
        destination: str,
        columns: tuple[ColumnInfo, ...],
    ) -> tuple[str, ...]:
        return tuple(
            f"ALTER TABLE {destination} ADD COLUMN "
            f"{self.render_identifier(column.name)} {column.type}"
            for column in columns
        )

    def render_drop_columns(
        self,
        *,
        destination: str,
        column_names: tuple[str, ...],
    ) -> tuple[str, ...]:
        return tuple(
            f"ALTER TABLE {destination} DROP COLUMN {self.render_identifier(column_name)}"
            for column_name in column_names
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
        self,
        *,
        destination: str,
        columns: tuple[ColumnInfo, ...],
    ) -> tuple[str, ...]:
        return tuple(
            "ALTER TABLE "
            f"{destination} ALTER COLUMN {self.render_identifier(column.name)} TYPE {column.type}"
            for column in columns
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
            f"__target.{self.render_identifier(key)} = __source.{self.render_identifier(key)}"
            for key in unique_key
        )
        update_assignments: str = ", ".join(
            f"{self.render_identifier(column)} = __source.{self.render_identifier(column)}"
            for column in source_columns
            if column.lower() not in immutable_columns
        )
        insert_columns: str = ", ".join(self.render_identifier(column) for column in source_columns)
        insert_values: str = ", ".join(
            f"__source.{self.render_identifier(column)}" for column in source_columns
        )
        merge_sql: str = (
            f"MERGE INTO {destination} AS __target USING ({sql}) AS __source ON {join_condition} "
        )
        if update_assignments:
            merge_sql += f"WHEN MATCHED THEN UPDATE SET {update_assignments} "
        merge_sql += f"WHEN NOT MATCHED THEN INSERT ({insert_columns}) VALUES ({insert_values})"
        return (merge_sql,)

    def create_table_as(
        self,
        *,
        connection: Any,
        destination: str,
        sql: str,
        config: dict[str, Any] | None = None,
        statement_recorder: StatementRecorder,
    ) -> None:
        del config
        statements: tuple[str, ...] = self.render_create_table_as(destination=destination, sql=sql)
        statement_recorder.record_many(statements)
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

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
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

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
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

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
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

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
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

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
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

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
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

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
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

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
            raise AdapterUserError(message="Databricks relation move/copy requires --allow-copy")
        statements: tuple[str, ...] = self.render_replace_table_from_relation(
            destination=destination,
            origin=origin,
        )
        if remove_origin:
            statements = (*statements, *self.render_drop(destination=origin))
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
    ) -> None:
        statements: tuple[str, ...] = self.render_append(
            destination=destination, sql=sql, columns=columns
        )
        statement_recorder.record_many(statements)
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

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
            destination=destination,
            sql=sql,
            unique_key=keys,
            columns=columns,
        )
        statement_recorder.record_many(statements)
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

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
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

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
        column_defs: str = ", ".join(
            f"{col.name} {self._to_databricks_type(col.type)}" for col in columns
        )
        create_sql: str = f"CREATE TABLE {destination} ({column_defs})"
        statement_recorder.record(create_sql)
        self.execute(connection=connection, sql=create_sql)

        column_names: tuple[str, ...] = tuple(column.name for column in columns)
        placeholders: str = ", ".join(["?"] * len(column_names))
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
                skipinitialspace=False
                if csv_settings.skipinitialspace is None
                else csv_settings.skipinitialspace,
            )
            row: dict[str, str] | None
            for row in reader:
                if row is None:
                    continue
                rows.append(
                    tuple(
                        normalize_seed_csv_value(
                            value=row.get(column_name),
                            column_name=column_name,
                            csv_settings=csv_settings,
                        )
                        for column_name in column_names
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

    def query_column_names(self, *, connection: Any, sql: str) -> tuple[str, ...]:
        cursor: Any = connection.cursor()
        try:
            cursor.execute(f"SELECT * FROM ({sql}) AS __describe_source LIMIT 0")
            description: tuple[Any, ...] | None = cursor.description
            if description is None:
                return ()
            return tuple(str(column[0]) for column in description)
        finally:
            cursor.close()

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

    def rename_column(
        self,
        *,
        connection: Any,
        destination: str,
        old_name: str,
        new_name: str,
        statement_recorder: StatementRecorder,
    ) -> None:
        reason: str | None = self.column_rename_unavailable_reason(
            connection=connection, destination=destination
        )
        if reason is not None:
            raise AdapterUserError(message=reason)
        statements: tuple[str, ...] = self.render_rename_column(
            destination=destination, old_name=old_name, new_name=new_name
        )
        statement_recorder.record_many(statements)
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

    def column_rename_unavailable_reason(self, *, connection: Any, destination: str) -> str | None:
        rows: list[tuple[Any, ...]] = self.execute(
            connection=connection,
            sql=f"SHOW TBLPROPERTIES {destination} ('{DELTA_COLUMN_MAPPING_PROPERTY}')",
        ).fetchall()
        mode: str | None = next(
            (
                str(row[1]).strip().lower()
                for row in rows
                if len(row) > 1 and str(row[0]) == DELTA_COLUMN_MAPPING_PROPERTY
            ),
            None,
        )
        if mode == DELTA_COLUMN_MAPPING_NAME_MODE:
            return None
        return (
            f"Databricks renames a column in place only when the Delta table uses column "
            f"mapping ('{DELTA_COLUMN_MAPPING_PROPERTY}' = '{DELTA_COLUMN_MAPPING_NAME_MODE}'), "
            f"and {destination} has {'no column mapping' if mode is None else repr(mode)}. "
            f"Enable it with ALTER TABLE {destination} SET TBLPROPERTIES "
            f"('{DELTA_COLUMN_MAPPING_PROPERTY}' = '{DELTA_COLUMN_MAPPING_NAME_MODE}', "
            "'delta.minReaderVersion' = '2', 'delta.minWriterVersion' = '5'), which upgrades "
            "the table protocol, or full-refresh the model"
        )

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
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

    def estimate_relation_row_count(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str | None,
        name: str,
        metadata_only: bool = False,
    ) -> RelationRowCountEstimate:
        """Read ``ANALYZE TABLE`` statistics; ``DESCRIBE DETAIL`` has no row count."""

        target: str = ".".join(
            self.render_identifier(part) for part in (database, schema, name) if part is not None
        )
        cursor: Any = connection.cursor()
        try:
            cursor.execute(f"DESCRIBE TABLE EXTENDED {target}")
            rows: list[tuple[Any, ...]] = cursor.fetchall()
        finally:
            cursor.close()
        labels: list[str] = [str(row[0]).strip().lower() if row else "" for row in rows]
        detail_start: int = (
            labels.index(DETAILED_TABLE_INFORMATION_MARKER) + 1
            if DETAILED_TABLE_INFORMATION_MARKER in labels
            else 0
        )
        details: dict[str, str] = {
            str(row[0]).strip().lower(): str(row[1])
            for row in rows[detail_start:]
            if len(row) >= DESCRIBE_DETAIL_ROW_WIDTH and row[1] is not None
        }
        relation_type: str = details.get("type", "").strip().lower()
        if relation_type not in TABLE_RELATION_METADATA_TYPES:
            return RelationRowCountEstimate(
                row_count=None, detail=f"no row count ({relation_type or 'unknown type'})"
            )
        statistics_match: re.Match[str] | None = re.search(
            r"(\d+)\s+rows", details.get("statistics", "")
        )
        bytes_match: re.Match[str] | None = (
            re.search(r"(\d+)\s+bytes", details.get("statistics", "")) if metadata_only else None
        )
        return RelationRowCountEstimate(
            row_count=int(statistics_match.group(1)) if statistics_match is not None else None,
            detail=(
                "table statistics not collected; run ANALYZE TABLE ... COMPUTE STATISTICS"
                if statistics_match is None
                else None
            ),
            byte_count=int(bytes_match.group(1)) if bytes_match is not None else None,
            byte_count_basis="describe_table_statistics_bytes" if bytes_match is not None else None,
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
            raise AdapterUserError(message="Databricks row diff query returned no result")
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

    def describe_relation(self, *, connection: Any, relation: str) -> tuple[ColumnInfo, ...]:
        """Return Databricks relation metadata using DESCRIBE TABLE."""

        selectable_row_column_count: int = 2
        cursor: Any = connection.cursor()
        try:
            cursor.execute(f"DESCRIBE TABLE {relation}")
            rows: list[tuple[Any, ...]] = cursor.fetchall()
        finally:
            cursor.close()
        return tuple(
            ColumnInfo(name=str(row[0]).lower(), type=str(row[1]).upper())
            for row in rows
            if len(row) >= selectable_row_column_count
            and str(row[0]).strip()
            and not str(row[0]).startswith("#")
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
        clauses: list[str] = [
            f"{cursor_column} >= {self._render_cursor_value_literal(start_cursor)}"
        ]
        if end_cursor is not None:
            clauses.append(f"{cursor_column} < {self._render_cursor_value_literal(end_cursor)}")
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

    def resolve_row_diff_tolerance(
        self,
        *,
        column: str,
        column_type: str,
        tolerances: RowDiffTolerances | None,
    ) -> RowDiffTolerance | None:
        if tolerances is None:
            return None
        by_column: RowDiffTolerance | None = tolerances.by_column.get(column)
        if by_column is not None:
            self.validate_row_diff_tolerance(column=column, tolerance=by_column)
            return by_column
        normalized_type: str | None = self.normalize_row_diff_numeric_type(column_type)
        if normalized_type is None:
            return None
        by_type: RowDiffTolerance | None = tolerances.by_type.get(normalized_type)
        if by_type is not None:
            self.validate_row_diff_tolerance(column=column, tolerance=by_type)
            return by_type
        return None

    def validate_row_diff_tolerance(self, *, column: str, tolerance: RowDiffTolerance) -> None:
        if tolerance.absolute is None and tolerance.relative is None:
            raise AdapterUserError(
                message=f"row diff tolerance for column '{column}' must set absolute or relative"
            )
        if tolerance.absolute is not None and tolerance.absolute < Decimal("0"):
            raise AdapterUserError(
                message=f"row diff absolute tolerance for column '{column}' must be >= 0"
            )
        if tolerance.relative is not None and tolerance.relative < Decimal("0"):
            raise AdapterUserError(
                message=f"row diff relative tolerance for column '{column}' must be >= 0"
            )

    def schema_exists(
        self,
        *,
        connection: _DatabricksConnection,
        database: str | None,
        schema: str,
    ) -> bool:
        schemata: str = (
            CURRENT_CATALOG_INFORMATION_SCHEMA
            if database is None
            else self._information_schema(database)
        )
        query: str = (
            f"SELECT 1 FROM {schemata}.schemata WHERE schema_name = {self._string_literal(schema)}"
        )
        cursor: Any = connection.cursor()
        try:
            cursor.execute(query)
            return cursor.fetchone() is not None
        finally:
            cursor.close()

    def _initialize_session(
        self,
        *,
        connection: _DatabricksConnection,
        catalog: object | None,
        schema: object | None,
    ) -> None:
        statements: list[str] = []
        normalized_catalog: str | None = self._normalize_session_value(catalog)
        normalized_schema: str | None = self._normalize_session_value(schema)
        if normalized_catalog is not None:
            statements.append(f"USE CATALOG `{normalized_catalog}`")
        if (
            normalized_schema is not None
            and normalized_catalog is not None
            and self.schema_exists(
                connection=connection,
                database=normalized_catalog,
                schema=normalized_schema,
            )
        ):
            statements.append(f"USE SCHEMA `{normalized_schema}`")
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

    @staticmethod
    def _normalize_session_value(value: object | None) -> str | None:
        if not isinstance(value, str):
            return None
        stripped: str = value.strip()
        if not stripped or stripped.startswith("<none"):
            return None
        return stripped

    @staticmethod
    def _string_literal(value: str) -> str:
        escaped_value: str = value.replace("'", "''")
        return f"'{escaped_value}'"

    @staticmethod
    def _information_schema(database: str) -> str:
        return f"`{database}`.information_schema"

    @staticmethod
    def _build_in_filter(*, column: str, values: tuple[str, ...]) -> str:
        literals: str = ", ".join(DatabricksAdapter._string_literal(value) for value in values)
        return f" AND {column} IN ({literals})"

    @staticmethod
    def _normalize_relation_type(value: str) -> str:
        normalized: str = value.lower()
        if normalized in TABLE_RELATION_METADATA_TYPES:
            return RelationType.TABLE
        return normalized

    @staticmethod
    def _to_databricks_type(column_type: str) -> str:
        normalized: str = column_type.upper()
        if any(token in normalized for token in ("CHAR", "TEXT", "STRING", "VARCHAR")):
            return "STRING"
        return normalized

    @staticmethod
    def _render_cursor_value_literal(cursor_value: CursorValue) -> str:
        if cursor_value.kind == CursorKind.INTEGER:
            return str(cursor_value.value)
        return f"TIMESTAMP '{cursor_value.value}'"

    @staticmethod
    def _render_cursor_bound_string(value: str) -> str:
        if value.isdigit():
            return value
        return f"TIMESTAMP '{value}'"

    @staticmethod
    def _to_int(value: object) -> int:
        if isinstance(value, int):
            return value
        return int(str(value))
