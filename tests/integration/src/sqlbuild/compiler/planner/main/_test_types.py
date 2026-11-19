from dataclasses import dataclass, field
from datetime import timedelta

from sqlbuild._native import (
    BackfillAction,
    PlanAction,
    PlanReason,
    WarningSeverity,
)
from sqlbuild.adapter.contract.types import MaintainedListing
from sqlbuild.compiler.compile.types import FunctionLanguage


@dataclass(frozen=True)
class FormatPlanIntegrationTestCase:
    description: str
    setup_sql: tuple[str, ...]
    model_locations: dict[str, str]
    model_configs: dict[str, dict[str, object]]
    model_queries: dict[str, str]
    full_refresh: bool
    expected_format_fragments: tuple[str, ...]
    unexpected_format_fragments: tuple[str, ...] = ()
    model_deps: dict[str, tuple[str, ...]] = field(default_factory=dict)
    seed_locations: dict[str, str] = field(default_factory=dict)
    effective_connection: dict[str, object] = field(default_factory=dict)


@dataclass(frozen=True)
class BuildExecutionPlanTestCase:
    description: str
    setup_sql: tuple[str, ...]
    model_locations: dict[str, str]
    model_configs: dict[str, dict[str, object]]
    model_queries: dict[str, str]
    full_refresh: bool
    expected_action: dict[str, PlanAction]
    expected_reason: dict[str, PlanReason]
    expected_ddl_fragments: dict[str, str] = field(default_factory=dict)
    expected_warning_severity: WarningSeverity | None = None
    expected_warning_count: int = 0
    expected_warning_fragment: str | None = None
    seed_locations: dict[str, str] = field(default_factory=dict)
    function_locations: dict[str, str] = field(default_factory=dict)
    function_bodies: dict[str, str] = field(default_factory=dict)
    previous_function_bodies: dict[str, str] = field(default_factory=dict)
    function_languages: dict[str, FunctionLanguage] = field(default_factory=dict)
    function_deps: dict[str, tuple[str, ...]] = field(default_factory=dict)
    select: tuple[str, ...] = ()
    expected_seed_names: tuple[str, ...] = ()
    expected_model_count: int | None = None
    effective_connection: dict[str, object] = field(default_factory=dict)
    model_deps: dict[str, tuple[str, ...]] = field(default_factory=dict)
    expected_backfill_action: dict[str, BackfillAction] = field(default_factory=dict)
    expected_progress_fragments: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class PlannerChangeDetectionWorkTestCase:
    description: str
    selection_diagnostics: bool
    expected_stale_detection_calls: dict[str, int]
    expected_warning_count: int
    query_change_tracking: bool = True
    expected_identity_builds: int = 1


@dataclass(frozen=True)
class SourceCursorInputPlanErrorTestCase:
    description: str
    setup_sql: tuple[str, ...]
    model_name: str
    source_name: str
    source_schema: str
    source_table: str
    cursor_column: str
    cursor_input_column: str
    expected_error_fragment: str


@dataclass(frozen=True)
class FutureCursorPlannerTestCase:
    """Planner call-flow case for future cursor policy."""

    description: str
    warehouse_type: str
    minimum: str
    maximum: str
    expected_relation: str


@dataclass(frozen=True)
class FutureCursorPlannerErrorTestCase:
    description: str
    expected_error_fragment: str


@dataclass(frozen=True)
class TableTypePlanAssemblyTestCase:
    description: str
    expected_entry_names: tuple[str, ...]


@dataclass(frozen=True)
class SnowflakeInspectionBudgetTestCase:
    """A synthetic multi-schema Snowflake project planned against a recording warehouse."""

    description: str
    unmanaged_relations_per_schema: int
    expected_schema_reads: dict[str, int]
    expected_metadata_budget: int
    expected_in_list_limit: int
    expected_freshness_reads: int


@dataclass(frozen=True)
class SnowflakeCursorBoundsBudgetTestCase:
    """Cursor-bound reads issued while planning a synthetic Snowflake project."""

    description: str
    statement_latency_seconds: float
    expected_max_concurrency: int


@dataclass(frozen=True)
class SnowflakeReplanTestCase:
    """Planning the same project twice in separate invocations."""

    description: str
    expected_listing_reads: int


@dataclass(frozen=True)
class SnowflakeManySchemasTestCase:
    """Many small model schemas planned against a recording warehouse."""

    description: str
    schema_count: int
    models_per_schema: int
    schema_listing_beyond_32: bool
    expected_per_relation_column_reads: int
    expected_schema_column_reads: int


@dataclass(frozen=True)
class SnowflakeFreshTargetTestCase:
    """Planning before the first build, when target schemas or the database do not exist."""

    description: str
    warehouse_schemas: frozenset[str]
    expected_reason: str
    expected_schema_checks: int


@dataclass(frozen=True)
class SnowflakeMissingDatabaseTestCase:
    """Planning against a database that does not exist or the role cannot use."""

    description: str
    warehouse_database: str
    expected_error_fragment: str


@dataclass(frozen=True)
class SourceTableProbeBudgetTestCase:
    description: str
    source_schema: str
    source_table: str
    expected_source_listings: int
    expected_probes: tuple[str, ...]


@dataclass(frozen=True)
class UnlistedMigrationOriginTestCase:
    description: str
    select: tuple[str, ...]
    expected_unlisted_reads: int


@dataclass(frozen=True)
class DeferredMissingOriginTestCase:
    description: str
    select: tuple[str, ...]
    expected_missing: tuple[str, ...]
    expected_unlisted_reads: int


@dataclass(frozen=True)
class PlanFromTestCase:
    """Where a target and the command ask a plan to read from, and what the plan reports."""

    description: str
    target_plan_from: str | None
    requested: str | None
    store: bool
    expected_report: str
    expected_absent: tuple[str, ...]
    expected_live_listing: bool


@dataclass(frozen=True)
class SyncedScanTestCase:
    """When a table's last DDL ran relative to the scan that last found its record unchanged."""

    description: str
    last_ddl_after_scan_start: timedelta
    expected_live_describe: bool
    location_clock_ahead: timedelta = timedelta()


@dataclass(frozen=True)
class MaintainedRefreshPlanTestCase:
    description: str
    listed_settings: MaintainedListing | None
    upstream_query: str
    expected_upstream_action: PlanAction
    expected_action: PlanAction
    expected_reason: PlanReason
    expected_reinitialised_by: str | None
    expected_read_names: tuple[str, ...]
    expected_paused: bool = False
    totals_config: dict[str, object] | None = None


@dataclass(frozen=True)
class ChangeTrackingPlanTestCase:
    description: str
    select: tuple[str, ...]
    expected_maintained_readers: dict[str, tuple[str, ...]]
    expected_refresh_on_build: dict[str, bool]
