from dataclasses import dataclass

from sqlbuild.adapter.contract.models import (
    ColumnInfo,
    CursorValue,
    RowDiffTolerances,
)


@dataclass(frozen=True)
class SqlServerMergeExclusionTestCase:
    description: str
    expected_update_clause: str
    expected_insert_clause: str


@dataclass(frozen=True)
class SqlServerAdapterDefaultsTestCase:
    description: str
    expected_default_schema: str
    expected_default_database: str | None
    expected_sql_analysis_dialect: str | None
    expected_identifier_length: int


@dataclass(frozen=True)
class SqlServerRenderIdentifierTestCase:
    description: str
    name: str
    expected_identifier: str


@dataclass(frozen=True)
class SqlServerRenderCreateTableAsTestCase:
    description: str
    target: str
    sql: str
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class SqlServerRenderCreateFunctionTestCase:
    description: str
    arguments: tuple[tuple[str, str], ...]
    returns: str
    body_sql: str
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class SqlServerRenderCreateSchemaTestCase:
    description: str
    schema: str
    expected_statement: str


@dataclass(frozen=True)
class SqlServerRenderQualifiedNameTestCase:
    description: str
    database: str | None
    schema: str | None
    name: str
    expected_name: str


@dataclass(frozen=True)
class SqlServerRenderRenameTestCase:
    description: str
    origin: str
    destination: str
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class SqlServerCursorBoundLiteralTestCase:
    description: str
    value: str
    cursor_type: str | None
    expected_literal: str


@dataclass(frozen=True)
class SqlServerDeleteInsertCursorSqlTestCase:
    description: str
    cursor_column: str
    cursor_start: str
    cursor_end: str
    cursor_type: str | None
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class SqlServerDiffCursorFilterTestCase:
    description: str
    cursor_column: str
    start_cursor: CursorValue
    end_cursor: CursorValue | None
    expected_filter: str


@dataclass(frozen=True)
class SqlServerRowDiffEqualitySqlTestCase:
    description: str
    column: str
    column_info: ColumnInfo
    tolerances: RowDiffTolerances | None
    expected_fragments: tuple[str, ...]
    unexpected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SqlServerRollbackTestCase:
    description: str
    expected_statement: str


@dataclass(frozen=True)
class SqlServerMoveOrCopyRelationTestCase:
    description: str
    source: str
    target: str
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class SqlServerIndexSqlTestCase:
    description: str
    database: str | None
    schema: str
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class SqlServerLatestReadSqlTestCase:
    description: str
    database: str | None
    schema: str
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SqlServerPruneSqlTestCase:
    description: str
    database: str | None
    schema: str
    retain_versions: int
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SqlServerMigrationEventWriteTestCase:
    description: str
    existing_rows: tuple[tuple[object, ...], ...]
    expected_statement_prefixes: tuple[str, ...]


@dataclass(frozen=True)
class SqlServerMigrationTableTestCase:
    description: str
    schema: str
    expected_prefix: str
    expected_suffix: str


@dataclass(frozen=True)
class SqlServerMigrationEventReadTestCase:
    description: str
    stored_created_at: object
    expected_read_suffix: str
