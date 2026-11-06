"""Build executor domain models."""

from __future__ import annotations

from collections.abc import Callable, Mapping
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path

from sqlbuild._native import (
    AuditPlanEntry,
    CompiledObjectKey,
    CompiledRelationLocation,
    DiscoveredLoaderFunction,
    FunctionPlanEntry,
    ModelPlanEntry,
    SeedPlanEntry,
    SnapshotsConfig,
    SourceLoadPlanEntry,
    SqlTestPlanEntry,
)
from sqlbuild.adapter.contract.models import LifeCycleEvent, RelationInfo
from sqlbuild.adapter.contract.types import TablePromotionMode
from sqlbuild.cost.models import StatementExecutionTelemetry
from sqlbuild.executor.auditing.models import AuditExecutionResult
from sqlbuild.executor.build.types import BeforeNodeWriteCallback, BuildStatus
from sqlbuild.executor.custom.models import MaterializationResult
from sqlbuild.executor.load.models import LoadExecutionResult
from sqlbuild.executor.python_nodes.types import PythonIdentityRecorder
from sqlbuild.executor.run.models import MicrobatchBatchProgress, ModelExecutionResult
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.executor.testing.models import SqlTestExecutionResult
from sqlbuild.microbatches.models import MicrobatchScope
from sqlbuild.microbatches.types import MicrobatchEventStore
from sqlbuild.provider.main.runtime import ProviderContainer
from sqlbuild.python_nodes.models import SqlResourceRef
from sqlbuild.runtime.contracts.types import ConnectionElapsedCallback, NodeStartCallback
from sqlbuild.runtime.execution_limits.models import RunItemTimeouts


@dataclass(frozen=True)
class BuildExecutionTimings:
    """Monotonic phase durations for one build pipeline execution."""

    connection_preparation_seconds: float | None = None
    schema_preparation_seconds: float | None = None
    execution_seconds: float | None = None


@dataclass(frozen=True)
class SchedulerState:
    """Observable concurrent scheduler frontier state."""

    running: int
    ready: int
    waiting: int
    limit: int
    aborted: int = 0


@dataclass(frozen=True)
class BuildRuntimeParams:
    """Runtime-invariant configuration for one build execution."""

    run_id: str
    runtime_dir: Path = Path("target")
    promotion_mode: TablePromotionMode | None = None
    query_change_tracking: bool | None = None
    snapshots: SnapshotsConfig | None = None
    allow_snapshot_schema_change: bool = False
    run_audits: bool = True
    run_tests: bool = True
    fail_fast: bool = False
    max_concurrency: int = 1
    loader_is_reload: bool = False
    start_cursor_ts: datetime | None = None
    end_cursor_ts: datetime | None = None
    start_cursor_int: int | None = None
    end_cursor_int: int | None = None
    target: str = ""
    effective_vars: dict[str, object] | None = None
    use_color: bool = False
    providers: ProviderContainer | None = None
    microbatch_concurrency: bool = False
    microbatch_unaccounted_partition_policy: str = "synthesize"
    microbatch_state_resolver: (
        Callable[[ModelPlanEntry, object], tuple[MicrobatchEventStore, MicrobatchScope]] | None
    ) = None
    microbatch_location_state_resolver: (
        Callable[
            [str, CompiledRelationLocation, str | None, object],
            tuple[MicrobatchEventStore, MicrobatchScope],
        ]
        | None
    ) = None
    microbatch_lease_check: Callable[[], None] | None = None
    project_relations: Mapping[SqlResourceRef, str] | None = None
    test_timeouts: RunItemTimeouts | None = None


@dataclass(frozen=True)
class BuildCallbacks:
    """Progress and lifecycle callbacks for one build execution."""

    on_progress: Callable[[str], None] | None = None
    on_sub_progress: Callable[[str], None] | None = None
    on_batch_complete: Callable[[MicrobatchBatchProgress], None] | None = None
    on_node_start: NodeStartCallback | None = None
    on_node_complete: Callable[[object], None] | None = None
    before_node_write: BeforeNodeWriteCallback | None = None
    on_connection_start: Callable[[int], None] | None = None
    on_connection_complete: ConnectionElapsedCallback | None = None
    on_connection_error: ConnectionElapsedCallback | None = None
    python_identity_recorder: PythonIdentityRecorder | None = None
    on_scheduler_state: Callable[[SchedulerState], None] | None = None
    on_statement_complete: Callable[[StatementExecutionTelemetry], None] | None = None
    on_migration_progress: Callable[[str], None] | None = None


