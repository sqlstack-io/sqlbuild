from dataclasses import dataclass, field

from sqlbuild.adapter.contract.models import (
    ColumnInfo,
    CursorValue,
    QueryResult,
    RowDiffResult,
    RowDiffSampleRow,
    RowDiffTolerances,
    SchemaDiffResult,
)


@dataclass(frozen=True)
class PostgresSchemaIntrospectionTestCase:
    description: str
    table_name: str
    ddl: str
    expected_columns: tuple[ColumnInfo, ...]


@dataclass(frozen=True)
class PostgresQueryTestCase:
    description: str
    sql: str
    expected_result: QueryResult


@dataclass(frozen=True)
class PostgresBuildFlowTestCase:
    description: str
    table_name: str
    source_sql: str
    expected_row_count: int


@dataclass(frozen=True)
class PostgresMergeTestCase:
    description: str
    table_name: str
    initial_sql: str
    merge_sql: str
    unique_key: tuple[str, ...]
    expected_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class PostgresRowDiffTestCase:
    description: str
    left_sql: str
    right_sql: str
    unique_key: tuple[str, ...]
    expected_result: RowDiffResult
    excluded_columns: tuple[str, ...] = field(default_factory=tuple)
    tolerances: RowDiffTolerances | None = None
    cursor_column: str | None = None
    start_cursor: CursorValue | None = None
    end_cursor: CursorValue | None = None


@dataclass(frozen=True)
class PostgresRowDiffErrorTestCase:
    description: str
    left_sql: str
    right_sql: str
    unique_key: tuple[str, ...]
    expected_error_fragment: str
    tolerances: RowDiffTolerances | None = None


@dataclass(frozen=True)
class PostgresRowDiffSampleTestCase:
    description: str
    left_sql: str
    right_sql: str
    unique_key: tuple[str, ...]
    expected_unequal_rows: tuple[RowDiffSampleRow, ...] = field(default_factory=tuple)
    expected_side_only_rows: tuple[tuple[tuple[str, object], ...], ...] = field(
        default_factory=tuple
    )
    side: str = "left"


@dataclass(frozen=True)
class PostgresSchemaDiffTestCase:
    description: str
    left_ddl: str
    right_ddl: str
    expected_result: SchemaDiffResult


@dataclass(frozen=True)
class PostgresSeedTestCase:
    description: str
    csv_text: str
    expected_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class PostgresLoadSessionTestCase:
    description: str
    files: tuple[tuple[str, str], ...]
    expected_rows: tuple[tuple[object, ...], ...]
