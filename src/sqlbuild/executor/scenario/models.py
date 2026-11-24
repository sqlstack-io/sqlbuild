"""Scenario executor domain models."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass, field
from pathlib import Path

from sqlbuild._native import (
    CompiledRelationLocation,
    MaterializationType,
    ScenarioArtifactKind,
    ScenarioExecutionPlan,
    ScenarioRelationMap,
    SqlLexicalSyntax,
)
from sqlbuild.adapter.contract.models import LifeCycleEvent
from sqlbuild.adapter.contract.types import TablePromotionMode
from sqlbuild.executor.build.models import FunctionExecutionResult, SeedExecutionResult
from sqlbuild.executor.run.models import ModelExecutionResult
from sqlbuild.executor.scenario.classes.prepared_scenario_schemas import PreparedScenarioSchemas
from sqlbuild.executor.scenario.classes.scenario_interrupts import ScenarioInterrupts
from sqlbuild.executor.scenario.types import ScenarioLocalRunStatus, ScenarioSnapshotState
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.runtime.execution_limits.models import RunItemTimeouts


@dataclass(frozen=True)
class ScenarioFixtureExecutionResult:
    """Outcome of materializing one scenario fixture relation."""

    scenario_name: str
    kind: ScenarioArtifactKind
    logical_name: str
    target_relation: str
    status: ExecutionStatus
    lifecycle_events: tuple[LifeCycleEvent, ...] = field(default_factory=tuple)
    error_code: str | None = None
    error_help: str | None = None
    error_message: str | None = None


@dataclass(frozen=True)
class ScenarioRunOptions:
    """Invocation-wide settings shared by every warehouse scenario run."""

    promotion_mode: TablePromotionMode
    prepared_schemas: PreparedScenarioSchemas | None = None
    interrupts: ScenarioInterrupts = field(default_factory=ScenarioInterrupts)


@dataclass(frozen=True)
class ScenarioCleanupTarget:
    """One planned scenario-owned relation selected for cleanup."""

    kind: ScenarioArtifactKind
    logical_name: str
    target_relation: str
    materialization_type: MaterializationType = MaterializationType.TABLE
    database: str | None = None
    schema: str | None = None
    name: str | None = None
    listed: bool = False


@dataclass(frozen=True)
class ScenarioSnapshotColumn:
    """One column captured in a local scenario snapshot relation."""

    name: str
    warehouse_type: str
    local_type: str


@dataclass(frozen=True)
class ScenarioSnapshotRelation:
    """One captured relation recorded in a local scenario snapshot manifest."""

    kind: ScenarioArtifactKind
    logical_name: str
    file_path: Path
    row_count: int
    byte_count: int
    columns: tuple[ScenarioSnapshotColumn, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class ScenarioSnapshotManifest:
    """Metadata for one local scenario snapshot."""

    version: int
    scenario_name: str
    captured_at: str
    capture_adapter: str
    capture_dialect: str
    sqlbuild_version: str
    input_fingerprint: str
    total_rows: int
    total_bytes: int
    relations: tuple[ScenarioSnapshotRelation, ...] = field(default_factory=tuple)
    format: str = "jsonl"


@dataclass(frozen=True)
class ScenarioSnapshotInputSpec:
    """Stable input requirement used to fingerprint local scenario snapshots."""

    kind: ScenarioArtifactKind
    logical_name: str
    file_path: Path
    capture_sql: str
    fingerprint_sql: str | None = None


@dataclass(frozen=True)
class ScenarioSnapshotFileStats:
    """Write statistics for one durable local scenario snapshot file."""

    row_count: int
    byte_count: int


@dataclass(frozen=True)
class ScenarioSnapshotCaptureLimits:
    """Safety limits for writing durable local scenario snapshots."""

    max_rows_per_relation: int | None = None
    max_total_rows: int | None = None
    max_bytes_per_relation: int | None = None
    max_total_bytes: int | None = None
    force: bool = False


@dataclass(frozen=True)
class ScenarioLocalReplaySource:
    """Project adapter whose dialect locally replayed scenario SQL is authored in."""

    lexical_syntax: SqlLexicalSyntax
    capture_adapter: str | None = None
    capture_dialect: str | None = None


@dataclass(frozen=True)
class LocalConversion:
    """What SQLQuack's conversion of local scenario SQL reads: model column types and time zone."""

    model_columns: Mapping[str, tuple[tuple[str, str], ...]] = field(default_factory=dict)
    time_zone: str | None = None
    # Each model's file, relative to the project, and its authored SQL, to place refusals in.
    model_files: Mapping[str, tuple[str, str]] = field(default_factory=dict)