@dataclass(frozen=True)
class BuildCustomizations:
    """User-supplied materializations, hooks, and loader functions."""

    custom_materializations: Mapping[str, Callable[..., MaterializationResult]] | None = None
    loader_functions: tuple[DiscoveredLoaderFunction, ...] = ()


@dataclass(frozen=True)
class BuildInitialState:
    """Pre-seeded scheduler state carried in from earlier lifecycle phases."""

    warehouse_relations: dict[str, RelationInfo] | None = None
    precompleted_keys: frozenset[CompiledObjectKey] = frozenset()
    initial_load_results: tuple[LoadExecutionResult, ...] = ()
    initial_failed_keys: frozenset[CompiledObjectKey] = frozenset()


@dataclass(frozen=True)
class SeedExecutionResult:
    """Outcome of one seed load."""

    seed_name: str
    status: ExecutionStatus
    duration_ms: int | None = None
    lifecycle_events: tuple[LifeCycleEvent, ...] = field(default_factory=tuple)
    error_code: str | None = None
    error_help: str | None = None
    error_message: str | None = None
    warning_messages: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class FunctionExecutionResult:
    """Outcome of one SQL function creation."""

    function_name: str
    status: ExecutionStatus
    function_kind: str
    duration_ms: int | None = None
    error_code: str | None = None
    error_help: str | None = None
    error_message: str | None = None
    warning_messages: tuple[str, ...] = field(default_factory=tuple)
    lifecycle_events: tuple[LifeCycleEvent, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class SourceAuditRunResult:
    """Outcome of running pending source audits for one model."""

    blocked: bool
    executed_source_names: tuple[str, ...] = field(default_factory=tuple)
    failed_source_names: tuple[str, ...] = field(default_factory=tuple)
    newly_blocked_keys: tuple[CompiledObjectKey, ...] = field(default_factory=tuple)
    audit_results: tuple[AuditExecutionResult, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class BuildIndexes:
    """Precomputed lookup structures for build execution loop."""

    model_entries_by_key: dict[CompiledObjectKey, ModelPlanEntry] = field(default_factory=dict)
    seed_entries_by_key: dict[CompiledObjectKey, SeedPlanEntry] = field(default_factory=dict)
    function_entries_by_key: dict[CompiledObjectKey, FunctionPlanEntry] = field(
        default_factory=dict
    )
    source_load_entries_by_key: dict[CompiledObjectKey, SourceLoadPlanEntry] = field(
        default_factory=dict
    )
    test_entries_by_key: dict[CompiledObjectKey, SqlTestPlanEntry] = field(default_factory=dict)
    source_audits_by_source: dict[str, tuple[AuditPlanEntry, ...]] = field(default_factory=dict)
    seed_audits_by_seed: dict[str, tuple[AuditPlanEntry, ...]] = field(default_factory=dict)
    model_audits_by_model: dict[str, tuple[AuditPlanEntry, ...]] = field(default_factory=dict)
    end_audits: tuple[AuditPlanEntry, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class NodeCompletion:
    """Result of a single node execution passed back from a worker to the scheduler."""

    key: CompiledObjectKey
    result: (
        ModelExecutionResult
        | SeedExecutionResult
        | FunctionExecutionResult
        | SqlTestExecutionResult
        | LoadExecutionResult
    )


@dataclass(frozen=True)
class ExternalSourceLoadResults:
    """Results and scheduler state from pre-connection external source loads."""

    results: tuple[LoadExecutionResult, ...]
    completed_keys: frozenset[CompiledObjectKey]
    failed_keys: frozenset[CompiledObjectKey]


@dataclass(frozen=True)
class BuildExecutionResult:
    """Aggregate outcome of a full build execution."""

    status: BuildStatus
    model_results: tuple[ModelExecutionResult, ...] = field(default_factory=tuple)
    seed_results: tuple[SeedExecutionResult, ...] = field(default_factory=tuple)
    function_results: tuple[FunctionExecutionResult, ...] = field(default_factory=tuple)
    load_results: tuple[LoadExecutionResult, ...] = field(default_factory=tuple)
    test_results: tuple[SqlTestExecutionResult, ...] = field(default_factory=tuple)
    source_audit_results: tuple[AuditExecutionResult, ...] = field(default_factory=tuple)
    end_audit_results: tuple[AuditExecutionResult, ...] = field(default_factory=tuple)
    success_count: int = 0
    failure_count: int = 0
    timeout_count: int = 0
    skipped_count: int = 0
    warning_count: int = 0
    insufficient_count: int = 0
    audit_result_projection_attempted_count: int = 0
    audit_result_projection_written_count: int = 0
    audit_result_projection_failed_count: int = 0
    timings: BuildExecutionTimings = field(default_factory=BuildExecutionTimings)
