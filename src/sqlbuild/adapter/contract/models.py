"""Adapter domain models."""

from __future__ import annotations

from collections.abc import Callable, Mapping
from dataclasses import dataclass, field
from datetime import datetime
from decimal import Decimal
from pathlib import Path
from typing import Any

from sqlbuild.adapter.contract.types import (
    CursorKind,
    FunctionNullabilityRule,
    HistoricalSnapshotCloseStyle,
    HistoricalSnapshotInsertStyle,
    LifeCycleEventKind,
    MigrationTransfer,
    RelationReadStatus,
    RetentionChangePhase,
    RetentionScope,
    SnapshotLatestVersionStyle,
    SnapshotUpdateStyle,
    TableFreshnessStatus,
    TypeFamily,
)
from sqlbuild.compiler.compile.types import FunctionLanguage


@dataclass(frozen=True)
class CursorValue:
    """A typed cursor boundary for incremental and diff operations."""

    kind: CursorKind | str
    value: datetime | int

    def __post_init__(self) -> None:
        object.__setattr__(self, "kind", CursorKind(self.kind))


@dataclass(frozen=True)
class ColumnInfo:
    """One column from a warehouse relation."""

    name: str
    type: str


@dataclass(frozen=True)
class NormalizedType:
    """Semantic type shape used for schema comparison and numeric-family checks."""

    normalized_name: str
    family: TypeFamily
    precision: int | None = None
    scale: int | None = None
    length: int | None = None


@dataclass(frozen=True)
class ExpressionInferenceProfile:
    """Static SQL expression inference behavior exposed by an adapter."""

    sql_analysis_dialect: str | None = None
    function_nullability_rules: Mapping[str, FunctionNullabilityRule] = field(default_factory=dict)
    function_return_types: Mapping[str, str] = field(default_factory=dict)
    quoted_identifiers_ignore_case: bool = False
    semantic_known_functions: tuple[str, ...] = ()
    semantic_known_types: tuple[str, ...] = ()
    binding_catalog: Any | None = field(default=None, repr=False, compare=False)

    def function_nullability_rule(self, function_name: str) -> FunctionNullabilityRule | None:
        """Return the adapter rule for a function name, if one is registered."""

        return self.function_nullability_rules.get(function_name.upper())

    def function_return_type(self, function_name: str) -> str | None:
        """Return the adapter-declared function result type, if one is registered."""

        return self.function_return_types.get(function_name.upper())


@dataclass(frozen=True)
class RelationInfo:
    """Metadata for one discovered warehouse relation."""

    database: str | None
    schema: str | None
    name: str
    relation_type: str
    created_at: datetime | None = None
    last_altered_at: datetime | None = None
    is_transient: bool | None = None
    retention_days: int | None = None

    @property
    def identity(self) -> tuple[str | None, str | None, str]:
        """Return the case-insensitive physical relation identity."""

        return RelationLookup.key(database=self.database, schema=self.schema, name=self.name)


@dataclass(frozen=True)
class RelationUsage:
    """Reads of one relation within a usage window: the latest, who read it and how often."""

    database: str | None
    schema: str | None
    name: str
    last_read_at: datetime
    readers: tuple[str, ...]
    query_count: int
    direct: bool = True
    via: tuple[str, ...] = ()


@dataclass(frozen=True)
class RelationRead:
    """One reader's reads of one object; indirect reads came through `via`, empty if unknown."""

    object_name: str
    reader: str | None
    query_count: int
    last_read_at: object
    direct: bool = True
    via: tuple[str, ...] = ()


@dataclass(frozen=True)
class RelationUsageReport:
    """The relations read within a usage window, or why the warehouse could not tell."""

    checked: bool
    reason: str | None = None
    usages: tuple[RelationUsage, ...] = ()
    notices: tuple[str, ...] = ()


