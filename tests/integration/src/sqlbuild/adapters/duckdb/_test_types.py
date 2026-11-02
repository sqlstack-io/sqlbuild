from contextlib import AbstractContextManager
from dataclasses import dataclass, field

from sqlbuild._native import InferredNullability, SeedCsvSettings
from sqlbuild.adapter.contract.models import (
    ColumnInfo,
    CursorValue,
    QueryResult,
    RowDiffResult,
    RowDiffTolerances,
    SchemaDiffResult,
)


@dataclass(frozen=True)
class TypedSqlRenderingExecutionTestCase:
    description: str
    expected_row: tuple[object, ...]


@dataclass(frozen=True)
class PhysicalRelationGenerationTestCase:
    description: str
    expected_stable: bool
    expected_recreated_distinct: bool


@dataclass(frozen=True)
class ExpressionNullabilityRuleTestCase:
    description: str
    function_name: str
    sql_expression: str
    rule_args: tuple[InferredNullability, ...]
    expected_nullability: InferredNullability
    expected_is_null: bool


@dataclass(frozen=True)
class ConnectTestCase:
    description: str
    config: dict[str, object]
    expected_connects: bool


@dataclass(frozen=True)
class ContendedConnectTestCase:
    description: str
    read_only: bool
    release_key: str
    expected_connection: AbstractContextManager[object]
    expected_rows: list[tuple[int]]


@dataclass(frozen=True)
class ConnectSettingsTestCase:
    description: str
    config: dict[str, object]
    expected_setting_value: str


@dataclass(frozen=True)
class QueryTestCase:
    description: str
    sql: str
    limit: int | None
    expected_result: QueryResult


@dataclass(frozen=True)
class RelationExistsTestCase:
    description: str
    setup_sql: tuple[str, ...]
    database: str | None
    schema: str | None
    name: str
    expected_exists: bool


@dataclass(frozen=True)
class ListRelationsTestCase:
    description: str
    setup_sql: tuple[str, ...]
    database: str | None
    schemas: tuple[str, ...] | None
    names: tuple[str, ...] | None
    expected_names: tuple[str, ...]


@dataclass(frozen=True)
class GetColumnsTestCase:
    description: str
    setup_sql: tuple[str, ...]
    database: str | None
    schema: str | None
    name: str
    expected_columns: tuple[ColumnInfo, ...]


@dataclass(frozen=True)
class GetAllColumnsTestCase:
    description: str
    setup_sql: tuple[str, ...]
    database: str | None
    schemas: tuple[str, ...] | None
    names: tuple[str, ...] | None
    expected_columns_by_table: dict[str, tuple[ColumnInfo, ...]]


@dataclass(frozen=True)
class MaterializeTestCase:
    description: str
    setup_sql: tuple[str, ...]
    expected_row_count: int


@dataclass(frozen=True)
class DropTestCase:
    description: str
    setup_sql: tuple[str, ...]
    target: str
    expected_exists: bool


@dataclass(frozen=True)
class RenameTestCase:
    description: str
    setup_sql: tuple[str, ...]
    source: str
    target: str
    expected_source_exists: bool
    expected_target_exists: bool


@dataclass(frozen=True)
class SwapTestCase:
    description: str
    setup_sql: tuple[str, ...]
    expected_left_value: str
    expected_right_value: str


@dataclass(frozen=True)
class ViewSwapTestCase:
    """A swap whose left relation is a view, which DuckDB must not rename as a table."""

    description: str
    setup_sql: tuple[str, ...]
    left: str
    expected_relations: list[tuple[str, str]]


@dataclass(frozen=True)
class DeleteInsertTestCase:
    description: str
    setup_sql: tuple[str, ...]
    sql: str
    unique_key: str | tuple[str, ...]
    expected_row_count: int
    expected_updated_value: str


