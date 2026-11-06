from dataclasses import dataclass
from datetime import datetime

from sqlbuild._native import InferredNullability
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.models import FunctionInfo, RelationUsageReport, SchemaDiffResult
from sqlbuild.adapter.contract.types import TableFreshnessStatus
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.cost.types import CostStatus
from tests.unit.src.sqlbuild.adapters.snowflake.inspection.helpers import FakeRelation


@dataclass(frozen=True)
class SnowflakeRetentionTestCase:
    description: str
    desired_days: int
    relation: FakeRelation
    expected_days: int
    expected_kind: str
    expected_sql: str


@dataclass(frozen=True)
class SnowflakeShowRetentionTestCase:
    description: str
    relations: tuple[FakeRelation, ...]
    steps: tuple[tuple[str, str, str], ...]
    in_build: bool
    expected_steps: tuple[tuple[int | None, tuple[str, ...]], ...]


@dataclass(frozen=True)
class SnowflakeTableTypeDdlTestCase:
    description: str
    table_type: str
    expected_prefix: str


@dataclass(frozen=True)
class SnowflakeCostCollectionTestCase:
    description: str
    connect_error: bool
    collection_error: bool
    close_error: bool
    expected_status: CostStatus
    expected_message_fragment: str | None = None
    expected_limitation_fragment: str | None = None


@dataclass(frozen=True)
class SnowflakeCostPurposeTestCase:
    description: str
    expected_purposes: list[str | None]


@dataclass(frozen=True)
class SnowflakeMergeExclusionTestCase:
    description: str
    expected_update_assignment: str
    expected_insert_clause: str


@dataclass(frozen=True)
class SnowflakeExpressionInferenceProfileTestCase:
    description: str
    expected_sql_analysis_dialect: str
    expected_identifier_limit: int
    expected_rule_results: dict[str, InferredNullability]
    expected_return_types: dict[str, str]


@dataclass(frozen=True)
class SnowflakeRenderCursorBoundLiteralTestCase:
    description: str
    value: str
    cursor_type: str | None
    expected_literal: str


@dataclass(frozen=True)
class SnowflakeRenderCloneTestCase:
    description: str
    source: str
    target: str
    hard_copy: bool
    origin_is_transient: bool
    expected_statements: tuple[str, ...]
    expected_supports_zero_copy: bool


@dataclass(frozen=True)
class SnowflakeMoveOrCopyRelationTestCase:
    description: str
    source: str
    target: str
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class SnowflakeRenderIdentifierTestCase:
    description: str
    name: str
    expected_identifier: str


@dataclass(frozen=True)
class SnowflakeSchemaDiffTestCase:
    description: str
    expected_result: SchemaDiffResult


@dataclass(frozen=True)
class SnowflakeRenderPythonFunctionTestCase:
    description: str
    expected_sql: str


@dataclass(frozen=True)
class SnowflakeRenderTableFunctionTestCase:
    description: str
    expected_sql: str


@dataclass(frozen=True)
class SnowflakeQueryColumnTypesTestCase:
    description: str
    cursor_description: tuple[tuple[object, ...], ...]
    expected_types: tuple[str | None, ...]


@dataclass(frozen=True)
class SnowflakeQueryColumnNamesTestCase:
    description: str
    cursor_description: tuple[tuple[str], ...]
    expected_columns: tuple[str, ...]


@dataclass(frozen=True)
class SnowflakeLoadSeedTestCase:
    description: str
    csv_text: str
    expected_rows: list[tuple[object, ...]]


@dataclass(frozen=True)
class SnowflakeTableFreshnessMetadataTestCase:
    description: str
    row: tuple[object, ...]
    expected_data_version: datetime
    expected_value_kind: str
    expected_supports_metadata: bool


@dataclass(frozen=True)
class SnowflakeTableFreshnessBatchTestCase:
    description: str
    expected_data_versions: tuple[datetime, ...]
    expected_query_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SnowflakeTableFreshnessOutcomeTestCase:
    description: str
    rows: tuple[tuple[object, ...], ...]
    expected_statuses: dict[str, TableFreshnessStatus]
    expected_message_fragments: dict[str, str]


@dataclass(frozen=True)
class SnowflakeTableFreshnessMetadataErrorTestCase:
    description: str
    row: tuple[object, ...] | None
    expected_error_fragment: str


@dataclass(frozen=True)
class SnowflakePruneSqlTestCase:
    description: str
    database: str | None
    schema: str
    retain_versions: int
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SnowflakeConnectConfigTestCase:
    description: str
    config: dict[str, object]
    expected_connect_kwargs: dict[str, object]
    expected_session_statements: tuple[str, ...] = ()


@dataclass(frozen=True)
class SnowflakeInvalidSecondaryRolesTestCase:
    description: str
    secondary_roles: str
    expected_error: str


@dataclass(frozen=True)
class SnowflakeInformationSchemaFilterTestCase:
    description: str
    database: str
    schemas: tuple[str, ...]
    names: tuple[str, ...]
    expected_params: tuple[str, ...]


@dataclass(frozen=True)
class SnowflakeQualifiedColumnInspectionTestCase:
    description: str
    relation_count: int
    expected_statement_count: int


@dataclass(frozen=True)
class SnowflakeBatchedRetentionTestCase:
    description: str
    requests: tuple[tuple[str, str], ...]
    relations: tuple[FakeRelation, ...]
    expected_queries: tuple[str, ...]
    expected_days: dict[str, int]


