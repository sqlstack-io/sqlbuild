"""Snowflake adapter implementation."""

from __future__ import annotations

import csv
import json
import logging
import os
from collections.abc import Callable
from dataclasses import replace
from datetime import date, datetime
from decimal import Decimal
from functools import partial
from pathlib import Path
from typing import Any, ClassVar, cast

from sqlbuild._native import (
    SeedCsvSettings,
    SourceFreshnessRecord,
    SqlLexicalSyntax,
    SqlValue,
    TableType,
    collect_snowflake_cost,
    render_insert_source_freshness_records_sql,
)
from sqlbuild.adapter.contract.classes.base_adapter import (
    BaseAdapter,
    _encode_typed_json,
    _render_ansi_typed_scalar,
    _render_typed_value_list,
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
    DIFF_LEFT_SIDE,
    DIFF_RIGHT_SIDE,
)
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.contract.main.column_name_key import column_name_key
from sqlbuild.adapter.contract.main.complete_table_freshness_results import (
    complete_table_freshness_results,
)
from sqlbuild.adapter.contract.main.is_quoted_identifier import is_quoted_identifier
from sqlbuild.adapter.contract.main.normalize_seed_csv_value import normalize_seed_csv_value
from sqlbuild.adapter.contract.main.run_relation_read_probe import run_relation_read_probe
from sqlbuild.adapter.contract.main.same_view_definition import same_view_definition
from sqlbuild.adapter.contract.models import (
    ColumnInfo,
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
    RetentionChangePhase,
    RetentionScope,
    SnapshotLatestVersionStyle,
    SnapshotUpdateStyle,
    StatementSizeLimit,
    TablePromotionMode,
)
from sqlbuild.adapter.relations.classes.inspection_catalog import InspectionCatalog
from sqlbuild.adapter.relations.classes.run_relation_listings import RunRelationListings
from sqlbuild.adapter.relations.constants import INSPECTION_IN_LIST_LIMIT
from sqlbuild.adapter.relations.main.active_inspection_catalog import active_inspection_catalog
from sqlbuild.adapter.relations.main.cached_view_definition import cached_view_definition
from sqlbuild.adapter.relations.main.relation_age_timestamp import relation_age_timestamp_utc
from sqlbuild.adapter.relations.main.render_inspection_sql import render_inspection_sql
from sqlbuild.adapter.relations.main.run_bounded_inspections import run_bounded_inspections
from sqlbuild.adapter.relations.main.run_recorded_inspection_query import (
    run_recorded_inspection_query,
)
from sqlbuild.adapter.relations.models import (
    ListedRelation,
    SchemaColumnListing,
    SchemaRelationListing,
)
from sqlbuild.adapter.relations.types import SchemaListingScope
from sqlbuild.adapter.type_system.main.conditional_result_nullability import (
    conditional_result_nullability,
)
from sqlbuild.adapter.type_system.main.first_arg_nullability import first_arg_nullability
from sqlbuild.adapter.type_system.main.normalize_numeric_family import normalize_numeric_family
from sqlbuild.adapter.type_system.main.types_equal import types_equal
from sqlbuild.adapters.snowflake._helpers.column_names import logical_column_name
from sqlbuild.adapters.snowflake._helpers.cost import cost_failure_cause
from sqlbuild.adapters.snowflake._helpers.grants import (
    render_snowflake_view_grants,
    render_snowflake_view_revokes,
    show_grants_object_kind,
    snowflake_relation_grants,
)
from sqlbuild.adapters.snowflake._helpers.metadata_types import (
    information_schema_type,
    show_columns_type,
)
from sqlbuild.adapters.snowflake._helpers.relation_usage import read_relation_usage
from sqlbuild.adapters.snowflake._helpers.show_metadata import (
    is_missing_object_error,
    listed_relation_from_show_table,
    listed_relation_from_show_view,
    show_like_pattern,
)
from sqlbuild.adapters.snowflake._helpers.state_table_retention import (
    is_invalid_retention_error,
)
from sqlbuild.adapters.snowflake.classes.snowflake_connection import _SnowflakeConnection
from sqlbuild.adapters.snowflake.classes.state_table_retention_fallbacks import (
    StateTableRetentionFallbacks,
)
from sqlbuild.adapters.snowflake.constants import (
    BASE_TABLE_METADATA_TYPE,
    CURRENT_DATABASE_ATTRIBUTE,
    EXTERNAL_BROWSER_AUTHENTICATOR,
    FIXED_POINT_RESULT_TYPE,
    MAX_STATEMENT_TIMEOUT_SECONDS,
    MFA_AUTHENTICATOR,
    OAUTH_AUTHORIZATION_CODE_AUTHENTICATOR,
    PLATFORM_DETECTION_DISABLED,
    PLATFORM_DETECTION_VARIABLE,
    SECONDARY_ROLES_ALL,
    SECONDARY_ROLES_NONE,
    SHOW_LIKE_ESCAPE_CHARACTER,
    STATUS_COLUMN_NAME,
    SUCCESS_STATUS_TOKENS,
    TEMPORARY_FUNCTION_PREFIX,
    TRUE_METADATA_VALUE,
    VIEW_RELATION_TYPE_TOKEN,
)
from sqlbuild.compiler.compile.types import FunctionLanguage
from sqlbuild.cost.models import RunCostSummary
from sqlbuild.cost.types import CostCapability, CostStatus
from sqlbuild.diagnostics.main.log_sql import log_sql
from sqlbuild.runtime.observability.classes.statement_lifecycle import StatementLifecycle
from sqlbuild.runtime.observability.constants import COST_TELEMETRY_PURPOSE
from sqlbuild.spec.contracts.constants import DEFAULT_SEED_CSV_SETTINGS

_EXACT_COLUMN_INSPECTION_LIMIT: int = 32
_STATE_TABLE_RETENTION_ATTRIBUTE: str = "_state_table_retention_fallbacks_state"
_METADATA_INSPECTION_CONCURRENCY: int = 8
_SHOW_RESULT_LIMIT: int = 10_000
_SHOW_OBJECT_RELATION_TYPES: dict[str, str] = {"TABLE": "base table", "VIEW": "view"}
_COST_LOGIN_TIMEOUT_SECONDS: int = 5
_COST_NETWORK_TIMEOUT_SECONDS: int = 15
_SHOW_COLUMN_KIND: str = "COLUMN"
_SHOW_COLUMN_KINDS: frozenset[str] = frozenset({_SHOW_COLUMN_KIND, "VIRTUAL_COLUMN"})
_UNRESOLVED: object = object()
_RELATION_LISTING_COLUMNS: str = (
    "table_name, table_schema, table_type, is_transient, created, last_altered, retention_time"
)
_BULK_COLUMN_RELATION_CHUNK_SIZE: int = 2_000


