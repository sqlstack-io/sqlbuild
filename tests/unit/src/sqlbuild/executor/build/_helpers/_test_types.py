from collections.abc import Callable
from dataclasses import dataclass

from sqlbuild._native import (
    DiscoveredLoaderFunction,
    MaterializationType,
    PlanAction,
    RetentionDirection,
    RetentionPlanPhase,
)
from sqlbuild.adapter.contract.models import RelationInfo
from sqlbuild.adapter.contract.types import RetentionChangePhase
from sqlbuild.executor.build.types import BuildStatus
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.runtime.contracts.types import ExecutionResourceKind


@dataclass(frozen=True)
class AuditExecutionIndexTestCase:
    description: str
    expected_model_audit_count: int
    expected_end_audit_count: int
    attachment_kind: str = "end"
    # Audits the plan selects by name, as audit: or a past run's failed audits select them.
    selected_audit_names: tuple[str, ...] = ()


@dataclass(frozen=True)
class LifecycleProgressTestCase:
    description: str
    expected_event_types: tuple[str, ...]


@dataclass(frozen=True)
class BuildSchedulerSourceLoadTestCase:
    description: str
    source_status: ExecutionStatus
    loader_factory: Callable[..., DiscoveredLoaderFunction]
    expected_load_status: ExecutionStatus
    expected_model_status: ExecutionStatus
    source_meta: dict[str, object]
    expected_resource_kind: ExecutionResourceKind
    expected_execution_order: tuple[str, ...] = ()
    expected_model_rows: tuple[tuple[object, ...], ...] = ()


@dataclass(frozen=True)
class BuildRetentionPhaseTestCase:
    description: str
    phase: RetentionPlanPhase
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class BuildModelRetentionReconciliationTestCase:
    description: str
    desired_days: int
    effective_days: int
    change_phase: RetentionChangePhase
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class FinalRetentionReconciliationTestCase:
    description: str
    planned_direction: RetentionDirection
    desired_days: int
    live_days: int
    expected_statements: tuple[str, ...]
    model_action: PlanAction = PlanAction.INCREMENTAL_MERGE


@dataclass(frozen=True)
class TableTypeConversionTestCase:
    description: str
    relation_snapshots: tuple[tuple[RelationInfo, ...], ...]
    expected_statements: tuple[str, ...]
    desired_type: str = "permanent"
    actual_type: str = "transient"


@dataclass(frozen=True)
class TableTypeConversionErrorTestCase:
    description: str
    relation_snapshots: tuple[tuple[RelationInfo, ...], ...]
    expected_error_fragment: str
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class BuildSchedulerModelHookTestCase:
    description: str
    hook_raises: bool
    expected_model_status: ExecutionStatus
    expected_events: tuple[str, ...]
    expected_model_rows: tuple[tuple[object, ...], ...] = ()


@dataclass(frozen=True)
class BuildSchedulerPreHookSkipTestCase:
    description: str
    expected_model_statuses: tuple[ExecutionStatus, ...]
    expected_execution_order: tuple[str, ...]


@dataclass(frozen=True)
class BuildSchedulerPlannedSkipTestCase:
    description: str
    expected_model_statuses: tuple[ExecutionStatus, ...]
    expected_build_status: BuildStatus
    expected_failure_count: int
    expected_skip_reason: str
    expected_execution_order: tuple[str, ...]


@dataclass(frozen=True)
class BuildSourceNodeExecutionTestCase:
    description: str
    source_name: str
    loader_name: str
    expected_progress_event: str
    expected_start_event: tuple[str, ExecutionResourceKind]
    expected_status: ExecutionStatus
    expected_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class BuildSourceNodeResultLocationTestCase:
    """Where a scheduled loader's result goes for an adapter default and a runtime location."""

    description: str
    adapter_default_schema: str | None
    runtime_schema: str
    expected_result_schemas: tuple[str, ...]


@dataclass(frozen=True)
class MaterializationRecreatesRelationTestCase:
    description: str
    materialization_type: MaterializationType
    action: PlanAction
    incremental_mode: str | None
    expected_recreates: bool


@dataclass(frozen=True)
class BatchedRetentionReconciliationTestCase:
    description: str
    entry_request_ids: tuple[str, ...]
    live_days: int
    desired_days: int
    expected_batch_request_ids: tuple[tuple[str, ...], ...]
    expected_single_inspections: tuple[str, ...]
    expected_statements: tuple[str, ...]


@dataclass(frozen=True)
class DownstreamBlockedKeysTestCase:
    description: str
    failed_name: str
    downstream_deps: dict[str, tuple[str, ...]]
    selected_names: frozenset[str]
    expected_names: frozenset[str]
