"""Adapter domain types."""

from __future__ import annotations

from collections.abc import Callable
from enum import StrEnum
from typing import Literal, Protocol

from sqlbuild._native import InferredNullability

type FunctionNullabilityRule = Callable[[tuple[InferredNullability, ...]], InferredNullability]
type StatementSizeLimit = tuple[int, Literal["bytes", "characters"]]


class AdapterExecute[ConnectionT, ResultT](Protocol):
    def __call__(self, *, connection: ConnectionT, sql: str) -> ResultT: ...


class BuiltinAdapter(StrEnum):
    DUCKDB = "duckdb"
    MOTHERDUCK = "motherduck"
    SNOWFLAKE = "snowflake"
    BIGQUERY = "bigquery"
    DATABRICKS = "databricks"
    POSTGRES = "postgres"
    SQLSERVER = "sqlserver"
    SQLITE = "sqlite"
    CLICKHOUSE = "clickhouse"


class RelationReadStatus(StrEnum):
    """Outcome of probing whether a rendered relation can be read."""

    READABLE = "readable"
    MISSING = "missing"
    MISSING_OR_UNREADABLE = "missing_or_unreadable"


class TableFreshnessStatus(StrEnum):
    """Per-table outcome of an adapter freshness metadata lookup."""

    OBSERVED = "observed"
    UNAVAILABLE = "unavailable"


class CursorKind(StrEnum):
    TIMESTAMP = "timestamp"
    INTEGER = "integer"


class PromotionStrategy(StrEnum):
    ATOMIC_SWAP = "atomic_swap"
    ATOMIC_REPLACE = "atomic_replace"


class MigrationTransfer(StrEnum):
    """How a model migration stage receives the origin's data."""

    CLONE = "clone"
    COPY = "copy"

    @property
    def label(self) -> str:
        """Return the human-readable plan label for this transfer."""

        return "zero-copy clone" if self == MigrationTransfer.CLONE else "physical copy"


class TablePromotionMode(StrEnum):
    IMMEDIATE = "immediate"
    STAGED = "staged"


class RetentionScope(StrEnum):
    RELATION = "relation"
    NAMESPACE = "namespace"


class RetentionChangePhase(StrEnum):
    PREPARE = "prepare"
    ALTER = "alter"
    FINALIZE = "finalize"


class RelationType(StrEnum):
    TABLE = "table"
    VIEW = "view"
    OTHER = "other"


class LifeCycleEventKind(StrEnum):
    SQL = "sql"
    LOG = "log"


class FrameworkType(StrEnum):
    STRING = "string"
    INTEGER = "integer"
    TIMESTAMP = "timestamp"


class LoaderLogicalType(StrEnum):
    BOOLEAN = "boolean"
    INTEGER = "integer"
    FLOAT = "float"
    DECIMAL = "decimal"
    STRING = "string"
    TIMESTAMP = "timestamp"
    DATE = "date"
    JSON = "json"


class TypeFamily(StrEnum):
    INTEGER = "integer"
    DECIMAL = "decimal"
    FLOAT = "float"
    STRING = "string"
    BOOLEAN = "boolean"
    TIMESTAMP = "timestamp"
    DATE = "date"
    DATETIME = "datetime"
    OTHER = "other"


class TypeDialect(StrEnum):
    GENERIC = "generic"
    BIGQUERY = "bigquery"
    SNOWFLAKE = "snowflake"
    DUCKDB = "duckdb"
    MOTHERDUCK = "motherduck"
    DATABRICKS = "databricks"
    POSTGRES = "postgres"
    TSQL = "tsql"


class SnapshotLatestVersionStyle(StrEnum):
    """How incremental snapshot SQL selects each key's latest stored version."""

    QUALIFY = "qualify"
    DERIVED_TABLE = "derived_table"
    ORDERED_CTE = "ordered_cte"


class SnapshotUpdateStyle(StrEnum):
    """How current-state snapshot SQL closes active versions."""

    UPDATE_FROM = "update_from"
    MERGE = "merge"
    TSQL = "tsql"


class HistoricalSnapshotCloseStyle(StrEnum):
    """How historical snapshot SQL closes versions superseded by new observations."""

    CORRELATED = "correlated"
    UPDATE_FROM = "update_from"
    MERGE_HARD_DELETES = "merge_hard_deletes"
    TSQL = "tsql"


class HistoricalSnapshotInsertStyle(StrEnum):
    """Where historical snapshot inserts place their common table expressions."""

    WITH_INSERT = "with_insert"
    INSERT_WITH = "insert_with"
    TSQL = "tsql"