@dataclass(frozen=True)
class SnowflakeBatchedRetentionErrorTestCase:
    description: str
    requests: tuple[tuple[str, str], ...]
    relations: tuple[FakeRelation, ...]
    expected_error: str


@dataclass(frozen=True)
class SnowflakeRelationAgeMetadataTestCase:
    description: str
    relation_type: str
    created: datetime | None
    last_altered: datetime | None
    expected_created_at: datetime | None
    expected_last_altered_at: datetime | None


@dataclass(frozen=True)
class SnowflakeFunctionDiscoveryTestCase:
    description: str
    database: str | None
    schemas: tuple[str, ...] | None
    names: tuple[str, ...] | None
    rows: tuple[tuple[object, ...], ...]
    expected_relation: str
    expected_params: tuple[str, ...]
    expected_functions: tuple[FunctionInfo, ...]


@dataclass(frozen=True)
class SnowflakeStateTableDdlTestCase:
    description: str
    render_method: str
    expected_prefix: str
    expected_suffix: str


@dataclass(frozen=True)
class SnowflakeStateTableExactDdlTestCase:
    description: str
    render_method: str
    expected_sql: str


@dataclass(frozen=True)
class SnowflakeStateTableFallbackTestCase:
    description: str
    adapter_type: type[SnowflakeAdapter]
    rejection_errno: int
    rejection_message: str
    expected_executed_tables: tuple[str, ...]
    expected_executed_retention_days: tuple[str, ...]


@dataclass(frozen=True)
class SnowflakeStateTableRetentionErrorTestCase:
    description: str
    errno: int
    message: str
    expected_executed_count: int
    expected_next_retention_days: str


@dataclass(frozen=True)
class SnowflakeNonStateRetentionErrorTestCase:
    description: str
    sql: str
    expected_executed_sql: tuple[str, ...]


@dataclass(frozen=True)
class SnowflakeStateTableFallbackLifecycleTestCase:
    description: str
    rejections: tuple[tuple[tuple[str, ...], Exception], ...]
    expected_statement_events: tuple[str, ...]
    expected_progress_fail_lines: int
    expected_executed_retention_days: tuple[str, ...]


@dataclass(frozen=True)
class SnowflakeStateTableSchemaFallbackTestCase:
    description: str
    rejecting_schema: str
    accepting_schema: str
    expected_executed: tuple[tuple[str, str], ...]


@dataclass(frozen=True)
class SnowflakeViewDefinitionsTestCase:
    description: str
    stored_rows: tuple[tuple[str, str], ...]
    prefetched_names: tuple[str, ...]
    read_names: tuple[str, ...]
    expected_definitions: tuple[str | None, ...]
    expected_query_count: int


@dataclass(frozen=True)
class SnowflakeRelationUsageTestCase:
    description: str
    relations: tuple[tuple[str | None, str, str], ...]
    current_database: str | None
    access_rows: tuple[tuple[object, ...], ...]
    expected_object_names: tuple[str, ...]
    expected_usages: tuple[tuple[str, tuple[str, ...], int, datetime], ...]


@dataclass(frozen=True)
class SnowflakeRelationUsageQueryTestCase:
    description: str
    window_seconds: int
    expected_params: tuple[object, ...]
    expected_sql_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SnowflakeRelationUsageChunkTestCase:
    description: str
    relation_count: int
    expected_query_names: tuple[int, ...]


@dataclass(frozen=True)
class SnowflakeRelationUsageErrorTestCase:
    description: str
    errno: int | None
    message: str
    expected_report: RelationUsageReport


@dataclass(frozen=True)
class SnowflakeRelationUsageFailureTestCase:
    description: str
    errno: int | None
    message: str
    expected_error_match: str


@dataclass(frozen=True)
class UnsupportedRelationUsageTestCase:
    description: str
    adapter: BaseAdapter
    expected_report: RelationUsageReport


@dataclass(frozen=True)
class SnowflakeRelationUsageViaTestCase:
    description: str
    relations: tuple[tuple[str, str, str, str], ...]
    access_rows: tuple[tuple[object, ...], ...]
    dependency_rows: tuple[tuple[str, str], ...]
    expected_dependency_params: tuple[tuple[object, ...], ...]
    expected_usages: tuple[tuple[str, tuple[str, ...], int, bool, tuple[str, ...]], ...]


@dataclass(frozen=True)
class SnowflakeCloneStatementTestCase:
    description: str
    statement: str
    expected_clone: bool


@dataclass(frozen=True)
class SnowflakeRelationUsageChainsUnfollowedTestCase:
    description: str
    errno: int | None
    message: str
    expected_usages: tuple[tuple[str, tuple[str, ...], int], ...]
    expected_notices: tuple[str, ...]


@dataclass(frozen=True)
class SnowflakePrefixListingTestCase:
    description: str
    relations: tuple[FakeRelation, ...]
    schema: str
    prefix: str
    expected_relations: tuple[tuple[str, str], ...]
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class SnowflakeRunListingTestCase:
    description: str
    relations: tuple[FakeRelation, ...]
    lookups: tuple[str, ...]
    expected_listings: tuple[int, int]


@dataclass(frozen=True)
class SnowflakePlatformDetectionTestCase:
    description: str
    environment: dict[str, str]
    expected_value: str