@dataclass(frozen=True)
class ScenarioTestSettings:
    """Cleanup, concurrency and time-limit settings for one warehouse scenario test run."""

    retain: bool
    concurrency: int = 1
    timeouts: RunItemTimeouts | None = None


@dataclass(frozen=True)
class ScenarioCaptureSettings:
    """Capture provenance and safety settings for one scenario snapshot run."""

    captured_at: str
    capture_adapter: str
    capture_dialect: str
    sqlbuild_version: str
    retain: bool
    limits: ScenarioSnapshotCaptureLimits | None = None


@dataclass(frozen=True)
class ScenarioFailureDetails:
    """Error code, help text, and message for one failed scenario step."""

    error_code: str | None = None
    error_help: str | None = None
    error_message: str | None = None


@dataclass(frozen=True)
class ScenarioStepResults:
    """Per-phase execution results collected during one scenario run."""

    fixture_results: tuple[ScenarioFixtureExecutionResult, ...] = ()
    seed_results: tuple[SeedExecutionResult, ...] = ()
    model_results: tuple[ModelExecutionResult, ...] = ()
    expected_results: tuple[ScenarioExpectedExpectationExecutionResult, ...] = ()
    assertion_results: tuple[ScenarioAssertionExpectationExecutionResult, ...] = ()


@dataclass(frozen=True)
class ScenarioSnapshotStateResult:
    """Manifest freshness classification for one local scenario snapshot."""

    state: ScenarioSnapshotState
    manifest_path: Path
    manifest: ScenarioSnapshotManifest | None = None
    error_code: str | None = None
    error_help: str | None = None
    error_message: str | None = None


@dataclass(frozen=True)
class ScenarioLocalSnapshotLoadedRelation:
    """One local DuckDB table loaded from a durable scenario snapshot relation."""

    kind: ScenarioArtifactKind
    logical_name: str
    file_path: Path
    table_name: str
    row_count: int