class SnowflakeAdapter(MicrobatchMixin, UnkeyedDiffMixin, BaseAdapter):
    """Snowflake adapter backed by snowflake-connector-python."""

    adapter_name: ClassVar[str] = BuiltinAdapter.SNOWFLAKE.value
    sql_analysis_dialect_name: ClassVar[str | None] = "snowflake"
    sql_lexical_syntax: ClassVar[SqlLexicalSyntax] = SqlLexicalSyntax(
        backslash_escape_quotes=frozenset({"'"}), line_comment_prefixes=frozenset({"--", "//"})
    )
    execution_duration_limit_seconds: ClassVar[int | None] = MAX_STATEMENT_TIMEOUT_SECONDS
    statement_limit_excludes_queueing: ClassVar[bool] = True
    metadata_inspection_concurrency: ClassVar[int] = _METADATA_INSPECTION_CONCURRENCY
    concurrent_ddl_limit: ClassVar[int] = 16
    exact_column_inspection_limit: ClassVar[int] = _EXACT_COLUMN_INSPECTION_LIMIT

    max_identifier_length: ClassVar[int] = 255
    supports_session_warehouse: ClassVar[bool] = True
    connection_routing_keys: ClassVar[frozenset[str]] = frozenset(
        {"source", "profile", "target", "project_dir", "profiles_dir"}
    )
    _snapshot_sql_dialect: ClassVar[SnapshotSqlDialect] = SnapshotSqlDialect(
        timestamp_type="TIMESTAMP",
        distinct_condition=render_is_distinct_from,
        update_style=SnapshotUpdateStyle.UPDATE_FROM,
        latest_version=SnapshotLatestVersionStyle.QUALIFY,
        historical_close=HistoricalSnapshotCloseStyle.UPDATE_FROM,
        historical_insert=HistoricalSnapshotInsertStyle.INSERT_WITH,
    )

    def inspect_retention(self, *, connection: Any, request: RetentionRequest) -> RetentionState:
        return self.inspect_retentions(connection=connection, requests=(request,))[
            request.request_id
        ]

    def inspect_retentions(
        self, *, connection: Any, requests: tuple[RetentionRequest, ...]
    ) -> dict[str, RetentionState]:
        """Inspect relation retention with SHOW TABLES, which needs no running warehouse."""

        grouped: dict[tuple[str | None, str], list[RetentionRequest]] = {}
        for request in requests:
            self._validate_relation_retention_request(request=request)
            grouped.setdefault((request.database, request.schema), []).append(request)
        states: dict[str, RetentionState] = {}
        for (database, schema), group in grouped.items():
            relations: dict[str, RelationInfo | None] = self._retention_relations(
                connection=connection, database=database, schema=schema, group=tuple(group)
            )
            for request in group:
                relation: RelationInfo | None = relations.get(
                    self._information_schema_identifier(str(request.name))
                )
                state: RetentionState | None = (
                    None
                    if relation is None
                    else self.retention_state_from_relation(request=request, relation=relation)
                )
                if state is None:
                    raise AdapterUserError(
                        message=f"Snowflake retention metadata not found for {request.name}"
                    )
                states[request.request_id] = state
        return states

    def _retention_relations(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str,
        group: tuple[RetentionRequest, ...],
    ) -> dict[str, RelationInfo | None]:
        """The listed relation of each request, keyed by stored name."""

        scope: SchemaListingScope = self._listing_scope(
            connection=connection, database=database, schema=schema
        )
        relations: dict[str, RelationInfo | None] = {}
        unanswered: list[str] = []
        for request in group:
            stored_name: str = self._information_schema_identifier(str(request.name))
            answered, relation = RunRelationListings.listed_relation(
                adapter=self, scope=scope, name=str(request.name), stored_name=stored_name
            )
            if answered:
                relations[stored_name] = relation
            else:
                unanswered.append(stored_name)
        if not unanswered:
            return relations
        if len(set(unanswered)) == 1 and SHOW_LIKE_ESCAPE_CHARACTER not in unanswered[0]:
            relations[unanswered[0]] = self._show_table(
                connection=connection, database=database, schema=schema, stored_name=unanswered[0]
            )
            return relations
        listed: dict[str, RelationInfo] = RunRelationListings.refresh_schema_listing(
            adapter=self,
            scope=scope,
            read_listing=partial(
                self._schema_relations, connection=connection, database=database, schema=schema
            ),
        )
        for stored_name in unanswered:
            relations[stored_name] = listed.get(stored_name)
        return relations

    def _show_table(
        self, *, connection: Any, database: str | None, schema: str, stored_name: str
    ) -> RelationInfo | None:
        """One table as SHOW TABLES lists it, or None when the schema holds no such table."""

        show_database: str = self._show_database(
            connection=connection, database=database, schema=schema
        )
        scope: str = self._show_schema_scope(
            connection=connection, database=database, schema=schema
        )
        rows: list[dict[str, object]] = self._fetch_schema_show_rows(
            connection=connection,
            query=f"SHOW TABLES LIKE '{show_like_pattern(stored_name)}' IN SCHEMA {scope}",
            show_database=show_database,
            schema=schema,
        )
        row: dict[str, object]
        for row in rows:
            if str(row.get("name")) == stored_name:
                return listed_relation_from_show_table(row=row, database=database).relation
        return None

    def _schema_relations(
        self, *, connection: Any, database: str | None, schema: str
    ) -> dict[str, RelationInfo]:
        """Every table and view in one schema, keyed by stored name."""

        listing: SchemaRelationListing = self.read_schema_relation_listing(
            connection=connection, database=database, schema=schema
        )
        return {entry.stored_name: entry.relation for entry in listing.entries}

    def relation_listing_scope(
        self, *, connection: Any, database: str | None, schema: str
    ) -> SchemaListingScope:
        """The stored database and schema names that key a build's listing of one schema."""

        return self._listing_scope(connection=connection, database=database, schema=schema)

    def _listing_scope(
        self, *, connection: Any, database: str | None, schema: str
    ) -> SchemaListingScope:
        """The stored database and schema names that key a build's listing of one schema."""

        return (
            self._information_schema_identifier(
                self._show_database(connection=connection, database=database, schema=schema)
            ),
            self._information_schema_identifier(schema),
        )

    def retention_state_from_relation(
        self, *, request: RetentionRequest, relation: RelationInfo
    ) -> RetentionState | None:
        if relation.retention_days is None or relation.is_transient is None:
            return None
        return RetentionState(
            request_id=request.request_id,
            scope=request.scope,
            configured_days=None,
            effective_days=relation.retention_days,
            relation_kind="TRANSIENT" if relation.is_transient else "PERMANENT",
            is_transient=relation.is_transient,
        )

    def render_retention_changes(
        self, *, request: RetentionRequest, state: RetentionState | None = None
    ) -> tuple[RenderedRetentionChange, ...]:
        del state
        self._validate_relation_retention_request(request=request)
        target: str | None = self.render_qualified_name(
            database=request.database,
            schema=request.schema,
            name=str(request.name),
        )
        if target is None:
            raise AdapterUserError(message="Snowflake retention requires a schema and relation")
        return (
            RenderedRetentionChange(
                phase=RetentionChangePhase.ALTER,
                statements=(
                    f"ALTER TABLE {target} SET DATA_RETENTION_TIME_IN_DAYS = "
                    f"{request.desired_days}",
                ),
            ),
        )

    @staticmethod
    def _validate_relation_retention_request(*, request: RetentionRequest) -> None:
        if request.scope != RetentionScope.RELATION or request.name is None:
            raise AdapterUserError(message="Snowflake retention requires relation scope")
        if request.desired_days < 0:
            raise AdapterUserError(message="Snowflake retention days must be non-negative")

    def cost_capability(self) -> CostCapability:
        return CostCapability.SNOWFLAKE_QUERY_HISTORY

    def collect_run_cost(
        self,
        *,
        connection_config: dict[str, object],
        target_database: str | None,
        run_id: str,
        started_at: datetime,
        completed_at: datetime,
        statement_ledger_path: Path,
        usd_per_credit: Decimal,
        rate_source: str,
        budget_seconds: float | None = None,
    ) -> RunCostSummary:
        with StatementLifecycle.purpose_scope(COST_TELEMETRY_PURPOSE):
            return self._collect_run_cost(
                connection_config=connection_config,
                target_database=target_database,
                run_id=run_id,
                started_at=started_at,
                completed_at=completed_at,
                statement_ledger_path=statement_ledger_path,
                usd_per_credit=usd_per_credit,
                rate_source=rate_source,
                budget_seconds=budget_seconds,
            )

    def _collect_run_cost(
        self,
        *,
        connection_config: dict[str, object],
        target_database: str | None,
        run_id: str,
        started_at: datetime,
        completed_at: datetime,
        statement_ledger_path: Path,
        usd_per_credit: Decimal,
        rate_source: str,
        budget_seconds: float | None = None,
    ) -> RunCostSummary:
        telemetry_connection_config: dict[str, object] = dict(connection_config)
        telemetry_connection_config["login_timeout"] = _COST_LOGIN_TIMEOUT_SECONDS
        telemetry_connection_config["network_timeout"] = _COST_NETWORK_TIMEOUT_SECONDS
        try:
            connection: Any = self.connect(telemetry_connection_config)
        except Exception as error:
            return RunCostSummary(
                status=CostStatus.COLLECTION_FAILED,
                usd_per_credit=usd_per_credit,
                rate_source=rate_source,
                message=f"Snowflake cost collection failed to connect ({type(error).__name__}).",
            )
        try:
            summary: RunCostSummary = collect_snowflake_cost(
                connection=connection,
                database=target_database,
                run_id=run_id,
                started_at=started_at,
                completed_at=completed_at,
                statement_ledger_path=statement_ledger_path,
                usd_per_credit=usd_per_credit,
                rate_source=rate_source,
                budget_seconds=budget_seconds,
            )
        except Exception as error:
            summary = RunCostSummary(
                status=CostStatus.COLLECTION_FAILED,
                usd_per_credit=usd_per_credit,
                rate_source=rate_source,
                message=f"Snowflake cost collection failed ({cost_failure_cause(error)}).",
            )
        finally:
            try:
                self.close(connection)
            except Exception as error:
                summary = replace(
                    summary,
                    status=(
                        summary.status
                        if summary.status == CostStatus.COLLECTION_FAILED
                        else CostStatus.PARTIAL
                    ),
                    limitations=(
                        *summary.limitations,
                        f"Snowflake cost history connection close failed ({type(error).__name__}).",
                    ),
                )
        return summary

    @staticmethod
    def _information_schema_identifier(value: str) -> str:
        return value.upper()

    def _information_schema_relation(self, *, database: str | None, name: str) -> str:
        if database is None:
            return f"information_schema.{name}"
        return f"{self.render_identifier(database)}.information_schema.{name}"

    def _freshness_rows(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str | None,
        scoped_requests: list[TableFreshnessRequest],
    ) -> list[tuple[Any, ...]]:
        """Read LAST_ALTERED for only the requested tables, in bounded IN-list chunks."""

        names: tuple[str, ...] = tuple(
            dict.fromkeys(
                self._information_schema_identifier(request.name) for request in scoped_requests
            )
        )
        relation: str = self._information_schema_relation(
            database=scoped_requests[0].database, name="tables"
        )
        scope_clauses: list[str] = []
        scope_params: list[str] = []
        if database is not None:
            scope_clauses.append("table_catalog = %s")
            scope_params.append(database)
        if schema is not None:
            scope_clauses.append("table_schema = %s")
            scope_params.append(schema)
        chunks: tuple[tuple[str, ...], ...] = tuple(
            names[start : start + INSPECTION_IN_LIST_LIMIT]
            for start in range(0, len(names), INSPECTION_IN_LIST_LIMIT)
        )
        catalog: InspectionCatalog | None = active_inspection_catalog(
            adapter=self, connection=connection
        )
        chunk_rows: list[list[tuple[Any, ...]]] = run_bounded_inspections(
            tasks=tuple(
                self._bind_freshness_chunk(
                    connection=connection,
                    query=(
                        "SELECT table_catalog, table_schema, table_name, table_type, last_altered "
                        f"FROM {relation} WHERE "
                        + " AND ".join(
                            (*scope_clauses, f"table_name IN ({', '.join(['%s'] * len(chunk))})")
                        )
                    ),
                    params=(*scope_params, *chunk),
                )
                for chunk in chunks
            ),
            concurrency=1 if catalog is None else catalog.concurrency,
        )
        rows: list[tuple[Any, ...]] = []
        chunk_result: list[tuple[Any, ...]]
        for chunk_result in chunk_rows:
            rows.extend(chunk_result)
        return rows

    def _bind_freshness_chunk(
        self, *, connection: Any, query: str, params: tuple[str, ...]
    ) -> Callable[[], list[tuple[Any, ...]]]:
        def read() -> list[tuple[Any, ...]]:
            return self._fetch_inspection_rows(connection=connection, query=query, params=params)

        return read

    @staticmethod
    def _freshness_request_key(
        *, database: str | None, schema: str | None, name: str
    ) -> tuple[str | None, str | None, str]:
        return (
            None if database is None else database.upper(),
            None if schema is None else schema.upper(),
            name.upper(),
        )

    @staticmethod
    def _find_freshness_request_for_row(
        *,
        requests_by_key: dict[tuple[str | None, str | None, str], TableFreshnessRequest],
        database: object | None,
        schema: object | None,
        name: object,
    ) -> TableFreshnessRequest | None:
        row_database: str | None = None if database is None else str(database).upper()
        row_schema: str | None = None if schema is None else str(schema).upper()
        row_name: str = str(name).upper()
        return (
            requests_by_key.get((row_database, row_schema, row_name))
            or requests_by_key.get((None, row_schema, row_name))
            or requests_by_key.get((row_database, None, row_name))
            or requests_by_key.get((None, None, row_name))
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

        return self._with_state_table_retention(
            create_sql=build_source_freshness_create_table_sql(
                database=database,
                schema=schema,
                render_qualified_name=self.render_qualified_name,
                render_framework_type=self.render_framework_type,
            ),
            database=database,
            schema=schema,
        )

    def render_create_source_freshness_index_sqls(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        del database, schema
        return ()

    def render_create_microbatch_state_table_sql(self, *, database: str | None, schema: str) -> str:
        return self._with_state_table_retention(
            create_sql=super().render_create_microbatch_state_table_sql(
                database=database, schema=schema
            ),
            database=database,
            schema=schema,
        )

    def _with_state_table_retention(
        self, *, create_sql: str, database: str | None, schema: str
    ) -> str:
        """Render state-table DDL with the maximum time travel its schema accepts."""

        return self._state_table_retention_fallbacks().render(
            create_sql=create_sql, database=database, schema=schema
        )

    def _state_table_retention_fallbacks(self) -> StateTableRetentionFallbacks:
        fallbacks: StateTableRetentionFallbacks | None = self.__dict__.get(
            _STATE_TABLE_RETENTION_ATTRIBUTE
        )
        if fallbacks is not None:
            return fallbacks
        return self.__dict__.setdefault(
            _STATE_TABLE_RETENTION_ATTRIBUTE, StateTableRetentionFallbacks()
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

        return self._with_state_table_retention(
            create_sql=build_node_results_create_table_sql(
                database=database,
                schema=schema,
                render_qualified_name=self.render_qualified_name,
                render_framework_type=self.render_framework_type,
            ),
            database=database,
            schema=schema,
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

        return self._with_state_table_retention(
            create_sql=build_audit_results_create_table_sql(
                database=database,
                schema=schema,
                render_qualified_name=self.render_qualified_name,
                render_framework_type=self.render_framework_type,
            ),
            database=database,
            schema=schema,
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

        return self._with_state_table_retention(
            create_sql=build_janitor_events_create_table_sql(
                database=database,
                schema=schema,
                render_qualified_name=self.render_qualified_name,
                render_framework_type=self.render_framework_type,
            ),
            database=database,
            schema=schema,
        )

    def render_create_migration_state_table_sql(self, *, database: str | None, schema: str) -> str:
        from sqlbuild._native import render_migration_state_create_sql

        return self._with_state_table_retention(
            create_sql=render_migration_state_create_sql(
                kind="model",
                database=database,
                schema=schema,
                render_qualified_name=self.render_qualified_name,
                render_framework_type=self.render_framework_type,
            ),
            database=database,
            schema=schema,
        )

    def render_create_old_name_view_state_table_sql(
        self, *, database: str | None, schema: str
    ) -> str:
        from sqlbuild._native import render_migration_state_create_sql

        return self._with_state_table_retention(
            create_sql=render_migration_state_create_sql(
                kind="old_name",
                database=database,
                schema=schema,
                render_qualified_name=self.render_qualified_name,
                render_framework_type=self.render_framework_type,
            ),
            database=database,
            schema=schema,
        )

    def render_create_column_migration_state_table_sql(
        self, *, database: str | None, schema: str
    ) -> str:
        from sqlbuild._native import render_migration_state_create_sql

        return self._with_state_table_retention(
            create_sql=render_migration_state_create_sql(
                kind="column",
                database=database,
                schema=schema,
                render_qualified_name=self.render_qualified_name,
                render_framework_type=self.render_framework_type,
            ),
            database=database,
            schema=schema,
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
            f"DELETE FROM {table_name} AS target USING ("
            "SELECT node_type, node_name, ts, run_id FROM ("
            "SELECT node_type, node_name, ts, run_id, ROW_NUMBER() OVER ("
            "PARTITION BY node_type, node_name "
            "ORDER BY ts DESC, run_id DESC"
            f") AS __sqlbuild_history_rank FROM {table_name}"
            ") WHERE __sqlbuild_history_rank > "
            f"{retain_versions}"
            ") AS stale "
            "WHERE target.node_type = stale.node_type "
            "AND target.node_name = stale.node_name "
            "AND target.ts = stale.ts "
            "AND target.run_id = stale.run_id"
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
            f"DELETE FROM {table_name} AS target USING ("
            "SELECT source_name, target_database, target_schema, target_name, observed_at, run_id "
            "FROM ("
            "SELECT source_name, target_database, target_schema, target_name, observed_at, run_id, "
            "ROW_NUMBER() OVER ("
            "PARTITION BY source_name, target_database, target_schema, target_name "
            "ORDER BY observed_at DESC, run_id DESC"
            f") AS __sqlbuild_history_rank FROM {table_name}"
            ") WHERE __sqlbuild_history_rank > "
            f"{retain_versions}"
            ") AS stale "
            "WHERE target.source_name = stale.source_name "
            "AND EQUAL_NULL(target.target_database, stale.target_database) "
            "AND EQUAL_NULL(target.target_schema, stale.target_schema) "
            "AND EQUAL_NULL(target.target_name, stale.target_name) "
            "AND target.observed_at = stale.observed_at "
            "AND target.run_id = stale.run_id"
        )

    def supports_relation_age_metadata(self) -> bool:
        return True

    def supports_table_freshness_metadata(self) -> bool:
        return True

    def probe_relation_read(self, *, connection: Any, relation: str) -> RelationReadProbe:
        probe: RelationReadProbe = run_relation_read_probe(
            execute=self.execute,
            connection=connection,
            relation=relation,
            classify_not_found=self._classify_relation_not_found,
        )
        if probe.status != RelationReadStatus.MISSING_OR_UNREADABLE:
            return probe
        return RelationReadProbe(
            status=probe.status, role=self._current_role_name(connection=connection)
        )

    @staticmethod
    def _classify_relation_not_found(error: BaseException) -> RelationReadStatus | None:
        ambiguous: bool = isinstance(error, Exception) and is_missing_object_error(error)
        return RelationReadStatus.MISSING_OR_UNREADABLE if ambiguous else None

    @staticmethod
    def _current_role_name(*, connection: Any) -> str | None:
        role: object = getattr(getattr(connection, "raw_connection", None), "role", None)
        return str(role) if isinstance(role, str) and role else None

    def get_table_freshness_metadata(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> TableFreshnessMetadata:
        clauses: list[str] = ["table_name = %s"]
        params: list[str] = [self._information_schema_identifier(name)]
        if schema is not None:
            clauses.append("table_schema = %s")
            params.append(self._information_schema_identifier(schema))
        if database is not None:
            clauses.append("table_catalog = %s")
            params.append(self._information_schema_identifier(database))
        cursor: Any = connection.cursor()
        try:
            cursor.execute(
                "SELECT table_type, last_altered FROM "
                + self._information_schema_relation(database=database, name="tables")
                + " WHERE "
                + " AND ".join(clauses),
                tuple(params),
            )
            row: tuple[Any, ...] | None = cursor.fetchone()
        finally:
            cursor.close()
        if row is None:
            raise AdapterUserError(
                message=f"Snowflake table freshness metadata not found for {name}"
            )
        table_type: str = str(row[0]).upper()
        if table_type != BASE_TABLE_METADATA_TYPE:
            raise AdapterUserError(
                message="Snowflake table freshness metadata only supports physical tables; "
                f"found {table_type}"
            )
        if row[1] is None:
            raise AdapterUserError(
                message=f"Snowflake table freshness metadata is missing LAST_ALTERED for {name}"
            )
        return TableFreshnessMetadata(
            data_version=row[1],
            value_kind="timestamp",
            observed_at=row[1] if isinstance(row[1], datetime) else None,
        )

    def relation_usage(
        self,
        *,
        connection: Any,
        relations: tuple[RelationInfo, ...],
        window_seconds: int,
    ) -> RelationUsageReport:
        """Read candidate reads from ACCOUNT_USAGE.ACCESS_HISTORY, which lags up to 3 hours."""

        return read_relation_usage(
            connection=connection,
            relations=relations,
            window_seconds=window_seconds,
            current_database=lambda: self._current_database(connection=connection),
        )

    def get_tables_freshness_metadata(
        self,
        *,
        connection: Any,
        requests: tuple[TableFreshnessRequest, ...],
    ) -> dict[TableFreshnessRequest, TableFreshnessMetadata]:
        if not requests:
            return {}
        results: dict[TableFreshnessRequest, TableFreshnessMetadata] = {}
        requests_by_key: dict[tuple[str | None, str | None, str], TableFreshnessRequest] = {
            self._freshness_request_key(
                database=request.database,
                schema=request.schema,
                name=request.name,
            ): request
            for request in requests
        }
        requests_by_scope: dict[tuple[str | None, str | None], list[TableFreshnessRequest]] = {}
        request: TableFreshnessRequest
        for request in requests:
            scope: tuple[str | None, str | None] = (
                None
                if request.database is None
                else self._information_schema_identifier(request.database),
                None
                if request.schema is None
                else self._information_schema_identifier(request.schema),
            )
            requests_by_scope.setdefault(scope, []).append(request)
        for (database, schema), scoped_requests in requests_by_scope.items():
            rows: list[tuple[Any, ...]] = self._freshness_rows(
                connection=connection,
                database=database,
                schema=schema,
                scoped_requests=scoped_requests,
            )
            row: tuple[Any, ...]
            for row in rows:
                matched_request: TableFreshnessRequest | None = (
                    self._find_freshness_request_for_row(
                        requests_by_key=requests_by_key,
                        database=row[0],
                        schema=row[1],
                        name=row[2],
                    )
                )
                if matched_request is None:
                    continue
                table_type: str = str(row[3]).upper()
                if table_type != BASE_TABLE_METADATA_TYPE:
                    results[matched_request] = TableFreshnessMetadata.unavailable(
                        message="Snowflake table freshness metadata only supports physical tables; "
                        f"found {table_type} for {matched_request.name}"
                    )
                    continue
                if row[4] is None:
                    results[matched_request] = TableFreshnessMetadata.unavailable(
                        message="Snowflake table freshness metadata is missing LAST_ALTERED "
                        f"for {matched_request.name}"
                    )
                    continue
                results[matched_request] = TableFreshnessMetadata(
                    data_version=row[4],
                    value_kind="timestamp",
                    observed_at=row[4] if isinstance(row[4], datetime) else None,
                )
        return complete_table_freshness_results(
            requests=requests, results=results, adapter_label="Snowflake"
        )

    def persists_python_functions(self) -> bool:
        return True

    def python_functions_inherit_default_namespace(self) -> bool:
        return True

    def supports_unqualified_function_fingerprints(self) -> bool:
        return False

    def max_statement_size(self) -> StatementSizeLimit | None:
        """1 MB truncates metadata, not execution: https://docs.snowflake.com/en/user-guide/query-size-limits."""

        return None

    def maximum_identifier_length(self) -> int:
        """Return the maximum unqualified identifier length supported by the adapter."""

        return self.max_identifier_length

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

    def render_create_table_as(self, *, destination: str, sql: str) -> tuple[str, ...]:
        return self._render_create_table_as_type(
            destination=destination, sql=sql, table_type="transient"
        )

    def _render_create_table_as_type(
        self, *, destination: str, sql: str, table_type: str
    ) -> tuple[str, ...]:
        table_kind: str = "TABLE" if table_type == TableType.PERMANENT else "TRANSIENT TABLE"
        return (f"CREATE OR REPLACE {table_kind} {destination} AS {sql}",)

    def render_create_view_as(self, *, destination: str, sql: str) -> tuple[str, ...]:
        return (f"CREATE OR REPLACE VIEW {destination} AS {sql}",)

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
        return (*create_staged, delete_sql, *insert_stmts, f"DROP TABLE IF EXISTS {staged}")

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
        quoted_cursor_column: str = self.render_identifier(cursor_column)
        delete_sql: str = (
            f"DELETE FROM {destination} WHERE {quoted_cursor_column} >= '{cursor_start}' "
            f"AND {quoted_cursor_column} < '{cursor_end}'"
        )
        insert_stmts: tuple[str, ...] = self.render_append(
            destination=destination, sql=sql, columns=columns
        )
        return (delete_sql, *insert_stmts)

    def render_drop_view(self, *, destination: str, if_exists: bool = True) -> tuple[str, ...]:
        exists_clause: str = " IF EXISTS" if if_exists else ""
        return (f"DROP VIEW{exists_clause} {destination}",)

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

    def render_current_timestamp(self) -> str:
        return "CURRENT_TIMESTAMP"

    def create_table_as(
        self,
        *,
        connection: Any,
        destination: str,
        sql: str,
        config: dict[str, Any] | None = None,
        statement_recorder: StatementRecorder,
    ) -> None:
        table_type: str = str((config or {}).get("table_type", "transient"))
        statements: tuple[str, ...] = self._render_create_table_as_type(
            destination=destination, sql=sql, table_type=table_type
        )
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

    def drop_table(
        self, *, connection: Any, destination: str, statement_recorder: StatementRecorder
    ) -> None:
        statement: str = f"DROP TABLE IF EXISTS {destination}"
        statement_recorder.record(statement)
        self.execute(connection=connection, sql=statement)

    def create_table_as_replaces(self) -> bool:
        return True

    def list_relations_with_prefix(
        self, *, connection: Any, database: str | None, schemas: tuple[str, ...], prefix: str
    ) -> tuple[RelationInfo, ...] | None:
        """One SHOW TERSE OBJECTS for a single schema; None for several or past its row limit."""

        if len(schemas) != 1:
            return None
        schema: str = schemas[0]
        stored_prefix: str = self._information_schema_identifier(prefix)
        scope: str = self._show_schema_scope(
            connection=connection, database=database, schema=schema
        )
        rows: list[dict[str, object]] = self._fetch_schema_show_rows(
            connection=connection,
            query=(
                f"SHOW TERSE OBJECTS LIKE '{show_like_pattern(stored_prefix)}%' "
                f"IN SCHEMA {scope} LIMIT {_SHOW_RESULT_LIMIT}"
            ),
            show_database=self._show_database(
                connection=connection, database=database, schema=schema
            ),
            schema=schema,
        )
        if len(rows) >= _SHOW_RESULT_LIMIT:
            return None
        return tuple(
            RelationInfo(
                database=database,
                schema=str(row.get("schema_name") or schema).lower(),
                name=str(row["name"]).lower(),
                relation_type=_SHOW_OBJECT_RELATION_TYPES.get(
                    str(row.get("kind") or "").upper(), str(row.get("kind") or "").lower()
                ),
            )
            for row in rows
            if str(row["name"]).startswith(stored_prefix)
        )

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
            statements: tuple[str, ...] = self.render_rename(origin=origin, destination=destination)
            statement_recorder.record_many(statements)
            stmt: str
            for stmt in statements:
                self.execute(connection=connection, sql=stmt)
            return
        if not allow_copy_fallback:
            raise AdapterUserError(message="Snowflake relation move/copy requires --allow-copy")
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
    ) -> int | None:
        statements: tuple[str, ...] = self.render_append(
            destination=destination, sql=sql, columns=columns
        )
        statement_recorder.record_many(statements)
        return self._inserted_rows(connection=connection, statements=statements)

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
        with self.transaction(connection):
            return self._inserted_rows(connection=connection, statements=statements)

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
        with self.transaction(connection):
            return self._inserted_rows(connection=connection, statements=statements)

    def _inserted_rows(self, *, connection: Any, statements: tuple[str, ...]) -> int | None:
        """Run DML statements; the rows the INSERT among them wrote, as Snowflake reports them."""

        inserted: int | None = None
        stmt: str
        for stmt in statements:
            cursor: Any = self.execute(connection=connection, sql=stmt)
            if stmt.startswith("INSERT"):
                inserted = self.affected_row_count(cursor=cursor)
        return inserted

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
        """Render a logical identifier using Snowflake's unquoted uppercase semantics."""

        if is_quoted_identifier(name):
            return name
        return '"' + name.upper().replace('"', '""') + '"'

    def render_exact_identifier(self, name: str) -> str:
        """Render one explicitly quoted Snowflake identifier with its exact case."""

        return '"' + name.replace('"', '""') + '"'

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
                return "VARIANT"

    def render_typed_scalar(self, *, value: SqlValue) -> str:
        return _render_ansi_typed_scalar(value=value)

    def render_typed_value_list(self, *, value: SqlValue) -> str:
        return _render_typed_value_list(value=value, render_scalar=self.render_typed_scalar)

    def render_typed_array(self, *, value: SqlValue) -> str:
        return "ARRAY_CONSTRUCT(" + self._render_typed_array_items(value) + ")"

    def render_typed_object(self, *, value: SqlValue) -> str:
        return f"PARSE_JSON({self._quote_sql_string(_encode_typed_json(value))})"

    def render_loader_value_literal(
        self, *, value: object, logical_type: LoaderLogicalType | None
    ) -> str:
        if value is not None and logical_type == LoaderLogicalType.JSON:
            return f"PARSE_JSON({self._quote_sql_string(json.dumps(value, sort_keys=True))})"
        if value is None:
            return "NULL"
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
                f"{column_sql_types.get(column_name, 'VARCHAR')}) AS "
                f"{self.render_identifier(column_name)}"
                for column_name in column_names
            )
            return f"SELECT {projections} WHERE 1 = 0"
        json_columns: frozenset[str] = frozenset(
            column_name
            for column_name in column_names
            if inferred_types.get(column_name) == LoaderLogicalType.JSON
        )
        value_rows: list[str] = []
        for row in rows:
            row_values: list[str] = []
            for column_name in column_names:
                value: object = row.get(column_name)
                row_values.append(
                    self._quote_sql_string(json.dumps(value, sort_keys=True))
                    if column_name in json_columns and value is not None
                    else self.render_loader_value_literal(
                        value=value, logical_type=inferred_types.get(column_name)
                    )
                )
            value_rows.append(f"({', '.join(row_values)})")
        values_sql: str = ", ".join(value_rows)
        column_sql: str = ", ".join(
            self.render_identifier(column_name) for column_name in column_names
        )
        projections: list[str] = []
        for column_name in column_names:
            identifier: str = self.render_identifier(column_name)
            value_sql: str = (
                f"PARSE_JSON({identifier})" if column_name in json_columns else identifier
            )
            projections.append(
                f"CAST({value_sql} AS {column_sql_types[column_name]}) AS {identifier}"
                if column_name in column_sql_types
                else f"{value_sql} AS {identifier}"
                if column_name in json_columns
                else identifier
            )
        select_sql: str = ", ".join(projections)
        return f"SELECT {select_sql} FROM (VALUES {values_sql}) AS __loader_rows({column_sql})"

    def _quote_sql_string(self, value: str) -> str:
        return "'" + value.replace("\\", "\\\\").replace("'", "''") + "'"

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
        return self._render_create_table_as_type(
            table_type=table_type,
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
        return self._render_create_table_as_type(
            destination=destination, sql=historical_sql, table_type=table_type
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
        historical_sql: str = HistoricalTimestampSnapshotSql.changes_initial_select_sql(
            origin=origin,
            unique_key=unique_key,
            updated_at_column=updated_at_column,
            valid_from_column=valid_from_column,
            valid_to_column=valid_to_column,
            output_columns=output_columns,
        )
        return self._render_create_table_as_type(
            destination=destination, sql=historical_sql, table_type=table_type
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
        return self._render_create_table_as_type(
            destination=destination, sql=historical_sql, table_type=table_type
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

    def supports_python_functions(self) -> bool:
        return True

    def connect(self, config: dict[str, Any]) -> _SnowflakeConnection:
        """Open a Snowflake connection, with platform detection off unless the user set it."""

        _ = os.environ.setdefault(PLATFORM_DETECTION_VARIABLE, PLATFORM_DETECTION_DISABLED)
        try:
            import snowflake.connector
        except ImportError as error:
            raise AdapterUserError(
                message="Snowflake adapter requires optional dependency "
                "snowflake-connector-python. Install with: sqlbuild[snowflake]",
                code="A301",
            ) from error

        connect_config: dict[str, Any] = dict(config)
        for routing_key in self.connection_routing_keys:
            connect_config.pop(routing_key, None)
        authenticator: str = str(connect_config.get("authenticator", "")).lower()
        if authenticator == MFA_AUTHENTICATOR:
            connect_config.setdefault("client_request_mfa_token", True)
            connect_config.setdefault("client_store_temporary_credential", True)
        elif authenticator == OAUTH_AUTHORIZATION_CODE_AUTHENTICATOR:
            connect_config.setdefault("client_store_temporary_credential", True)
            connect_config.setdefault("oauth_enable_refresh_tokens", True)
        elif authenticator == EXTERNAL_BROWSER_AUTHENTICATOR:
            connect_config.setdefault("client_store_temporary_credential", True)
        warehouse: object | None = connect_config.get("warehouse")
        database: object | None = connect_config.get("database")
        schema: object | None = connect_config.get("schema")
        secondary_roles: str | None = self._normalize_secondary_roles(
            connect_config.pop("secondary_roles", None)
        )
        raw_connection: Any = snowflake.connector.connect(**connect_config)
        connection: _SnowflakeConnection = _SnowflakeConnection(raw_connection)
        self._initialize_session(
            connection=connection,
            secondary_roles=secondary_roles,
            warehouse=warehouse,
            database=database,
            schema=schema,
        )
        return connection

    def _execute(self, *, connection: _SnowflakeConnection, sql: str) -> Any:
        """Execute a SQL statement against a Snowflake connection."""

        fallbacks: StateTableRetentionFallbacks = self._state_table_retention_fallbacks()
        fallback_sql: str | None = fallbacks.fallback_for(sql)
        if fallback_sql is None:
            return self._execute_statement(connection=connection, sql=sql)
        if fallbacks.is_fallen_back(sql):
            return self._execute_statement(connection=connection, sql=fallback_sql)
        try:
            return self._execute_statement(connection=connection, sql=sql)
        except Exception as error:
            if not is_invalid_retention_error(error):
                raise
        fallbacks.remember_fallback(sql)
        StatementLifecycle.discard_failed_attempt()
        return self._execute_statement(connection=connection, sql=fallback_sql)

    def _execute_statement(self, *, connection: _SnowflakeConnection, sql: str) -> Any:
        log_sql(logger=logging.getLogger("sqlbuild.adapter.snowflake"), sql=sql)
        return connection.execute(sql)

    def query(self, *, connection: Any, sql: str, limit: int | None) -> QueryResult:
        """Execute SQL and return normalized rows for ad hoc query output."""

        cursor: Any = self.execute(connection=connection, sql=sql)
        try:
            description: Any | None = getattr(cursor, "description", None)
            if description is None:
                return QueryResult()
            columns: tuple[str, ...] = tuple(str(column[0]) for column in description)
            column_types: tuple[str | None, ...] = tuple(
                self._result_column_type(column) for column in description
            )
            if limit is None:
                rows: tuple[tuple[object, ...], ...] = tuple(
                    tuple(row) for row in cursor.fetchall()
                )
                if self._is_status_only_non_row_result(columns=columns, rows=rows):
                    return QueryResult()
                return QueryResult(columns=columns, rows=rows, column_types=column_types)
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
                column_types=column_types,
            )
        finally:
            cursor.close()

    @staticmethod
    def _result_column_type(column: Any) -> str | None:
        """Name a result column's Snowflake type from its cursor description entry."""

        from snowflake.connector.constants import FIELD_ID_TO_NAME

        type_name: str | None = FIELD_ID_TO_NAME.get(column[1])
        if type_name is None:
            return None
        if type_name != FIXED_POINT_RESULT_TYPE:
            return type_name
        precision: int | None = column[4]
        scale: int | None = column[5]
        if precision is None:
            return "NUMBER"
        return f"NUMBER({precision},{scale or 0})"

    @staticmethod
    def _is_status_only_non_row_result(
        *, columns: tuple[str, ...], rows: tuple[tuple[object, ...], ...]
    ) -> bool:
        if len(columns) != 1 or columns[0].lower() != STATUS_COLUMN_NAME or len(rows) != 1:
            return False
        status_value: object = rows[0][0]
        if not isinstance(status_value, str):
            return False
        lowered: str = status_value.lower()
        return any(token in lowered for token in SUCCESS_STATUS_TOKENS)

    def relation_exists(
        self,
        *,
        connection: _SnowflakeConnection,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> bool:
        """Look one relation up with SHOW, which needs no running warehouse."""

        if schema is None or SHOW_LIKE_ESCAPE_CHARACTER in name:
            return self._information_schema_relation_exists(
                connection=connection, database=database, schema=schema, name=name
            )
        show_database: str = self._show_database(
            connection=connection, database=database, schema=schema
        )
        scope: str = self._show_schema_scope(
            connection=connection, database=database, schema=schema
        )
        stored_name: str = self._information_schema_identifier(name)
        answered, listed = RunRelationListings.schema_relation(
            adapter=self,
            scope=self._listing_scope(connection=connection, database=database, schema=schema),
            name=name,
            stored_name=stored_name,
            read_listing=partial(
                self._schema_relations, connection=connection, database=database, schema=schema
            ),
        )
        if answered:
            return listed is not None
        pattern: str = show_like_pattern(stored_name)
        kind: str
        for kind in ("TABLES", "VIEWS"):
            rows: list[dict[str, object]] = self._fetch_schema_show_rows(
                connection=connection,
                query=f"SHOW TERSE {kind} LIKE '{pattern}' IN SCHEMA {scope}",
                show_database=show_database,
                schema=schema,
            )
            if any(str(row.get("name")) == stored_name for row in rows):
                return True
        return False

    def _information_schema_relation_exists(
        self,
        *,
        connection: _SnowflakeConnection,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> bool:
        return (
            self._information_schema_table_row(
                connection=connection,
                database=database,
                schema=schema,
                name=name,
                projection="1",
            )
            is not None
        )

    def _information_schema_table_row(
        self,
        *,
        connection: _SnowflakeConnection,
        database: str | None,
        schema: str | None,
        name: str,
        projection: str,
    ) -> tuple[Any, ...] | None:
        clauses: list[str] = ["table_name = %s"]
        params: list[str] = [self._information_schema_identifier(name)]
        if schema is not None:
            clauses.append("table_schema = %s")
            params.append(self._information_schema_identifier(schema))
        if database is not None:
            clauses.append("table_catalog = %s")
            params.append(self._information_schema_identifier(database))
        cursor: Any = connection.cursor()
        try:
            cursor.execute(
                f"SELECT {projection} FROM "
                + self._information_schema_relation(database=database, name="tables")
                + " WHERE "
                + " AND ".join(clauses),
                tuple(params),
            )
            return cursor.fetchone()
        finally:
            cursor.close()

    def list_relations(
        self,
        *,
        connection: _SnowflakeConnection,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[Any, ...]:
        query: str = (
            f"SELECT {_RELATION_LISTING_COLUMNS} "
            f"FROM {self._information_schema_relation(database=database, name='tables')} WHERE 1=1"
        )
        params: list[str] = []
        if schemas:
            placeholders: str = ", ".join(["%s"] * len(schemas))
            query += f" AND table_schema IN ({placeholders})"
            params.extend(self._information_schema_identifier(schema) for schema in schemas)
        if names:
            placeholders = ", ".join(["%s"] * len(names))
            query += f" AND table_name IN ({placeholders})"
            params.extend(self._information_schema_identifier(name) for name in names)
        if database is not None:
            query += " AND table_catalog = %s"
            params.append(self._information_schema_identifier(database))
        cursor: Any = connection.cursor()
        try:
            cursor.execute(query, tuple(params))
            rows: list[tuple[Any, ...]] = cursor.fetchall()
        finally:
            cursor.close()
        return tuple(self._relation_info_from_row(row=row, database=database) for row in rows)

    @staticmethod
    def _relation_info_from_row(*, row: tuple[Any, ...], database: str | None) -> RelationInfo:
        return RelationInfo(
            database=None if row[1] is None else database,
            schema=None if row[1] is None else str(row[1]).lower(),
            name=str(row[0]).lower(),
            relation_type=str(row[2]).lower(),
            is_transient=(None if row[3] is None else str(row[3]).upper() == TRUE_METADATA_VALUE),
            created_at=relation_age_timestamp_utc(row[4]),
            last_altered_at=relation_age_timestamp_utc(row[5]),
            retention_days=None if row[6] is None else int(row[6]),
        )

    def read_schema_relation_listing(
        self, *, connection: _SnowflakeConnection, database: str | None, schema: str
    ) -> SchemaRelationListing:
        """List one schema's tables and views with SHOW, which needs no running warehouse."""

        show_database: str = self._show_database(
            connection=connection, database=database, schema=schema
        )
        scope: str = self._show_schema_scope(
            connection=connection, database=database, schema=schema
        )
        tables: list[dict[str, object]] = self._fetch_paged_show_rows(
            connection=connection,
            query=f"SHOW TABLES IN SCHEMA {scope}",
            show_database=show_database,
            schema=schema,
        )
        views: list[dict[str, object]] = self._fetch_paged_show_rows(
            connection=connection,
            query=f"SHOW VIEWS IN SCHEMA {scope}",
            show_database=show_database,
            schema=schema,
        )
        entries: dict[str, ListedRelation] = {}
        row: dict[str, object]
        for row in tables:
            listed: ListedRelation = listed_relation_from_show_table(row=row, database=database)
            entries[listed.stored_name] = listed
        for row in views:
            listed = listed_relation_from_show_view(row=row, database=database)
            entries[listed.stored_name] = listed
        return SchemaRelationListing(
            database=database, schema=schema, entries=tuple(entries.values())
        )

    def read_schema_column_listing(
        self,
        *,
        connection: _SnowflakeConnection,
        database: str | None,
        schema: str,
        stored_names: frozenset[str],
        known_capped: bool = False,
    ) -> SchemaColumnListing:
        """List one schema's columns with SHOW COLUMNS; a cap-sized result rereads only names."""

        if not known_capped:
            scope: str = self._show_schema_scope(
                connection=connection, database=database, schema=schema
            )
            rows: list[dict[str, object]] = self._fetch_schema_show_rows(
                connection=connection,
                query=f"SHOW COLUMNS IN SCHEMA {scope}",
                show_database=self._show_database(
                    connection=connection, database=database, schema=schema
                ),
                schema=schema,
            )
            if len(rows) != _SHOW_RESULT_LIMIT:
                columns: dict[str, list[ColumnInfo]] = {}
                row: dict[str, object]
                for row in rows:
                    if str(row.get("kind") or _SHOW_COLUMN_KIND).upper() not in _SHOW_COLUMN_KINDS:
                        continue
                    columns.setdefault(str(row["table_name"]), []).append(
                        ColumnInfo(
                            name=logical_column_name(str(row["column_name"])),
                            type=show_columns_type(row["data_type"]),
                        )
                    )
                return SchemaColumnListing(
                    database=database,
                    schema=schema,
                    columns_by_stored_name={name: tuple(value) for name, value in columns.items()},
                )
        return replace(
            self._read_information_schema_column_listing(
                connection=connection,
                database=database,
                schema=schema,
                stored_names=stored_names,
            ),
            complete=False,
        )

    def _read_information_schema_column_listing(
        self,
        *,
        connection: _SnowflakeConnection,
        database: str | None,
        schema: str,
        stored_names: frozenset[str] | None = None,
    ) -> SchemaColumnListing:
        """Read columns from INFORMATION_SCHEMA, unfiltered or for names in capped IN chunks."""

        query: str = (
            "SELECT table_name, column_name, data_type, numeric_precision, numeric_scale, "
            "character_maximum_length FROM "
            + self._information_schema_relation(database=database, name="columns")
            + " WHERE table_schema = %s"
        )
        params: list[str] = [self._information_schema_identifier(schema)]
        if database is not None:
            query += " AND table_catalog = %s"
            params.append(self._information_schema_identifier(database))
        names: tuple[str, ...] = tuple(sorted(stored_names or ()))
        chunks: tuple[tuple[str, ...], ...] = (
            ((),)
            if stored_names is None
            else tuple(
                names[start : start + INSPECTION_IN_LIST_LIMIT]
                for start in range(0, len(names), INSPECTION_IN_LIST_LIMIT)
            )
        )
        catalog: InspectionCatalog | None = active_inspection_catalog(
            adapter=self, connection=connection
        )
        chunk_rows: list[list[tuple[Any, ...]]] = run_bounded_inspections(
            tasks=tuple(
                self._bind_freshness_chunk(
                    connection=connection,
                    query=query
                    + (f" AND table_name IN ({', '.join(['%s'] * len(chunk))})" if chunk else "")
                    + " ORDER BY table_name, ordinal_position",
                    params=(*params, *chunk),
                )
                for chunk in chunks
            ),
            concurrency=1 if catalog is None else catalog.concurrency,
        )
        columns: dict[str, list[ColumnInfo]] = {}
        rows: list[tuple[Any, ...]]
        row: tuple[Any, ...]
        for rows in chunk_rows:
            for row in rows:
                columns.setdefault(str(row[0]), []).append(
                    ColumnInfo(
                        name=logical_column_name(str(row[1])),
                        type=information_schema_type(
                            data_type=str(row[2]),
                            numeric_precision=row[3],
                            numeric_scale=row[4],
                            character_maximum_length=row[5],
                        ),
                    )
                )
        return SchemaColumnListing(
            database=database,
            schema=schema,
            columns_by_stored_name={name: tuple(value) for name, value in columns.items()},
        )

    def _show_schema_scope(
        self, *, connection: _SnowflakeConnection, database: str | None, schema: str
    ) -> str:
        """Qualify SHOW with the database so a missing one is reported, never listed empty."""

        resolved: str = self._show_database(connection=connection, database=database, schema=schema)
        return f"{self.render_identifier(resolved)}.{self.render_identifier(schema)}"

    def _show_database(
        self, *, connection: _SnowflakeConnection, database: str | None, schema: str
    ) -> str:
        resolved: str | None = (
            database if database is not None else self._current_database(connection=connection)
        )
        if resolved is None:
            raise AdapterUserError(
                message=f"Snowflake schema {schema} needs a database; the session has none"
            )
        return resolved

    @staticmethod
    def _current_database(*, connection: _SnowflakeConnection) -> str | None:
        cached: object = getattr(connection, CURRENT_DATABASE_ATTRIBUTE, _UNRESOLVED)
        if cached is not _UNRESOLVED:
            return cast(str | None, cached)
        cursor: Any = connection.cursor()
        try:
            cursor.execute("SELECT CURRENT_DATABASE()")
            row: tuple[Any, ...] | None = cursor.fetchone()
        finally:
            cursor.close()
        current: str | None = None if row is None or row[0] is None else str(row[0])
        setattr(connection, CURRENT_DATABASE_ATTRIBUTE, current)
        return current

    def _fetch_paged_show_rows(
        self, *, connection: _SnowflakeConnection, query: str, show_database: str, schema: str
    ) -> list[dict[str, object]]:
        """Page SHOW output past its row cap; SHOW orders rows by name."""

        rows: list[dict[str, object]] = []
        page_query: str = f"{query} LIMIT {_SHOW_RESULT_LIMIT}"
        while True:
            page: list[dict[str, object]] = self._fetch_schema_show_rows(
                connection=connection, query=page_query, show_database=show_database, schema=schema
            )
            rows.extend(page)
            if len(page) < _SHOW_RESULT_LIMIT:
                return rows
            last_name: str = str(page[-1]["name"]).replace("'", "''")
            page_query = f"{query} LIMIT {_SHOW_RESULT_LIMIT} FROM '{last_name}'"

    @staticmethod
    def _fetch_show_rows(
        *, connection: _SnowflakeConnection, query: str
    ) -> list[dict[str, object]]:
        """Run one SHOW command and return its rows keyed by lowercase output column."""

        def fetch() -> list[dict[str, object]]:
            cursor: Any = connection.cursor()
            try:
                cursor.execute(query)
                names: list[str] = [str(column[0]).lower() for column in cursor.description or ()]
                return [dict(zip(names, row, strict=False)) for row in cursor.fetchall()]
            finally:
                cursor.close()

        return run_recorded_inspection_query(sql=query, run=fetch)

    def _fetch_schema_show_rows(
        self, *, connection: _SnowflakeConnection, query: str, show_database: str, schema: str
    ) -> list[dict[str, object]]:
        """Run SHOW ... IN SCHEMA; a schema that does not exist lists nothing, as before."""

        try:
            return self._fetch_show_rows(connection=connection, query=query)
        except Exception as error:
            if not is_missing_object_error(error) or self._schema_exists(
                connection=connection, show_database=show_database, schema=schema
            ):
                raise
            return []

    def _schema_exists(
        self, *, connection: _SnowflakeConnection, show_database: str, schema: str
    ) -> bool:
        """Tell a missing schema from a missing database or another failure, once per plan."""

        def read() -> bool:
            return self._read_schema_exists(
                connection=connection, show_database=show_database, schema=schema
            )

        catalog: InspectionCatalog | None = active_inspection_catalog(
            adapter=self, connection=connection
        )
        if catalog is None:
            return read()
        return catalog.remember(
            key=(
                "snowflake_schema_exists",
                self._information_schema_identifier(show_database),
                self._information_schema_identifier(schema),
            ),
            compute=read,
        )

    def _read_schema_exists(
        self, *, connection: _SnowflakeConnection, show_database: str, schema: str
    ) -> bool:
        stored_schema: str = self._information_schema_identifier(schema)
        query: str = (
            f"SHOW SCHEMAS LIKE '{stored_schema.replace(chr(39), chr(39) * 2)}' "
            f"IN DATABASE {self.render_identifier(show_database)}"
        )
        try:
            rows: list[dict[str, object]] = self._fetch_show_rows(
                connection=connection, query=query
            )
        except Exception as error:
            if not is_missing_object_error(error):
                raise
            raise AdapterUserError(
                message=(
                    f"Snowflake database {show_database} does not exist or the current role "
                    f"cannot use it: {error}"
                )
            ) from error
        return any(str(row.get("name")) == stored_schema for row in rows)

    def read_relation_columns(
        self, *, connection: _SnowflakeConnection, relation: RelationInfo
    ) -> tuple[ColumnInfo, ...] | None:
        """Read one relation's columns with SHOW COLUMNS; None once it no longer exists."""

        try:
            return self._show_columns_for_relation(connection=connection, relation=relation)
        except Exception as error:
            if is_missing_object_error(error):
                return None
            raise

    def metadata_name_key(self, name: str) -> str:
        """Unquoted logical identifiers match the uppercase name Snowflake stores."""

        return self._information_schema_identifier(name)

    @staticmethod
    def _fetch_inspection_rows(
        *, connection: _SnowflakeConnection, query: str, params: tuple[str, ...]
    ) -> list[tuple[Any, ...]]:
        def fetch() -> list[tuple[Any, ...]]:
            cursor: Any = connection.cursor()
            try:
                cursor.execute(query, params)
                return list(cursor.fetchall())
            finally:
                cursor.close()

        return run_recorded_inspection_query(
            sql=render_inspection_sql(query=query, params=params), run=fetch
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
        connection: _SnowflakeConnection,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[FunctionInfo, ...]:
        query: str = (
            "SELECT function_name, function_schema, 'function' "
            "FROM "
            + self._information_schema_relation(database=database, name="functions")
            + " WHERE 1=1"
        )
        params: list[str] = []
        if schemas:
            placeholders: str = ", ".join(["%s"] * len(schemas))
            query += f" AND UPPER(function_schema) IN ({placeholders})"
            params.extend(schema.upper() for schema in schemas)
        if names:
            placeholders = ", ".join(["%s"] * len(names))
            query += f" AND UPPER(function_name) IN ({placeholders})"
            params.extend(name.upper() for name in names)
        if database is not None:
            query += " AND UPPER(function_catalog) = UPPER(%s)"
            params.append(database)
        cursor: Any = connection.cursor()
        try:
            cursor.execute(query, tuple(params))
            rows: list[tuple[Any, ...]] = cursor.fetchall()
        finally:
            cursor.close()
        return tuple(
            FunctionInfo(
                database=None if row[1] is None else database,
                schema=None if row[1] is None else str(row[1]).lower(),
                name=str(row[0]).lower(),
                function_type=str(row[2]).lower(),
            )
            for row in rows
        )

    def get_columns(
        self,
        *,
        connection: _SnowflakeConnection,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> tuple[ColumnInfo, ...]:
        """Read one relation's columns with SHOW COLUMNS, which needs no running warehouse."""

        if schema is None:
            return self._information_schema_columns(
                connection=connection, database=database, schema=schema, name=name
            )
        show_database: str = self._show_database(
            connection=connection, database=database, schema=schema
        )
        relation: str = ".".join(
            self.render_identifier(part) for part in (show_database, schema, name)
        )
        kind: str
        for kind in ("TABLE", "VIEW"):
            try:
                rows: list[dict[str, object]] = self._fetch_show_rows(
                    connection=connection, query=f"SHOW COLUMNS IN {kind} {relation}"
                )
            except Exception as error:
                if not is_missing_object_error(error):
                    raise
                continue
            return tuple(
                ColumnInfo(
                    name=logical_column_name(str(row["column_name"])),
                    type=show_columns_type(row["data_type"]),
                )
                for row in rows
                if str(row.get("kind") or _SHOW_COLUMN_KIND).upper() in _SHOW_COLUMN_KINDS
            )
        _ = self._schema_exists(connection=connection, show_database=show_database, schema=schema)
        return ()

    def _information_schema_columns(
        self,
        *,
        connection: _SnowflakeConnection,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> tuple[ColumnInfo, ...]:
        query: str = (
            "SELECT column_name, data_type, numeric_precision, numeric_scale, "
            "character_maximum_length FROM "
            + self._information_schema_relation(database=database, name="columns")
            + " "
            "WHERE table_name = %s"
        )
        params: list[str] = [self._information_schema_identifier(name)]
        if schema is not None:
            query += " AND table_schema = %s"
            params.append(self._information_schema_identifier(schema))
        if database is not None:
            query += " AND table_catalog = %s"
            params.append(self._information_schema_identifier(database))
        query += " ORDER BY ordinal_position"
        cursor: Any = connection.cursor()
        try:
            cursor.execute(query, tuple(params))
            rows: list[tuple[Any, ...]] = cursor.fetchall()
        finally:
            cursor.close()
        return tuple(
            ColumnInfo(
                name=logical_column_name(str(row[0])),
                type=self._build_information_schema_type(
                    data_type=str(row[1]),
                    numeric_precision=row[2],
                    numeric_scale=row[3],
                    character_maximum_length=row[4],
                ),
            )
            for row in rows
        )

    def get_all_columns(
        self,
        *,
        connection: _SnowflakeConnection,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> dict[str, tuple[ColumnInfo, ...]]:
        query: str = (
            "SELECT table_name, column_name, data_type, numeric_precision, numeric_scale, "
            "character_maximum_length FROM "
            + self._information_schema_relation(database=database, name="columns")
            + " WHERE 1=1"
        )
        params: list[str] = []
        if schemas:
            placeholders: str = ", ".join(["%s"] * len(schemas))
            query += f" AND table_schema IN ({placeholders})"
            params.extend(self._information_schema_identifier(schema) for schema in schemas)
        if names:
            placeholders = ", ".join(["%s"] * len(names))
            query += f" AND table_name IN ({placeholders})"
            params.extend(self._information_schema_identifier(name) for name in names)
        if database is not None:
            query += " AND table_catalog = %s"
            params.append(self._information_schema_identifier(database))
        query += " ORDER BY table_name, ordinal_position"
        cursor: Any = connection.cursor()
        try:
            cursor.execute(query, tuple(params))
            rows: list[tuple[Any, ...]] = cursor.fetchall()
        finally:
            cursor.close()
        result: dict[str, list[ColumnInfo]] = {}
        row: tuple[Any, ...]
        for row in rows:
            table_name: str = str(row[0]).lower()
            result.setdefault(table_name, []).append(
                ColumnInfo(
                    name=logical_column_name(str(row[1])),
                    type=self._build_information_schema_type(
                        data_type=str(row[2]),
                        numeric_precision=row[3],
                        numeric_scale=row[4],
                        character_maximum_length=row[5],
                    ),
                )
            )
        return {key: tuple(value) for key, value in result.items()}

    def get_columns_for_relations(
        self,
        *,
        connection: _SnowflakeConnection,
        relations: tuple[RelationInfo, ...],
    ) -> dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]]:
        """Use exact SHOW calls for sparse plans and qualified bulk metadata for broad plans."""

        ordered_relations: tuple[RelationInfo, ...] = tuple(
            sorted(
                relations,
                key=lambda relation: tuple(part or "" for part in relation.identity),
            )
        )
        if len(ordered_relations) <= _EXACT_COLUMN_INSPECTION_LIMIT:
            exact: dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]] = {}
            for relation in ordered_relations:
                columns: tuple[ColumnInfo, ...] | None = self.read_relation_columns(
                    connection=connection, relation=relation
                )
                if columns is not None:
                    exact[relation.identity] = columns
            return exact
        return self._get_columns_for_relations_bulk(
            connection=connection, relations=ordered_relations
        )

    def _show_columns_for_relation(
        self,
        *,
        connection: _SnowflakeConnection,
        relation: RelationInfo,
    ) -> tuple[ColumnInfo, ...]:
        qualified_name: str | None = self.render_qualified_name(
            database=relation.database,
            schema=relation.schema,
            name=relation.name,
        )
        if qualified_name is None:
            raise AdapterUserError(message=f"Snowflake relation is not qualified: {relation.name}")
        relation_kind: str = (
            "VIEW" if VIEW_RELATION_TYPE_TOKEN in relation.relation_type.lower() else "TABLE"
        )
        query: str = f"SHOW COLUMNS IN {relation_kind} {qualified_name}"

        def fetch() -> list[tuple[Any, ...]]:
            cursor: Any = connection.cursor()
            try:
                cursor.execute(query)
                return list(cursor.fetchall())
            finally:
                cursor.close()

        rows: list[tuple[Any, ...]] = run_recorded_inspection_query(sql=query, run=fetch)
        return tuple(
            ColumnInfo(
                name=logical_column_name(str(row[2])),
                type=show_columns_type(row[3]),
            )
            for row in rows
        )

    def _get_columns_for_relations_bulk(
        self,
        *,
        connection: _SnowflakeConnection,
        relations: tuple[RelationInfo, ...],
    ) -> dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]]:
        by_database: dict[str | None, list[RelationInfo]] = {}
        for relation in relations:
            by_database.setdefault(relation.database, []).append(relation)
        result: dict[tuple[str | None, str | None, str], list[ColumnInfo]] = {}
        for database, database_relations in by_database.items():
            chunk_start: int
            for chunk_start in range(0, len(database_relations), _BULK_COLUMN_RELATION_CHUNK_SIZE):
                chunk: list[RelationInfo] = database_relations[
                    chunk_start : chunk_start + _BULK_COLUMN_RELATION_CHUNK_SIZE
                ]
                rows: list[tuple[Any, ...]] = self._get_columns_for_relations_bulk_chunk(
                    connection=connection, database=database, relations=chunk
                )
                for row in rows:
                    identity: tuple[str | None, str | None, str] = (
                        None if database is None else str(row[0]).lower(),
                        None if row[1] is None else str(row[1]).lower(),
                        str(row[2]).lower(),
                    )
                    result.setdefault(identity, []).append(
                        ColumnInfo(
                            name=logical_column_name(str(row[3])),
                            type=self._build_information_schema_type(
                                data_type=str(row[4]),
                                numeric_precision=row[5],
                                numeric_scale=row[6],
                                character_maximum_length=row[7],
                            ),
                        )
                    )
        return {identity: tuple(columns) for identity, columns in result.items()}

    def _get_columns_for_relations_bulk_chunk(
        self,
        *,
        connection: _SnowflakeConnection,
        database: str | None,
        relations: list[RelationInfo],
    ) -> list[tuple[Any, ...]]:
        scopes: dict[str | None, list[str]] = {}
        for relation in relations:
            scopes.setdefault(relation.schema, []).append(relation.name)
        clauses: list[str] = []
        params: list[str] = []
        for schema, names in sorted(scopes.items(), key=lambda item: item[0] or ""):
            name_placeholders: str = ", ".join(["%s"] * len(names))
            if schema is None:
                clauses.append(f"table_name IN ({name_placeholders})")
            else:
                clauses.append(f"(table_schema = %s AND table_name IN ({name_placeholders}))")
                params.append(self._information_schema_identifier(schema))
            params.extend(self._information_schema_identifier(name) for name in names)
        metadata_table: str = self._information_schema_relation(database=database, name="columns")
        query: str = (
            "SELECT table_catalog, table_schema, table_name, column_name, data_type, "
            "numeric_precision, numeric_scale, character_maximum_length "
            f"FROM {metadata_table} WHERE "
            + " OR ".join(clauses)
            + " ORDER BY table_catalog, table_schema, table_name, ordinal_position"
        )
        cursor: Any = connection.cursor()
        try:
            cursor.execute(query, tuple(params))
            return list(cursor.fetchall())
        finally:
            cursor.close()

    def close(self, connection: _SnowflakeConnection) -> None:
        """Close a Snowflake connection."""

        connection.close()

    def star_exclude_keyword(self) -> str:
        """Snowflake uses EXCLUDE for SELECT * EXCLUDE."""

        return "EXCLUDE"

    def default_schema(self) -> str | None:
        """Snowflake projects should usually provide schema explicitly."""

        return None

    def default_database(self) -> str | None:
        """Snowflake projects should usually provide database explicitly."""

        return None

    def render_qualified_name(
        self,
        *,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> str | None:
        """Render Snowflake relation names using generic dot qualification."""

        if database is not None and schema is not None:
            return f"{database}.{schema}.{name}"
        if schema is not None:
            return f"{schema}.{name}"
        return None

    def render_framework_type(self, type_name: FrameworkType) -> str:
        """Render Snowflake internal framework types explicitly."""

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
        """Render Snowflake source expression type-enforcement casts explicitly."""

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
        """Snowflake does not require aliases for derived table factors."""

        return False

    def render_set_difference_operator(self) -> str:
        """Render Snowflake set-difference operator explicitly."""

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

        return self._with_state_table_retention(
            create_sql=build_create_table_sql(
                database=database,
                schema=schema,
                render_qualified_name=self.render_qualified_name,
                render_framework_type=self.render_framework_type,
            ),
            database=database,
            schema=schema,
        )

    def render_table_function_call(self, *, target: str, call_suffix_sql: str) -> str:
        return f"TABLE({target}{call_suffix_sql})"

    def render_udf_call(self, *, target: str, call_suffix_sql: str) -> str:
        return f"{target}{call_suffix_sql}"

    def expression_inference_profile(self) -> ExpressionInferenceProfile:
        return ExpressionInferenceProfile(
            sql_analysis_dialect=self.sql_analysis_dialect(),
            function_nullability_rules={
                "IFF": conditional_result_nullability,
                "LOWER": first_arg_nullability,
                "UPPER": first_arg_nullability,
            },
            function_return_types={
                "DATE_FROM_PARTS": "DATE",
                "LISTAGG": "TEXT",
                "LIST_AGG": "TEXT",
                "OBJECT_AGG": "OBJECT",
                "OBJECT_CONSTRUCT": "OBJECT",
                "REGEXP_REPLACE": "TEXT",
                "REGEXP_SUBSTR": "TEXT",
                "REPLACE": "TEXT",
                "SPLIT_PART": "TEXT",
                "STRING_AGG": "TEXT",
                "TO_ARRAY": "ARRAY",
                "TO_DATE": "DATE",
                "TO_JSON": "TEXT",
                "TO_OBJECT": "OBJECT",
                "TO_TIMESTAMP": "TIMESTAMP",
                "TO_TIMESTAMP_LTZ": "TIMESTAMP_LTZ",
                "TO_TIMESTAMP_NTZ": "TIMESTAMP_NTZ",
                "TO_TIMESTAMP_TZ": "TIMESTAMP_TZ",
                "TO_VARIANT": "VARIANT",
                "TO_VARCHAR": "TEXT",
                "TRIM": "TEXT",
                "TRY_TO_DATE": "DATE",
                "TRY_TO_DOUBLE": "FLOAT",
                "TRY_TO_TIMESTAMP": "TIMESTAMP",
                "TRY_TO_TIMESTAMP_LTZ": "TIMESTAMP_LTZ",
                "TRY_TO_TIMESTAMP_NTZ": "TIMESTAMP_NTZ",
                "TRY_TO_TIMESTAMP_TZ": "TIMESTAMP_TZ",
            },
        )

    def supports_table_functions(self) -> bool:
        return True

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
                raise AdapterUserError(message="Snowflake table functions must use SQL language")
            column_sql: str = ", ".join(f"{column.name} {column.type}" for column in return_columns)
            del returns, runtime_version, entry_point, packages
            return (
                f"CREATE OR REPLACE FUNCTION {destination}({argument_sql})\n"
                f"RETURNS TABLE ({column_sql})\n"
                f"AS $$\n{body_sql}\n$$",
            )
        if language == FunctionLanguage.PYTHON:
            if runtime_version is None or entry_point is None:
                raise AdapterUserError(
                    message="Snowflake Python UDFs require runtime_version and entry_point"
                )
            package_clause: str = ""
            if packages:
                package_values: str = "','".join(packages)
                package_clause = f"PACKAGES = ('{package_values}')\n"
            return (
                f"CREATE OR REPLACE FUNCTION {destination}({argument_sql})\n"
                f"RETURNS {returns}\n"
                "LANGUAGE PYTHON\n"
                f"RUNTIME_VERSION = '{runtime_version}'\n"
                f"HANDLER = '{entry_point}'\n"
                f"{package_clause}"
                f"AS $$\n{body_sql}\n$$",
            )
        return (
            f"CREATE OR REPLACE FUNCTION {destination}({argument_sql})\n"
            f"RETURNS {returns}\n"
            "LANGUAGE SQL\n"
            f"AS $$\n{body_sql}\n$$",
        )

    def render_create_temporary_function(
        self, *, name: str, definition: FunctionDefinition
    ) -> TemporaryFunction | None:
        """Copy a function into a session TEMPORARY function in the session's current schema."""

        target: str = self.render_identifier(f"{TEMPORARY_FUNCTION_PREFIX}{name}")
        statements: tuple[str, ...] = self.render_create_function(
            destination=target,
            arguments=definition.arguments,
            returns=definition.returns,
            body_sql=definition.body_sql,
            return_columns=definition.return_columns,
            language=definition.language,
            runtime_version=definition.runtime_version,
            entry_point=definition.entry_point,
            packages=definition.packages,
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

    def render_cursor_bound_literal(self, *, value: str, cursor_type: str | None) -> str:
        if cursor_type == CursorKind.INTEGER:
            return value
        if cursor_type == CursorKind.TIMESTAMP:
            escaped_value: str = value.replace("'", "''")
            return f"'{escaped_value}'"
        return f"'{value}'"

    def interrupt_connection(self, connection: Any) -> bool:
        """Abort the connection's running statements through Snowflake's cancel request."""

        connection.active_statements.cancel(raw_connection=connection.raw_connection)
        return True

    def create_sql_test_output(self, *, connection: Any, sql: str, name: str) -> str | None:
        """Run the output query once and read its persisted result through RESULT_SCAN."""

        del name
        cursor: Any = self.execute(connection=connection, sql=sql)
        query_id: object | None = getattr(cursor, "sfqid", None)
        if not isinstance(query_id, str) or not query_id:
            return None
        return f"TABLE(RESULT_SCAN('{query_id}'))"

    def drop_sql_test_output(self, *, connection: Any, relation: str) -> None:
        """Persisted query results expire on their own; there is nothing to drop."""

        del connection, relation

    def lists_relations_authoritatively(self, *, database: str | None, schema: str) -> bool:
        """Return whether ``list_relations`` reports every relation in this scope."""

        del database, schema
        return True

    def supports_zero_copy_clone(self) -> bool:
        return True

    def supports_durable_clone(self) -> bool:
        return True

    def render_drop(self, *, destination: str, if_exists: bool = True) -> tuple[str, ...]:
        exists_clause: str = " IF EXISTS" if if_exists else ""
        return (
            f"DROP TABLE{exists_clause} {destination}",
            f"DROP VIEW{exists_clause} {destination}",
        )

    def render_rename(self, *, origin: str, destination: str) -> tuple[str, ...]:
        return (f"ALTER TABLE {origin} RENAME TO {destination}",)

    def render_rename_view(self, *, origin: str, destination: str) -> tuple[str, ...]:
        return (f"ALTER VIEW {origin} RENAME TO {destination}",)

    def render_swap(self, *, left: str, right: str) -> tuple[str, ...]:
        return (f"ALTER TABLE {left} SWAP WITH {right}",)

    def render_clone(
        self,
        *,
        origin: str,
        destination: str,
        hard_copy: bool = False,
        origin_is_transient: bool = False,
    ) -> tuple[str, ...]:
        if hard_copy:
            return self.render_create_table_as(
                destination=destination, sql=f"SELECT * FROM {origin}"
            )
        table_kind: str = "TRANSIENT TABLE" if origin_is_transient else "TABLE"
        return (f"CREATE OR REPLACE {table_kind} {destination} CLONE {origin}",)

    def render_durable_clone(
        self, *, origin: str, destination: str, origin_is_transient: bool = False
    ) -> tuple[str, ...]:
        table_kind: str = "TRANSIENT TABLE" if origin_is_transient else "TABLE"
        return (f"CREATE OR REPLACE {table_kind} {destination} CLONE {origin}",)

    def render_migration_stage(
        self,
        *,
        origin: str,
        stage: str,
        origin_is_transient: bool = False,
        stage_is_transient: bool | None = None,
    ) -> MigrationStagePlan:
        transient: bool = origin_is_transient if stage_is_transient is None else stage_is_transient
        if origin_is_transient and not transient:
            return MigrationStagePlan(
                transfer=MigrationTransfer.COPY,
                statements=(
                    f"CREATE TABLE {stage} LIKE {origin} COPY GRANTS",
                    f"INSERT INTO {stage} SELECT * FROM {origin}",
                ),
            )
        table_kind: str = "TRANSIENT TABLE" if transient else "TABLE"
        return MigrationStagePlan(
            transfer=MigrationTransfer.CLONE,
            statements=(f"CREATE {table_kind} {stage} CLONE {origin} COPY GRANTS",),
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
        return snowflake_relation_grants(rows)

    def render_relation_grants(
        self, *, grants: tuple[RelationGrant, ...], destination: str, columns: tuple[str, ...]
    ) -> tuple[str, ...]:
        del columns
        return render_snowflake_view_grants(grants=grants, destination=destination)

    def render_replace_view_keeping_grants(
        self, *, destination: str, sql: str
    ) -> tuple[str, ...] | None:
        return (f"CREATE OR REPLACE VIEW {destination} COPY GRANTS AS {sql}",)

    def read_view_definition(
        self, *, connection: Any, database: str | None, schema: str, name: str
    ) -> str | None:
        prefetched: tuple[bool, str | None] = cached_view_definition(
            adapter=self, connection=connection, database=database, schema=schema, name=name
        )
        if prefetched[0]:
            return prefetched[1]
        views: str = (
            "INFORMATION_SCHEMA.VIEWS"
            if database is None
            else (f"{database}.INFORMATION_SCHEMA.VIEWS")
        )
        rows: list[tuple[Any, ...]] = self.execute(
            connection=connection,
            sql=(
                f"SELECT VIEW_DEFINITION FROM {views} WHERE UPPER(TABLE_SCHEMA) = UPPER("
                + "'"
                + schema.replace("'", "''")
                + "'"
                + ") AND UPPER(TABLE_NAME) = UPPER("
                + "'"
                + name.replace("'", "''")
                + "'"
                + ")"
            ),
        ).fetchall()
        return None if not rows else str(rows[0][0])

    def read_view_definitions(
        self, *, connection: Any, database: str | None, schema: str, names: tuple[str, ...]
    ) -> dict[str, str]:
        """Read several views' definitions in one schema, matching names case-insensitively."""

        if not names:
            return {}
        views: str = (
            "INFORMATION_SCHEMA.VIEWS"
            if database is None
            else (f"{database}.INFORMATION_SCHEMA.VIEWS")
        )
        literals: str = ", ".join("UPPER('" + name.replace("'", "''") + "')" for name in names)
        rows: list[tuple[Any, ...]] = self.execute(
            connection=connection,
            sql=(
                f"SELECT TABLE_NAME, VIEW_DEFINITION FROM {views} WHERE UPPER(TABLE_SCHEMA) = "
                "UPPER('" + schema.replace("'", "''") + "') "
                f"AND UPPER(TABLE_NAME) IN ({literals})"
            ),
        ).fetchall()
        found: dict[str, str] = {}
        row: tuple[Any, ...]
        for row in rows:
            found.setdefault(str(row[0]).upper(), str(row[1]))
        return {name: found[name.upper()] for name in names if name.upper() in found}

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
        return render_snowflake_view_revokes(grants=grants, destination=destination)

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
        del infer_types
        if replace:
            self.drop(
                connection=connection,
                destination=destination,
                if_exists=True,
                statement_recorder=statement_recorder,
            )
        column_defs: str = ", ".join(f"{col.name} {col.type}" for col in columns)
        create_sql: str = f"CREATE TRANSIENT TABLE {destination} ({column_defs})"
        statement_recorder.record(create_sql)
        self.execute(connection=connection, sql=create_sql)

        column_names: tuple[str, ...] = tuple(column.name for column in columns)
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
        """Return Snowflake query column names using cursor description."""

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
        """Read ``ROW_COUNT`` from the information schema; views report no count."""

        row: tuple[Any, ...] | None = self._information_schema_table_row(
            connection=connection,
            database=database,
            schema=schema,
            name=name,
            projection="row_count,bytes" if metadata_only else "row_count",
        )
        if row is None:
            return RelationRowCountEstimate(row_count=None, detail="relation not found")
        byte_count: int | None = int(row[1]) if metadata_only and row[1] is not None else None
        return RelationRowCountEstimate(
            row_count=int(row[0]) if row[0] is not None else None,
            detail="no row count (view)" if row[0] is None else None,
            byte_count=byte_count,
            byte_count_basis="information_schema_bytes" if byte_count is not None else None,
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
        samples: list[tuple[tuple[str, object], ...]] = []
        for row in rows:
            sample: list[tuple[str, object]] = []
            for index, key in enumerate(keys):
                sample.append((key, row[index]))
            samples.append(tuple(sample))
        return tuple(samples)

    def describe_relation(self, *, connection: Any, relation: str) -> tuple[ColumnInfo, ...]:
        """Return Snowflake relation metadata using DESCRIBE TABLE."""

        cursor: Any = connection.cursor()
        try:
            cursor.execute(f"DESCRIBE TABLE {relation}")
            rows: list[tuple[Any, ...]] = cursor.fetchall()
        finally:
            cursor.close()
        return tuple(
            ColumnInfo(name=logical_column_name(str(row[0])), type=str(row[1])) for row in rows
        )

    def build_cursor_filter(
        self,
        *,
        cursor_column: str | None,
        start_cursor: Any | None,
        end_cursor: Any | None,
    ) -> str:
        """Build a Snowflake cursor filter clause."""

        if cursor_column is None or start_cursor is None:
            return ""
        clauses: list[str] = [f"{cursor_column} >= '{start_cursor.value}'"]
        if end_cursor is not None:
            clauses.append(f"{cursor_column} < '{end_cursor.value}'")
        return " AND ".join(clauses)

    def _build_information_schema_type(
        self,
        *,
        data_type: str,
        numeric_precision: object,
        numeric_scale: object,
        character_maximum_length: object,
    ) -> str:
        return information_schema_type(
            data_type=data_type,
            numeric_precision=numeric_precision,
            numeric_scale=numeric_scale,
            character_maximum_length=character_maximum_length,
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
        return normalize_numeric_family(type_sql=column_type, dialect=self.sql_analysis_dialect())

    def format_row_diff_decimal_sql(self, value: Decimal) -> str:
        return format(value, "f")

    def _initialize_session(
        self,
        *,
        connection: _SnowflakeConnection,
        secondary_roles: str | None,
        warehouse: object | None,
        database: object | None,
        schema: object | None,
    ) -> None:
        statements: list[str] = []
        if secondary_roles is not None:
            statements.append(f"USE SECONDARY ROLES {secondary_roles}")
        normalized_warehouse: str | None = self._normalize_session_value(warehouse)
        normalized_database: str | None = self._normalize_session_value(database)
        normalized_schema: str | None = self._normalize_session_value(schema)
        if normalized_warehouse is not None:
            statements.append(f"USE WAREHOUSE {normalized_warehouse}")
        if normalized_database is not None:
            statements.append(f"USE DATABASE {normalized_database}")
        if normalized_schema is not None and self.schema_exists(
            connection=connection,
            database=normalized_database,
            schema=normalized_schema,
        ):
            statements.append(f"USE SCHEMA {normalized_schema}")
        statement: str
        for statement in statements:
            self.execute(connection=connection, sql=statement)

    @staticmethod
    def _normalize_secondary_roles(value: object | None) -> str | None:
        if value is None:
            return None
        normalized: str = str(value).strip().upper()
        if normalized not in {SECONDARY_ROLES_ALL, SECONDARY_ROLES_NONE}:
            raise AdapterUserError(
                message="Snowflake connection secondary_roles must be 'ALL' or 'NONE'",
                code="A301",
            )
        return normalized

    @staticmethod
    def _normalize_session_value(value: object | None) -> str | None:
        if not isinstance(value, str):
            return None
        stripped: str = value.strip()
        if not stripped or stripped.startswith("<none"):
            return None
        return stripped

    def schema_exists(
        self,
        *,
        connection: _SnowflakeConnection,
        database: str | None,
        schema: str,
    ) -> bool:
        query: str = (
            "SELECT 1 FROM information_schema.schemata WHERE UPPER(schema_name) = UPPER(%s)"
        )
        params: list[str] = [schema]
        if database is not None:
            query += " AND UPPER(catalog_name) = UPPER(%s)"
            params.append(database)
        cursor: Any = connection.cursor()
        try:
            cursor.execute(query, tuple(params))
            return cursor.fetchone() is not None
        finally:
            cursor.close()
