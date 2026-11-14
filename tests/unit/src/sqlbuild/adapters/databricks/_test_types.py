from dataclasses import dataclass
from datetime import datetime

from sqlbuild._native import InferredNullability
from sqlbuild.adapter.contract.models import RelationInfo, RelationUsageReport
from tests.unit.src.sqlbuild.adapters.databricks.helpers import FakeDatabricksMetadataCursor


@dataclass(frozen=True)
class DatabricksRetentionTestCase:
    description: str
    desired_days: int
    observed_row: tuple[object, ...]
    expected_effective_days: int
    expected_log_days: int
    expected_deleted_days: int
    expected_sql: str


@dataclass(frozen=True)
class DatabricksInvalidRetentionTestCase:
    description: str
    observed_row: tuple[object, ...]
    expected_error_fragment: str


@dataclass(frozen=True)
class DatabricksRetentionOrderingTestCase:
    description: str
    desired_days: int
    log_days: int
    deleted_days: int
    expected_phases: tuple[str, ...]
    expected_property_order: tuple[str, ...]


@dataclass(frozen=True)
class DatabricksMergeExclusionTestCase:
    description: str
    expected_update_assignment: str
    expected_insert_clause: str


@dataclass(frozen=True)
class DatabricksExpressionInferenceProfileTestCase:
    description: str
    expected_sql_analysis_dialect: str
    expected_identifier_limit: int
    expected_rule_results: dict[str, InferredNullability]


@dataclass(frozen=True)
class DatabricksRenderDeleteInsertCursorTestCase:
    description: str
    target: str
    sql: str
    cursor_column: str
    cursor_start: str
    cursor_end: str
    columns: tuple[str, ...] | None
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class DatabricksRenderCloneTestCase:
    description: str
    source: str
    target: str
    hard_copy: bool
    expected_statements: tuple[str, ...]
    expected_supports_zero_copy: bool


@dataclass(frozen=True)
class DatabricksRenderDurableCloneTestCase:
    description: str
    source: str
    target: str
    expected_statements: tuple[str, ...]
    expected_supports_durable_clone: bool


@dataclass(frozen=True)
class DatabricksRenderPythonFunctionTestCase:
    description: str
    body_sql: str
    packages: tuple[str, ...]
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class DatabricksRenderTableFunctionTestCase:
    description: str
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class DatabricksPythonFunctionSupportTestCase:
    description: str
    expected_supports_python_functions: bool


@dataclass(frozen=True)
class DatabricksStringTypeCastRenderingTestCase:
    description: str
    declared_type: str
    expected_loader_fragment: str
    expected_source_cast: str


@dataclass(frozen=True)
class DatabricksPruneSqlTestCase:
    description: str
    database: str | None
    schema: str
    retain_versions: int
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class DatabricksTableFreshnessBatchTestCase:
    description: str
    expected_data_versions: tuple[datetime, ...]
    expected_query_fragments: tuple[str, ...]


@dataclass(frozen=True)
class DatabricksTableFreshnessMixedBatchTestCase:
    description: str
    delta_version: datetime
    view_error: str
    expected_view_message: str


@dataclass(frozen=True)
class DatabricksRelationAgeMetadataTestCase:
    description: str
    table_type: str
    created: datetime | None
    last_altered: datetime | None
    expected_relation_type: str
    expected_timestamps: tuple[str, str]


@dataclass(frozen=True)
class DatabricksDeltaDetailAgeTestCase:
    description: str
    relation: RelationInfo
    cursors: tuple[FakeDatabricksMetadataCursor, ...]
    expected_sql: tuple[str, ...]
    expected_timestamps: tuple[str, str]


@dataclass(frozen=True)
class DatabricksRelationUsageTestCase:
    description: str
    relations: tuple[tuple[str | None, str, str], ...]
    rows: tuple[tuple[object, ...], ...]
    expected_object_names: tuple[str, ...]
    expected_usages: tuple[tuple[str, tuple[str, ...], int, datetime], ...]
    expected_via: tuple[tuple[bool, tuple[str, ...]], ...] = ()


@dataclass(frozen=True)
class DatabricksRelationUsageQueryTestCase:
    description: str
    window_seconds: int
    expected_params: dict[str, object]
    expected_sql_fragments: tuple[str, ...]


@dataclass(frozen=True)
class DatabricksRelationUsageChunkTestCase:
    description: str
    relation_count: int
    expected_query_names: tuple[int, ...]


@dataclass(frozen=True)
class DatabricksRelationUsageErrorTestCase:
    description: str
    message: str
    sqlstate: str | None
    expected_report: RelationUsageReport


@dataclass(frozen=True)
class DatabricksRelationUsageFailureTestCase:
    description: str
    message: str
    sqlstate: str | None
    expected_error_match: str


@dataclass(frozen=True)
class DatabricksSchemaExistsTestCase:
    """A schema existence check with or without a catalog, and what it must query and answer."""

    description: str
    database: str | None
    rows: list[tuple[object, ...]]
    expected_exists: bool
    expected_query_fragment: str