@dataclass(frozen=True)
class ScenarioLocalSnapshotLoadResult:
    """Outcome of loading one local scenario snapshot into DuckDB."""

    scenario_name: str
    manifest: ScenarioSnapshotManifest
    relations: tuple[ScenarioLocalSnapshotLoadedRelation, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class ScenarioLocalSnapshotTypeValidationIssue:
    """One DuckDB-rejected local type in a scenario snapshot manifest."""

    scenario_name: str
    manifest_path: Path
    kind: ScenarioArtifactKind
    logical_name: str
    column_name: str
    local_type: str
    error_message: str


@dataclass(frozen=True)
class ScenarioSnapshotCaptureRelationPlan:
    """One scenario input relation planned for durable local snapshot capture."""

    kind: ScenarioArtifactKind
    logical_name: str
    source_target: CompiledRelationLocation
    file_path: Path
    capture_sql: str


@dataclass(frozen=True)
class ScenarioSnapshotCapturePlan:
    """Executor-side plan for capturing scenario inputs into local snapshot files."""

    scenario_name: str
    snapshot_root: Path
    manifest_path: Path
    input_fingerprint: str
    relations: tuple[ScenarioSnapshotCaptureRelationPlan, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class ScenarioSnapshotCaptureRelationResult:
    """Outcome of capturing one scenario input relation into JSONL."""

    kind: ScenarioArtifactKind
    logical_name: str
    source_relation: str
    file_path: Path
    status: ExecutionStatus
    row_count: int = 0
    byte_count: int = 0
    error_code: str | None = None
    error_help: str | None = None
    error_message: str | None = None


@dataclass(frozen=True)
class ScenarioSnapshotCaptureResult:
    """Outcome of writing local snapshot files for one scenario capture plan."""

    scenario_name: str
    status: ExecutionStatus
    manifest_path: Path
    manifest: ScenarioSnapshotManifest | None = None
    relation_results: tuple[ScenarioSnapshotCaptureRelationResult, ...] = field(
        default_factory=tuple
    )
    error_code: str | None = None
    error_help: str | None = None
    error_message: str | None = None


@dataclass(frozen=True)
class ScenarioSnapshotCaptureRunResult:
    """Outcome of materializing inputs and writing one local scenario snapshot."""

    scenario_name: str
    status: ExecutionStatus
    retained: bool = False
    fixture_results: tuple[ScenarioFixtureExecutionResult, ...] = field(default_factory=tuple)
    seed_results: tuple[SeedExecutionResult, ...] = field(default_factory=tuple)
    capture_result: ScenarioSnapshotCaptureResult | None = None
    prepare_cleanup_result: ScenarioCleanupExecutionResult | None = None
    cleanup_result: ScenarioCleanupExecutionResult | None = None
    error_code: str | None = None
    error_help: str | None = None
    error_message: str | None = None
    duration_ms: float | None = None


@dataclass(frozen=True)
class ScenarioCleanupExecutionResult:
    """Outcome of dropping planned scenario-owned relations."""

    scenario_name: str
    status: ExecutionStatus
    targets: tuple[ScenarioCleanupTarget, ...] = field(default_factory=tuple)
    lifecycle_events: tuple[LifeCycleEvent, ...] = field(default_factory=tuple)
    error_code: str | None = None
    error_help: str | None = None
    error_message: str | None = None


@dataclass(frozen=True)
class ScenarioExpectedExpectationExecutionResult:
    """Outcome of comparing one scenario model relation to expected SQL; no counts if not run."""

    scenario_name: str
    model_name: str
    status: ExecutionStatus
    actual_row_count: int | None = None
    expected_row_count: int | None = None
    mismatched_row_count: int | None = None
    missing_row_count: int | None = None
    error_code: str | None = None
    error_help: str | None = None
    error_message: str | None = None


@dataclass(frozen=True)
class ScenarioAssertionExpectationExecutionResult:
    """Outcome of executing one zero-row scenario assertion; no count if it did not run."""

    scenario_name: str
    name: str
    status: ExecutionStatus
    failing_row_count: int | None = None
    sample_rows: tuple[tuple[object, ...], ...] = field(default_factory=tuple)
    error_code: str | None = None
    error_help: str | None = None
    error_message: str | None = None


@dataclass(frozen=True)
class ScenarioRunResult:
    """Result for one scenario execution run."""

    scenario_name: str
    status: ExecutionStatus
    retained: bool = False
    local_status: ScenarioLocalRunStatus | None = None
    local_duckdb_path: Path | None = None
    local_execution_plan: ScenarioExecutionPlan | None = None
    local_snapshot_relations: tuple[ScenarioLocalSnapshotLoadedRelation, ...] = field(
        default_factory=tuple
    )
    relation_map: ScenarioRelationMap | None = None
    fixture_results: tuple[ScenarioFixtureExecutionResult, ...] = field(default_factory=tuple)
    seed_results: tuple[SeedExecutionResult, ...] = field(default_factory=tuple)
    function_results: tuple[FunctionExecutionResult, ...] = field(default_factory=tuple)
    model_results: tuple[ModelExecutionResult, ...] = field(default_factory=tuple)
    expected_results: tuple[ScenarioExpectedExpectationExecutionResult, ...] = field(
        default_factory=tuple
    )
    assertion_results: tuple[ScenarioAssertionExpectationExecutionResult, ...] = field(
        default_factory=tuple
    )
    prepare_cleanup_result: ScenarioCleanupExecutionResult | None = None
    cleanup_result: ScenarioCleanupExecutionResult | None = None
    error_code: str | None = None
    error_help: str | None = None
    error_message: str | None = None
    duration_ms: float | None = None
