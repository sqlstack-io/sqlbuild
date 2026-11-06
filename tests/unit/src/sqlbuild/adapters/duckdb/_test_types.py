from dataclasses import dataclass

from sqlbuild._native import InferredNullability


@dataclass(frozen=True)
class DuckDbMergeExclusionTestCase:
    description: str
    expected_update_clause: str
    expected_insert_clause: str
    unique_key: tuple[str, ...] = ("order_id", "line_id")
    source_columns: tuple[str, ...] = ("order_id", "line_id", "status", "updated_at")


@dataclass(frozen=True)
class DuckDbExpressionInferenceProfileTestCase:
    description: str
    expected_sql_analysis_dialect: str
    expected_rule_results: dict[str, InferredNullability]


@dataclass(frozen=True)
class DuckDbRenderCursorBoundLiteralTestCase:
    description: str
    value: str
    cursor_type: str | None
    expected_literal: str


@dataclass(frozen=True)
class DuckDbRenderIdentifierTestCase:
    description: str
    name: str
    expected_identifier: str


@dataclass(frozen=True)
class DuckDbRenderTableFunctionTestCase:
    description: str
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class DuckDbMetadataSqlTestCase:
    description: str
    database: str
    schema: str
    name: str
    expected_sql: tuple[str, ...]


@dataclass(frozen=True)
class DuckDbPruneSqlTestCase:
    description: str
    database: str | None
    schema: str
    retain_versions: int
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class DuckDbRenderSwapTestCase:
    description: str
    left: str
    right: str
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class DuckDbRenderSourceFreshnessQueryTestCase:
    description: str
    column: str
    source_relation: str
    source_is_subquery: bool
    where_sql: str
    expected_sql: str