@dataclass(frozen=True)
class RelationLookup:
    """Existing warehouse relations gathered once and queried in memory."""

    relations_by_key: dict[tuple[str | None, str | None, str], RelationInfo]

    @staticmethod
    def key(
        *, database: str | None = None, schema: str | None, name: str
    ) -> tuple[str | None, str | None, str]:
        """Build the case-insensitive lookup key for one relation."""

        return (
            None if database is None else database.lower(),
            None if schema is None else schema.lower(),
            name.lower(),
        )

    def get(
        self, *, database: str | None = None, schema: str | None, name: str
    ) -> RelationInfo | None:
        """Return the gathered relation at one schema and name when present."""

        return self.relations_by_key.get(self.key(database=database, schema=schema, name=name))

    def exists(self, *, database: str | None = None, schema: str | None, name: str) -> bool:
        """Return whether a relation was present at one schema and name."""

        return self.key(database=database, schema=schema, name=name) in self.relations_by_key

    def is_transient(self, *, database: str | None = None, schema: str | None, name: str) -> bool:
        """Return whether a gathered relation is transient."""

        relation: RelationInfo | None = self.get(database=database, schema=schema, name=name)
        return bool(relation.is_transient) if relation is not None else False


@dataclass(frozen=True)
class RelationReadProbe:
    """Whether a rendered relation is readable, and the role used when the adapter knows it."""

    status: RelationReadStatus
    role: str | None = None


@dataclass(frozen=True)
class TableFreshnessMetadata:
    """Adapter freshness outcome for one physical table: observed or unavailable."""

    data_version: object
    value_kind: str
    observed_at: datetime | None = None
    status: TableFreshnessStatus = TableFreshnessStatus.OBSERVED
    message: str | None = None

    @classmethod
    def unavailable(cls, *, message: str) -> TableFreshnessMetadata:
        """Return the outcome for a table whose freshness metadata cannot be compared."""

        return cls(
            data_version=None,
            value_kind="timestamp",
            status=TableFreshnessStatus.UNAVAILABLE,
            message=message,
        )


@dataclass(frozen=True)
class RelationGrant:
    """One privilege granted or denied on a relation, or on one of its columns."""

    privilege: str
    grantee: str | None
    column: str | None = None
    grantable: bool = False
    denied: bool = False
    grantee_kind: str = ""


@dataclass(frozen=True)
class TableFreshnessRequest:
    """Physical table identity for adapter metadata freshness lookup."""

    database: str | None
    schema: str | None
    name: str


@dataclass(frozen=True)
class RetentionRequest:
    """One identified desired retention setting for a warehouse object."""

    request_id: str
    scope: RetentionScope
    database: str | None
    schema: str
    desired_days: int
    name: str | None = None


@dataclass(frozen=True)
class RetentionState:
    """Typed warehouse retention values observed for one request."""

    request_id: str
    scope: RetentionScope
    configured_days: int | None
    effective_days: int
    exists: bool = True
    relation_kind: str | None = None
    is_transient: bool | None = None
    delta_log_retention_days: int | None = None
    delta_deleted_file_retention_days: int | None = None
    max_time_travel_hours: int | None = None


@dataclass(frozen=True)
class RenderedRetentionChange:
    """One ordered phase of SQL statements for a retention change."""

    phase: RetentionChangePhase
    statements: tuple[str, ...]


@dataclass(frozen=True)
class MigrationStagePlan:
    """Fresh migration stage statements, plus a copy used only for recognized clone refusals."""

    transfer: MigrationTransfer
    statements: tuple[str, ...]
    fallback_statements: tuple[str, ...] = ()
    is_clone_refusal: Callable[[BaseException], bool] | None = None


@dataclass(frozen=True)
class FunctionInfo:
    """Metadata for one discovered warehouse function or routine."""

    database: str | None
    schema: str | None
    name: str
    function_type: str


@dataclass(frozen=True)
class SnapshotChangeTarget:
    """Common destination/origin columns for a snapshot change-apply render."""

    destination: str
    origin: str
    unique_key: tuple[str, ...]
    valid_from_column: str
    valid_to_column: str
    output_columns: tuple[str, ...]


@dataclass(frozen=True)
class SnapshotSqlDialect:
    """Dialect differences in adapter-rendered snapshot SQL."""

    timestamp_type: str
    distinct_condition: Callable[..., str]
    update_style: SnapshotUpdateStyle
    latest_version: SnapshotLatestVersionStyle
    historical_close: HistoricalSnapshotCloseStyle
    historical_insert: HistoricalSnapshotInsertStyle


@dataclass(frozen=True)
class FunctionDefinition:
    """Warehouse function definition inputs for create/render adapter methods."""

    destination: str
    arguments: tuple[Any, ...]
    returns: str
    body_sql: str
    return_columns: tuple[Any, ...] = ()
    language: FunctionLanguage = FunctionLanguage.SQL
    runtime_version: str | None = None
    entry_point: str | None = None
    packages: tuple[str, ...] = ()
    source_file_path: Path | None = None


