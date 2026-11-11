from dataclasses import dataclass

from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.strict_adapter import StrictAdapter
from sqlbuild.adapter.contract.models import RelationGrant
from sqlbuild.adapter.contract.types import MigrationTransfer


@dataclass(frozen=True)
class AdapterDurableCloneTestCase:
    description: str
    adapter: StrictAdapter
    source: str
    target: str
    expected_statements: tuple[str, ...]
    expected_supports_durable_clone: bool


@dataclass(frozen=True)
class AdapterCloneModeTestCase:
    description: str
    adapter: StrictAdapter
    source: str
    target: str
    hard_copy: bool
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class AdapterEligibleMaxCursorSqlTestCase:
    description: str
    adapter: StrictAdapter
    cursor_column: str
    maximum_allowed: str
    is_date: bool
    expected_sql: str


@dataclass(frozen=True)
class AdapterManagedWriteSchemaCapabilityTestCase:
    description: str
    adapter: BaseAdapter
    expected_allows_implicit_schema: bool


@dataclass(frozen=True)
class RowDiffSampleSqlTestCase:
    description: str
    adapter: BaseAdapter
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class AdapterRelationAgeMetadataCapabilityTestCase:
    description: str
    adapter: StrictAdapter
    expected_supported: bool


@dataclass(frozen=True)
class AdapterMigrationStageTestCase:
    description: str
    adapter: BaseAdapter
    origin_is_transient: bool
    stage_is_transient: bool | None
    expected_transfer: MigrationTransfer
    expected_statements: tuple[str, ...]
    expected_fallback_statements: tuple[str, ...] = ()


@dataclass(frozen=True)
class AdapterTransactionalDdlTestCase:
    description: str
    adapter: BaseAdapter
    expected_transactional: bool


@dataclass(frozen=True)
class AdapterRenameColumnSqlTestCase:
    description: str
    adapter: StrictAdapter
    destination: str
    old_name: str
    new_name: str
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class AdapterColumnMigrationStateTableTestCase:
    description: str
    adapter: StrictAdapter
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class DatabricksColumnMappingTestCase:
    description: str
    property_rows: list[tuple[str, str]]
    expected_available: bool


@dataclass(frozen=True)
class RelationGrantCaptureTestCase:
    description: str
    adapter: BaseAdapter
    relation_type: str
    answers: tuple[list[tuple[object, ...]], ...]
    destination: str
    columns: tuple[str, ...]
    expected_query_fragment: str
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class ViewReplaceTestCase:
    description: str
    adapter: BaseAdapter
    expected_statements: tuple[str, ...] | None


@dataclass(frozen=True)
class RelationRevokeTestCase:
    description: str
    adapter: BaseAdapter
    grants: tuple[RelationGrant, ...]
    destination: str
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class AdapterStateTableDdlTestCase:
    description: str
    adapter: StrictAdapter
    render_method: str
    expected_create_fragment: str


@dataclass(frozen=True)
class RelationRowCountEstimateTestCase:
    description: str
    adapter: BaseAdapter
    metadata_rows: tuple[tuple[object, ...], ...]
    expected_row_count: int | None
    expected_detail: str | None
    expected_sql_fragment: str
    database: str | None = "analytics"
    schema: str | None = "dev"
    metadata_only: bool = False
    expected_byte_count: int | None = None
    expected_byte_count_basis: str | None = None
    metadata_columns: tuple[str, ...] = ()
    name: str = "orders"


@dataclass(frozen=True)
class BigQueryRowCountEstimateTestCase:
    description: str
    table_type: str
    num_rows: int | None
    expected_row_count: int | None
    expected_table_id: str = "proj.dev.orders"
    metadata_only: bool = False
    num_bytes: int | None = None
    expected_byte_count: int | None = None


@dataclass(frozen=True)
class DuckDbRowCountEstimateTestCase:
    description: str
    setup_sql: tuple[str, ...]
    schema: str | None
    name: str
    expected_row_count: int | None
    expected_detail: str | None = None


@dataclass(frozen=True)
class StatementCancellationTestCase:
    description: str
    expected_limit_range: tuple[int, int]
    expected_asynchronous: bool


@dataclass(frozen=True)
class BigQueryQueuedJobTestCase:
    """One BigQuery job that waits in a queue before it runs."""

    description: str
    pending_polls: int
    poll_seconds: float
    expected_maximum_running_seconds: float


@dataclass(frozen=True)
class AdapterCursorLiteralEscapingTestCase:
    description: str
    adapter: StrictAdapter
    expected_literal: str
    expected_delete_bound: str
