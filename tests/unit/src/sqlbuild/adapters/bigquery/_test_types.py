from collections.abc import Callable
from dataclasses import dataclass
from datetime import datetime
from decimal import Decimal

from sqlbuild._native import InferredNullability
from sqlbuild.adapter.contract.models import (
    ColumnInfo,
    RelationUsageReport,
    RowDiffResult,
    RowDiffSampleRow,
    SchemaDiffResult,
)
from sqlbuild.adapter.contract.types import LoaderLogicalType
from sqlbuild.adapters.bigquery.classes.bigquery_adapter import BigQueryAdapter


@dataclass(frozen=True)
class BigQueryRetentionTestCase:
    description: str
    desired_days: int
    expected_hours: int
    expected_sql: str


@dataclass(frozen=True)
class BigQueryInvalidRetentionTestCase:
    description: str
    desired_days: int
    expected_error_fragment: str


@dataclass(frozen=True)
class BigQueryMissingRetentionDatasetTestCase:
    description: str
    desired_days: int
    expected_effective_days: int
    expected_exists: bool


@dataclass(frozen=True)
class BigQueryMergeExclusionTestCase:
    description: str
    expected_update_assignment: str
    expected_insert_clause: str


@dataclass(frozen=True)
class BigQueryExpressionInferenceProfileTestCase:
    description: str
    expected_sql_analysis_dialect: str
    expected_identifier_limit: int
    expected_rule_results: dict[str, InferredNullability]


@dataclass(frozen=True)
class BigQueryQueryTestCase:
    description: str
    sql: str
    limit: int | None
    expected_columns: tuple[str, ...]
    expected_rows: tuple[tuple[object, ...], ...]
    expected_truncated: bool
    expected_row_count: int


@dataclass(frozen=True)
class BigQueryDmlLifecycleTestCase:
    description: str
    sql: str
    statement_type: str
    affected_rows: object | None
    expected_affected_rows: int | None


@dataclass(frozen=True)
class BigQueryRenderSchemaTestCase:
    description: str
    database: str | None
    schema: str
    location: str | None
    expected_sql: str


@dataclass(frozen=True)
class BigQueryRenderQualifiedNameTestCase:
    description: str
    database: str | None
    schema: str | None
    name: str
    expected_qualified_name: str | None


@dataclass(frozen=True)
class BigQueryRenderCloneTestCase:
    description: str
    source: str
    target: str
    hard_copy: bool
    expected_statements: tuple[str, ...]
    expected_supports_zero_copy: bool


@dataclass(frozen=True)
class BigQueryRenderDropViewTestCase:
    description: str
    target: str
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class BigQueryRenderPythonFunctionTestCase:
    description: str
    expected_sql: str


@dataclass(frozen=True)
class BigQueryRenderTableFunctionTestCase:
    description: str
    expected_sql: str


@dataclass(frozen=True)
class BigQuerySchemaExistsTestCase:
    description: str
    missing_dataset: bool
    expected_exists: bool


@dataclass(frozen=True)
class BigQueryConnectErrorTestCase:
    description: str
    config: dict[str, object]
    expected_error_fragment: str


@dataclass(frozen=True)
class BigQueryExecutionErrorTestCase:
    description: str
    error_message: str
    error_details: list[dict[str, object]]
    expected_error_fragment: str
    expected_error_code: str


@dataclass(frozen=True)
class BigQueryRenderCursorBoundLiteralTestCase:
    description: str
    value: str
    cursor_type: str | None
    expected_literal: str


@dataclass(frozen=True)
class BigQueryRenderDeleteInsertTestCase:
    description: str
    expected_fragments: tuple[str, ...]
    unexpected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class BigQuerySchemaDiffTestCase:
    description: str
    expected_result: SchemaDiffResult
    relation_columns: dict[str, tuple[ColumnInfo, ...]]


@dataclass(frozen=True)
class BigQueryRowDiffTestCase:
    description: str
    expected_result: RowDiffResult


@dataclass(frozen=True)
class BigQuerySampleRowsTestCase:
    description: str
    expected_unequal_samples: tuple[RowDiffSampleRow, ...]
    expected_side_only_samples: tuple[tuple[tuple[str, object], ...], ...]


@dataclass(frozen=True)
class BigQueryPruneSqlTestCase:
    description: str
    database: str | None
    schema: str
    retain_versions: int
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class BigQueryTableFreshnessBatchTestCase:
    description: str
    location: str
    expected_data_versions: tuple[datetime, ...]
    expected_query_fragments: tuple[str, ...]


@dataclass(frozen=True)
class BigQueryTableFreshnessWildcardTestCase:
    description: str
    table_name: str
    expected_error_fragment: str


@dataclass(frozen=True)
class BigQueryRelationAgeMetadataTestCase:
    description: str
    metadata_rows: tuple[tuple[object, ...], ...]
    expected_timestamps: tuple[tuple[str, str], ...]
    relation_names: tuple[str, ...] = ("old_orders", "old_orders_view")


@dataclass(frozen=True)
class BigQueryCloneRefusalTestCase:
    description: str
    driver_error: Exception
    expected_refusal: bool


@dataclass(frozen=True)
class BigQueryJobInvalidationTestCase:
    """A BigQuery job that changes relations without a SQL statement."""

    description: str
    operation: str
    expected_reads: dict[str, int]


@dataclass(frozen=True)
class BigQueryRelationUsageTestCase:
    description: str
    relations: tuple[tuple[str | None, str, str], ...]
    dataset_locations: dict[str, str]
    rows: tuple[tuple[object, ...], ...]
    expected_queries: tuple[tuple[str, str, tuple[str, ...]], ...]
    expected_usages: tuple[tuple[str, tuple[str, ...], int, datetime], ...]


@dataclass(frozen=True)
class BigQueryRelationUsageQueryTestCase:
    description: str
    window_seconds: int
    expected_params: dict[str, object]
    expected_sql_fragments: tuple[str, ...]


@dataclass(frozen=True)
class BigQueryRelationUsageChunkTestCase:
    description: str
    relation_count: int
    expected_query_names: tuple[int, ...]


@dataclass(frozen=True)
class BigQueryRelationUsageErrorTestCase:
    description: str
    error: Exception
    expected_report: RelationUsageReport


@dataclass(frozen=True)
class BigQueryRelationUsageFailureTestCase:
    description: str
    error: Exception
    expected_error_match: str


@dataclass(frozen=True)
class BigQueryCloneStatementTestCase:
    description: str
    statement: str
    expected_clone: bool


@dataclass(frozen=True)
class BigQueryStringLiteralTestCase:
    description: str
    value: str
    expected_literal: str


@dataclass(frozen=True)
class BigQueryGoogleSqlRenderingTestCase:
    description: str
    render: Callable[[BigQueryAdapter], str | tuple[str, ...]]
    expected_sql: str | tuple[str, ...]


@dataclass(frozen=True)
class BigQueryKeyedCursorMergeTestCase:
    description: str
    unique_key: tuple[str, ...]
    columns: tuple[str, ...]
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class BigQueryLoaderDecimalRowTestCase:
    description: str
    value: Decimal
    column_type: str
    expected_sql: str
    inferred_types: tuple[tuple[str, LoaderLogicalType], ...] = (
        ("amount", LoaderLogicalType.DECIMAL),
    )