@dataclass(frozen=True)
class TemporaryFunction:
    """A session-scoped copy of a project function: its call target and creating DDL."""

    target: str
    statements: tuple[str, ...]


@dataclass(frozen=True)
class QueryResult:
    """Rows returned by an ad hoc warehouse query."""

    columns: tuple[str, ...] = field(default_factory=tuple)
    rows: tuple[tuple[object, ...], ...] = field(default_factory=tuple)
    truncated: bool = False
    column_types: tuple[str | None, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class SchemaDiffResult:
    """Schema comparison between two relations."""

    added_columns: tuple[ColumnInfo, ...] = field(default_factory=tuple)
    removed_columns: tuple[ColumnInfo, ...] = field(default_factory=tuple)
    type_changed_columns: tuple[tuple[ColumnInfo, ColumnInfo], ...] = field(default_factory=tuple)
    left_column_count: int = field(default=0, compare=False)
    right_column_count: int = field(default=0, compare=False)


@dataclass(frozen=True)
class RowDiffColumnResult:
    """Per-column row-level comparison summary."""

    name: str
    mismatched_count: int = 0
    tolerance: RowDiffTolerance | None = None


@dataclass(frozen=True)
class RowDiffCoverage:
    """Exact bounded relation coverage observed before row comparison."""

    row_count: int = 0
    minimum_cursor: object | None = None
    maximum_cursor: object | None = None


@dataclass(frozen=True)
class RelationRowCountEstimate:
    """Relation estimates; unavailable counts and storage bytes remain None."""

    row_count: int | None
    detail: str | None = None
    byte_count: int | None = None
    byte_count_basis: str | None = None


@dataclass(frozen=True)
class RowDiffSampling:
    """Deterministic key-sampling policy for a row comparison."""

    row_limit: int
    seed: int = 0


@dataclass(frozen=True)
class RowDiffPreparedRelations:
    """Comparison CTEs and optional exact bounded union population."""

    cte_sql: str
    population_count: int | None = None


@dataclass(frozen=True)
class RowDiffResult:
    """Row-level comparison between two relations."""

    left_count: int = 0
    right_count: int = 0
    joined_count: int = 0
    equal_count: int = 0
    unequal_count: int = 0
    left_only_count: int = 0
    right_only_count: int = 0
    column_results: tuple[RowDiffColumnResult, ...] = field(default_factory=tuple)
    population_count: int = field(default=0, compare=False)
    compared_count: int = field(default=0, compare=False)
    sampling: RowDiffSampling | None = field(default=None, compare=False)
    left_coverage: RowDiffCoverage | None = field(default=None, compare=False)
    right_coverage: RowDiffCoverage | None = field(default=None, compare=False)
    cursor_column: str | None = field(default=None, compare=False)
    cursor_start: object | None = field(default=None, compare=False)
    cursor_end: object | None = field(default=None, compare=False)

    @property
    def is_exhaustive(self) -> bool:
        """Return whether every bounded union key was compared."""

        return self.sampling is None or self.compared_count >= self.population_count


@dataclass(frozen=True)
class RowDiffTolerance:
    """Numeric tolerance for row-level diff comparisons."""

    absolute: Decimal | None = None
    relative: Decimal | None = None


@dataclass(frozen=True)
class RowDiffSampleCell:
    """One sampled left/right value pair for a changed column."""

    name: str
    left_value: object
    right_value: object


@dataclass(frozen=True)
class RowDiffSampleRow:
    """One sampled unequal row for verbose diff output."""

    key_values: tuple[tuple[str, object], ...] = field(default_factory=tuple)
    changed_cells: tuple[RowDiffSampleCell, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class RowDiffTolerances:
    """Resolved row-level diff tolerance rules."""

    by_type: dict[str, RowDiffTolerance] = field(default_factory=dict)
    by_column: dict[str, RowDiffTolerance] = field(default_factory=dict)
    sampling: RowDiffSampling | None = field(default=None, compare=False, repr=False)


@dataclass(frozen=True)
class LifeCycleEvent:
    """One recorded event from a model's runtime lifecycle."""

    kind: LifeCycleEventKind
    content: str