@dataclass(frozen=True)
class MergeTestCase:
    description: str
    setup_sql: tuple[str, ...]
    source_sql: str
    unique_key: str | tuple[str, ...]
    expected_row_count: int
    expected_values: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class LoadSeedTestCase:
    description: str
    csv_content: str
    columns: tuple[ColumnInfo, ...]
    infer_types: bool
    expected_row_count: int
    expected_first_row: tuple[object, ...]
    csv_settings: SeedCsvSettings = field(default_factory=SeedCsvSettings)
    expected_recorded_fragment: str = ""


@dataclass(frozen=True)
class DiffSchemaTestCase:
    description: str
    left_sql: str
    right_sql: str
    expected_result: SchemaDiffResult


@dataclass(frozen=True)
class DiffRowsTestCase:
    description: str
    left_sql: str
    right_sql: str
    unique_key: str | tuple[str, ...]
    excluded_columns: tuple[str, ...] = field(default_factory=tuple)
    tolerances: RowDiffTolerances | None = None
    cursor_column: str | None = None
    start_cursor: CursorValue | None = None
    end_cursor: CursorValue | None = None
    expected_result: RowDiffResult = field(default_factory=RowDiffResult)


@dataclass(frozen=True)
class CompositeRowDiffSamplingTestCase:
    description: str
    row_limit: int
    seed: int
    alternate_seed: int
    expected_population_count: int


@dataclass(frozen=True)
class DiffRowsErrorTestCase:
    description: str
    left_sql: str
    right_sql: str
    unique_key: str | tuple[str, ...]
    expected_error_fragment: str
    tolerances: RowDiffTolerances | None = None


@dataclass(frozen=True)
class RecorderWriteTestCase:
    description: str
    setup_sql: tuple[str, ...]
    operation: str
    target: str
    sql: str
    expected_recorded_statements: tuple[str, ...]
    unique_key: str | tuple[str, ...] | None = None


@dataclass(frozen=True)
class TransactionalAtomicityTestCase:
    description: str
    setup_sql: tuple[str, ...]
    target: str
    source_sql: str
    unique_key: str | tuple[str, ...]
    expected_rows_after_failure: tuple[tuple[object, ...], ...]
    verify_sql: str


@dataclass(frozen=True)
class SnapshotAdapterMethodsTestCase:
    description: str
    expected_initial_custom_rows: tuple[tuple[object, ...], ...]
    expected_timestamp_rows: tuple[tuple[object, ...], ...]
    expected_timestamp_hard_delete_rows: tuple[tuple[object, ...], ...]
    expected_check_rows: tuple[tuple[object, ...], ...]
    expected_historical_timestamp_rows: tuple[tuple[object, ...], ...]
    expected_historical_changes_rows: tuple[tuple[object, ...], ...]
    expected_historical_check_rows: tuple[tuple[object, ...], ...]
    expected_historical_check_apply_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SnapshotTransactionRollbackTestCase:
    description: str
    expected_error_fragment: str
    expected_rows_after_failure: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class StorageVersionConnectTestCase:
    """A database file written by an earlier storage version, reopened for local replay."""

    description: str
    expected_rows: list[tuple[int]]


@dataclass(frozen=True)
class UserStorageVersionKeyTestCase:
    """A user connection config naming a DuckDB storage version, and the storage files get."""

    description: str
    config_key: str
    expected_storage_version: str


@dataclass(frozen=True)
class PythonFunctionRegistrationTestCase:
    """A Python function registered while an earlier result is only partly read."""

    description: str
    open_result_sql: str
    call_sql: str
    expected_rows: list[tuple[object, ...]]


@dataclass(frozen=True)
class QualifiedViewDefinitionTestCase:
    """A view at a database-qualified location compared with the SELECT SQLBuild renders."""

    description: str
    setup_sql: tuple[str, ...]
    database: str | None
    schema: str
    name: str
    sql: str
    expected_match: bool


@dataclass(frozen=True)
class SearchPathCacheTestCase:
    """An unqualified existence lookup cached before and after a statement changes the path."""

    description: str
    hook_sql: str
    expected_before: bool
    expected_after: bool
