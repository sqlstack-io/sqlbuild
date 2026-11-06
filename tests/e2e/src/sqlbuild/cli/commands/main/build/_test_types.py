"""Test types for build e2e tests."""

from __future__ import annotations

import subprocess
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import NamedTuple


@dataclass(frozen=True)
class AuditFactoryFailureBuildE2ETestCase:
    description: str
    expected_exit_code: int
    expected_rows: tuple[tuple[int, int], ...]


@dataclass(frozen=True)
class ExecutionLimitBuildE2ETestCase:
    description: str
    expected_exit_code: int
    expected_tables: bool


@dataclass(frozen=True)
class MeasurementAuditCliE2ETestCase:
    description: str
    repo_files: dict[str, str]
    command: tuple[str, ...]
    expected_exit_code: int
    expected_output: str


@dataclass(frozen=True)
class BuildE2ETestCase:
    """Test case for sqb build e2e verification."""

    description: str
    expected_exit_code: int
    expected_table_names: tuple[str, ...]
    expected_view_names: tuple[str, ...]
    expected_seed_names: tuple[str, ...]
    expected_fact_orders_data: tuple[tuple[object, ...], ...] = field(default_factory=tuple)
    expected_fact_orders_python_udf_data: tuple[tuple[object, ...], ...] = field(
        default_factory=tuple
    )
    expected_customer_orders_table_function_data: tuple[tuple[object, ...], ...] = field(
        default_factory=tuple
    )
    expected_dim_customers_data: tuple[tuple[object, ...], ...] = field(default_factory=tuple)
    expected_waffle_types_data: tuple[tuple[object, ...], ...] = field(default_factory=tuple)
    expected_daily_revenue_data: tuple[tuple[object, ...], ...] = field(default_factory=tuple)
    expected_daily_order_partitioned_data: tuple[tuple[object, ...], ...] = field(
        default_factory=tuple
    )


@dataclass(frozen=True)
class TableFunctionDependencyBuildE2ETestCase:
    description: str
    function_argument: int
    expected_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class ReusableModelSchemaBuildE2ETestCase:
    description: str
    expected_rows: tuple[tuple[int, str], ...]


@dataclass(frozen=True)
class SeedWatermarkBuildE2ETestCase:
    description: str
    initial_seed: str
    changed_seed: str
    expected_initial_rows: tuple[tuple[object, ...], ...]
    expected_changed_rows: tuple[tuple[object, ...], ...]
    expected_plan_bounds: str


@dataclass(frozen=True)
class LoaderWatermarkBuildE2ETestCase:
    description: str
    initial_maximum: int
    changed_maximum: int
    expected_initial_rows: tuple[tuple[object, ...], ...]
    expected_changed_rows: tuple[tuple[object, ...], ...]
    expected_plan_bounds: str


@dataclass(frozen=True)
class CappedMicrobatchBuildE2ETestCase:
    description: str
    limit_action: str
    expected_first_ids: tuple[int, ...]
    expected_final_ids: tuple[int, ...]
    run_count: int


@dataclass(frozen=True)
class CappedMicrobatchScenarioE2ETestCase:
    description: str
    expected_exit_code: int


@dataclass(frozen=True)
class CappedFilterJoinE2ETestCase:
    """Expected rows from a consumer that filters a capped producer and joins its source table."""

    description: str
    limit_action: str
    expected_ids: tuple[int, ...]


@dataclass(frozen=True)
class CappedWatermarkRejectionE2ETestCase:
    """Expected static rejection for a capped watermark producer."""

    description: str
    expected_exit_code: int
    expected_error_fragment: str
    expected_absent_relations: tuple[str, ...]


@dataclass(frozen=True)
class CalendarGrainCursorBoundaryE2ETestCase:
    """Expected capped daily suffix from a calendar-grain producer maximum."""

    description: str
    expected_batch_starts: tuple[str, ...]
    expected_exclusive_end: str


@dataclass(frozen=True)
class EnumContractBuildE2ETestCase:
    """Test case for enum-backed contract enforcement."""

    description: str
    selected_member: str
    expected_exit_code: int
    expected_stdout_fragment: str


@dataclass(frozen=True)
class ManifestArtifactGatingE2ETestCase:
    description: str
    expected_manifest_after_build: bool
    expected_manifest_after_compile_manifest: bool
    expected_manifest_after_build_manifest: bool


@dataclass(frozen=True)
class BuildNoTestsNoAuditsFlagE2ETestCase:
    """Test case for build test/audit opt-out flags."""

    description: str
    project_name: str
    command: tuple[str, ...]
    expected_stdout_fragments: tuple[str, ...]
    unexpected_stdout_fragments: tuple[str, ...] = field(default_factory=tuple)
    expected_exit_code: int = 0


@dataclass(frozen=True)
class DeferCloneBuildE2ETestCase:
    """Test case for direct-mode build defer clone behavior."""

    description: str
    project_name: str
    initial_upstream_sql: str
    changed_upstream_sql: str
    downstream_sql: str
    prod_build_command: tuple[str, ...]
    dev_build_command: tuple[str, ...]
    expected_stdout_fragments: tuple[str, ...]
    unexpected_stdout_fragments: tuple[str, ...]
    expected_prod_upstream_rows: tuple[tuple[object, ...], ...]
    expected_dev_upstream_rows: tuple[tuple[object, ...], ...]
    expected_dev_downstream_rows: tuple[tuple[object, ...], ...]
    expected_fingerprint_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SelectionAwareStalenessBuildE2ETestCase:
    description: str
    project_name: str
    initial_command: tuple[str, ...]
    mixed_command: tuple[str, ...]
    replan_command: tuple[str, ...]
    expected_mixed_stdout_fragments: tuple[str, ...]
    expected_replan_stdout_fragments: tuple[str, ...]
    unexpected_replan_stdout_fragments: tuple[str, ...]
    expected_c_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class PythonBuildE2ETestCase:
    """Test case for direct build Python-node behavior."""

    description: str
    expected_exit_code: int
    expected_execution_fragments: tuple[str, ...]
    expected_table_names: tuple[str, ...]
    expected_notify_text: str
    expected_fact_orders_rows: tuple[tuple[object, ...], ...]
    expected_asset_payload: dict[str, object]
    expected_asset_materialized: str


@dataclass(frozen=True)
class PythonPersistedResultBuildE2ETestCase:
    """Test case for direct build persisted Python-node results."""

    description: str
    expected_exit_code: int
    expected_consumed_text: str
    expected_success_values: tuple[int, ...]
    expected_failed_status: str


@dataclass(frozen=True)
class PythonLoaderPersistedResultBuildE2ETestCase:
    """Test case for persisted direct-mode loader result summaries."""

    description: str
    expected_exit_code: int
    expected_loader_text: str


@dataclass(frozen=True)
class PythonLoaderStatusResultBuildE2ETestCase:
    """Test case for persisted failed/skipped direct-mode loader results."""

    description: str
    project_name: str
    repo_files: dict[str, str]
    command: tuple[str, ...]
    expected_exit_code: int
    expected_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class PythonTargetIsolationBuildE2ETestCase:
    """Test case for target-scoped persisted Python-node reads."""

    description: str
    expected_exit_code: int
    expected_consumed_text: str
    expected_target_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class DirectPythonBuildHardeningE2ETestCase:
    """Test case for direct build Python lifecycle hardening behavior."""

    description: str
    project_name: str
    command: tuple[str, ...]
    repo_files: dict[str, str]
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]
    expected_absent_tables: tuple[str, ...] = field(default_factory=tuple)
    expected_present_tables: tuple[str, ...] = field(default_factory=tuple)
    expected_markers: tuple[tuple[str, str], ...] = field(default_factory=tuple)
    expected_absent_paths: tuple[str, ...] = field(default_factory=tuple)
    expected_json_assets: tuple[tuple[str, str], ...] = field(default_factory=tuple)
    expected_json_status: str | None = None


@dataclass(frozen=True)
class PythonHooksBuildE2ETestCase:
    """Test case for build-time Python lifecycle hooks."""

    description: str
    expected_exit_code: int
    expected_orders_rows: tuple[tuple[object, ...], ...]
    expected_hook_log_rows: tuple[tuple[object, ...], ...]
    expected_output_fragments: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class PythonHookFailureBuildE2ETestCase:
    """Test case for build-time Python lifecycle hook failures."""

    description: str
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]
    expected_present_tables: tuple[str, ...]
    expected_absent_tables: tuple[str, ...]
    model_sql: str | None = None
    hook_files: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class PythonHookSkipBuildE2ETestCase:
    """Test case for build-time Python lifecycle hook skips."""

    description: str
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]
    expected_present_tables: tuple[str, ...]
    expected_absent_tables: tuple[str, ...]
    expected_rows: tuple[tuple[object, ...], ...] = ()


@dataclass(frozen=True)
class LongPythonHookNameBuildE2ETestCase:
    """Test case for CLI display of long Python hook names."""

    description: str
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]
    unexpected_output_fragments: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class SnapshotPythonHooksBuildE2ETestCase:
    """Test case for snapshot builds with Python lifecycle hooks."""

    description: str
    expected_exit_code: int
    expected_snapshot_rows: tuple[tuple[object, ...], ...]
    expected_hook_log_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class PythonHooksLifecycleMatrixBuildE2ETestCase:
    """Test case for build-time Python hooks across materialization kinds."""

    description: str
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]
    expected_hook_log_rows: tuple[tuple[object, ...], ...]
    expected_query_results: tuple[tuple[str, tuple[tuple[object, ...], ...]], ...]


@dataclass(frozen=True)
class DirectChangesOnlyBuildE2ETestCase:
    """Test case for direct build changes-only behavior."""

    description: str
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]
    unexpected_output_fragments: tuple[str, ...] = field(default_factory=tuple)
    expected_query_results: tuple[tuple[object, ...], ...] = field(default_factory=tuple)
    expected_failed_sql_fragment: str | None = None


@dataclass(frozen=True)
class ModelBackedCursorBuildE2ETestCase:
    """Test case for model-backed cursor build e2e regression coverage."""

    description: str
    repo_files: dict[str, str]
    command: tuple[str, ...]
    expected_exit_code: int
    expected_table_names: tuple[str, ...]
    expected_query_results: tuple[tuple[str, tuple[tuple[object, ...], ...]], ...]
    expected_absent_runtime_fragments: tuple[str, ...] = field(default_factory=tuple)
    expected_full_refresh_exit_code: int = 0
    expected_full_refresh_error_fragment: str | None = None
    expected_full_refresh_counts: tuple[tuple[str, int], ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class AppendCursorBuildE2ETestCase:
    """Test case for append cursor lower-bound behavior across reruns."""

    description: str
    repo_files: dict[str, str]
    initial_seed_sql: str
    mutation_sql: tuple[str, ...]
    command: tuple[str, ...]
    expected_exit_code: int
    expected_runtime_sql_fragment: str
    expected_query_results: tuple[tuple[str, tuple[tuple[object, ...], ...]], ...]


@dataclass(frozen=True)
class TimestampCursorBuildOverrideE2ETestCase:
    """Test case for timestamp cursor override behavior in sqb build."""

    description: str
    repo_files: dict[str, str]
    initial_seed_sql: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_runtime_sql_fragment: str
    expected_query_results: tuple[tuple[str, tuple[tuple[object, ...], ...]], ...]


@dataclass(frozen=True)
class SnapshotTimestampBuildE2ETestCase:
    """Test case for timestamp snapshot build behavior across CLI reruns."""

    description: str
    repo_files: dict[str, str]
    initial_seed_sql: str
    mutation_sql: tuple[str, ...]
    command: tuple[str, ...]
    expected_exit_code: int
    expected_query: str
    expected_initial_rows: tuple[tuple[object, ...], ...]
    expected_changed_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SnapshotTimestampFailureBuildE2ETestCase:
    """Test case for timestamp snapshot build failures through the CLI."""

    description: str
    repo_files: dict[str, str]
    initial_seed_sql: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SnapshotDmlFailureRollbackBuildE2ETestCase:
    """Test case for snapshot DML failure rollback behavior."""

    description: str
    repo_files: dict[str, str]
    initial_seed_sql: str
    mutation_sql: tuple[str, ...]
    command: tuple[str, ...]
    expected_initial_exit_code: int
    expected_failure_exit_code: int
    expected_error_fragments: tuple[str, ...]
    expected_query: str
    expected_rows_after_failure: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SnapshotCheckBuildE2ETestCase:
    """Test case for check snapshot build behavior across CLI reruns."""

    description: str
    repo_files: dict[str, str]
    initial_seed_sql: str
    mutation_sql: tuple[str, ...]
    command: tuple[str, ...]
    expected_exit_code: int
    expected_query: str
    expected_initial_rows: tuple[tuple[object, ...], ...]
    expected_changed_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SnapshotHistoricalCheckBuildE2ETestCase:
    """Test case for historical check snapshot build behavior across CLI reruns."""

    description: str
    repo_files: dict[str, str]
    initial_seed_sql: str
    mutation_sql: tuple[str, ...]
    command: tuple[str, ...]
    expected_exit_code: int
    expected_query: str
    expected_initial_rows: tuple[tuple[object, ...], ...]
    expected_changed_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SnapshotHistoricalTimestampBuildE2ETestCase:
    """Test case for historical timestamp snapshot build behavior across CLI reruns."""

    description: str
    repo_files: dict[str, str]
    initial_seed_sql: str
    mutation_sql: tuple[str, ...]
    command: tuple[str, ...]
    expected_exit_code: int
    expected_query: str
    expected_initial_rows: tuple[tuple[object, ...], ...]
    expected_changed_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SnapshotCheckFailureBuildE2ETestCase:
    """Test case for check snapshot build failures through the CLI."""

    description: str
    repo_files: dict[str, str]
    initial_seed_sql: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SnapshotFullRefreshFailureBuildE2ETestCase:
    """Test case for snapshot full-refresh safety failures through the CLI."""

    description: str
    repo_files: dict[str, str]
    initial_seed_sql: str
    initial_command: tuple[str, ...]
    full_refresh_command: tuple[str, ...]
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SnapshotFullRefreshSuccessBuildE2ETestCase:
    """Test case for confirmed snapshot full-refresh behavior through the CLI."""

    description: str
    repo_files: dict[str, str]
    initial_seed_sql: str
    mutation_sql: tuple[str, ...]
    initial_command: tuple[str, ...]
    full_refresh_command: tuple[str, ...]
    expected_exit_code: int
    expected_query: str
    expected_initial_rows: tuple[tuple[object, ...], ...]
    expected_refreshed_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SnapshotWaffleShopRerunBuildE2ETestCase:
    """Test case for a shallow multi-source snapshot rerun project."""

    description: str
    repo_files: dict[str, str]
    initial_seed_sql: str
    mutation_sql_by_round: tuple[tuple[str, ...], ...]
    command: tuple[str, ...]
    expected_exit_code: int
    expected_query_results_by_round: tuple[
        tuple[tuple[str, tuple[tuple[object, ...], ...]], ...], ...
    ]


@dataclass(frozen=True)
class SnapshotSelectorBuildE2ETestCase:
    """Test case for snapshot dependency selector behavior through the CLI."""

    description: str
    repo_files: dict[str, str]
    initial_seed_sql: str
    mutation_sql: tuple[str, ...]
    excluded_downstream_command: tuple[str, ...]
    downstream_command: tuple[str, ...]
    expected_exit_code: int
    expected_snapshot_query: str
    expected_snapshot_rows_after_excluded_downstream: tuple[tuple[object, ...], ...]
    expected_downstream_query: str
    expected_downstream_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SnapshotHookBuildE2ETestCase:
    """Test case for snapshot hook behavior through the CLI."""

    description: str
    repo_files: dict[str, str]
    initial_seed_sql: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_hook_query: str
    expected_hook_rows: tuple[tuple[object, ...], ...]
    expected_snapshot_query: str
    expected_snapshot_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class SnapshotFailureConsistencyBuildE2ETestCase:
    """Test case for snapshot consistency after failure paths."""

    description: str
    repo_files: dict[str, str]
    initial_seed_sql: str
    mutation_sql: tuple[str, ...]
    command: tuple[str, ...]
    expected_initial_exit_code: int
    expected_failure_exit_code: int
    expected_output_fragments: tuple[str, ...]
    expected_snapshot_query: str
    expected_rows_after_failure: tuple[tuple[object, ...], ...]
    recovery_sql: tuple[str, ...] = field(default_factory=tuple)
    expected_rows_after_recovery: tuple[tuple[object, ...], ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class TemplateExpressionsBuildE2ETestCase:
    """Test case for config-side template expression helpers."""

    description: str
    repo_files: dict[str, str]
    initial_seed_sql: str
    mutation_sql: tuple[str, ...]
    env: dict[str, str]
    command: tuple[str, ...]
    expected_exit_code: int
    expected_runtime_sql_fragment: str
    expected_query_results: tuple[tuple[str, tuple[tuple[object, ...], ...]], ...]


@dataclass(frozen=True)
class ExpressionSourceBuildE2ETestCase:
    """Test case for expression-backed source build coverage."""

    description: str
    repo_files: dict[str, str]
    command: tuple[str, ...]
    expected_exit_code: int
    expected_table_names: tuple[str, ...]
    expected_query_results: tuple[tuple[str, tuple[tuple[object, ...], ...]], ...]
    expected_runtime_fragments: tuple[str, ...]


@dataclass(frozen=True)
class QueryChangeTrackingBuildE2ETestCase:
    """Test case for query-change tracking e2e regression coverage."""

    description: str
    repo_files: dict[str, str]
    build_command: tuple[str, ...]
    plan_command: tuple[str, ...]
    expected_exit_code: int
    expected_fingerprint_models: tuple[str, ...]
    expected_unchanged_models: tuple[str, ...]


@dataclass(frozen=True)
class CliFailureBuildE2ETestCase:
    """Test case for expected CLI validation/build failures."""

    description: str
    repo_files: dict[str, str]
    command: tuple[str, ...]
    expected_exit_code: int
    expected_stderr_fragments: tuple[str, ...]
    verification_sql: str | None = None
    expected_verification_rows: tuple[tuple[object, ...], ...] = field(default_factory=tuple)
    expected_stdout_fragments: tuple[str, ...] = field(default_factory=tuple)
    pre_commands: tuple[tuple[str, ...], ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class QueryPropagationBuildE2ETestCase:
    """Test case for query-change propagation behavior across repeated CLI runs."""

    description: str
    repo_files: dict[str, str]
    initial_build_command: tuple[str, ...]
    plan_command: tuple[str, ...]
    mutation_file: str
    before_text: str
    after_text: str
    expected_exit_code: int
    expected_reasons: dict[str, str]
    expected_actions: dict[str, str] = field(default_factory=dict)
    expected_fingerprint_models: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class SelectorSurfaceBuildE2ETestCase:
    """Test case for selector surface CLI behavior."""

    description: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_stdout_fragments: tuple[str, ...]
    expected_stderr_fragments: tuple[str, ...]
    pre_commands: tuple[tuple[str, ...], ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class BuildRunContextOutputE2ETestCase:
    """Test case for verbose build context output modes."""

    description: str
    command: tuple[str, ...]
    expected_context_fragments: tuple[str, ...]


@dataclass(frozen=True)
class RuntimeArtifactPreservationBuildE2ETestCase:
    """Test case for runtime artifact preservation behavior."""

    description: str
    initial_command: tuple[str, ...]
    rerun_command: tuple[str, ...]
    expected_runtime_paths: tuple[str, ...]
    expected_exit_code: int
    expected_compiler_cache_count: int
    expected_local_history: bool
    expected_compiled_paths: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class LifecycleCommandsBuildE2ETestCase:
    """Test case for core lifecycle command behavior."""

    description: str
    expected_exit_code: int
    expected_fresh_plan_fragments: tuple[str, ...]
    expected_test_fragment: str
    expected_audit_fragment: str
    expected_run_fragment: str
    expected_rerun_reasons: dict[str, str]
    expected_full_refresh_fragment: str
    expected_plan_ordered_fragments: tuple[str, ...] = field(default_factory=tuple)
    expected_build_ordered_fragments: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class SchemaBackfillBuildE2ETestCase:
    """Test case for schema/backfill mutation behavior."""

    description: str
    mutate_model_file: str
    model_before_text: str
    model_after_text: str
    mutate_schema_file: str
    schema_before_text: str
    schema_after_text: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_plan_exit_code: int
    expected_reason: str
    expected_backfill_action: str | None = None
    expected_backfill_duration: str | None = None
    expected_warning_entries: tuple[tuple[str, str], ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class MixedTimestampGrainBuildE2ETestCase:
    """Test case for mixed timestamp grain replay behavior."""

    description: str
    repo_files: dict[str, str]
    initial_command: tuple[str, ...]
    rerun_command: tuple[str, ...]
    expected_exit_code: int
    expected_window_fragment: str
    expected_row_count: int


@dataclass(frozen=True)
class ConcurrentMicrobatchBehaviorE2ETestCase:
    """One multi-run concurrent microbatch lifecycle expectation."""

    description: str
    expected_exit_code: int = 0


@dataclass(frozen=True)
class StatelessSerialMicrobatchE2ETestCase:
    """Expected stateless serial behavior for one project capability setting."""

    description: str
    settings_toml: str
    expected_complexity_warning_count: int
    expected_state_table_count: int


@dataclass(frozen=True)
class MicrobatchReconciliationPolicyE2ETestCase:
    """Expected state facts after rebuilding with missing completion history."""

    description: str
    policy: str
    expected_synthetic: int
    expected_minimum_recovery: int
    expected_warning_fragment: str
    expected_fingerprint_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class AuditFailureBuildE2ETestCase:
    """Test case for audit failure CLI behavior."""

    description: str
    repo_files: dict[str, str]
    command: tuple[str, ...]
    expected_exit_code: int
    expected_failure_fragment: str
    expected_retained_relation_fragment: str


@dataclass(frozen=True)
class CompileJsonRaisedErrorE2ETestCase:
    """Expected JSON report for a compile that stops on a raised error."""

    description: str
    repo_files: dict[str, str]
    expected_code: str
    expected_message_fragment: str
    expected_help_fragment: str | None


@dataclass(frozen=True)
class CompileJsonBuildE2ETestCase:
    """Test case for compile JSON behavior."""

    description: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_model_names: tuple[str, ...]
    expected_sql_fragments: tuple[str, ...]
    expected_warning_count: int = 0
    expected_diagnostic_codes: tuple[str, ...] = field(default_factory=tuple)
    expected_pre_hooks: tuple[dict[str, object], ...] = field(default_factory=tuple)
    expected_post_hooks: tuple[dict[str, object], ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class DagJsonBuildE2ETestCase:
    """Test case for dag JSON behavior."""

    description: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_project_name: str
    expected_node_ids: tuple[str, ...]
    expected_edge_pairs: tuple[tuple[str, str], ...]
    expected_check_ids: tuple[str, ...]
    expected_absent_fragments: tuple[str, ...]


@dataclass(frozen=True)
class DiamondDagE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_fragments: tuple[str, ...]
    expected_max_lines_per_node: int


@dataclass(frozen=True)
class DiamondDagJsonE2ETestCase:
    description: str
    expected_node_ids: tuple[str, ...]
    expected_edge_count: int


@dataclass(frozen=True)
class PlanCommandBuildE2ETestCase:
    """Test case for plan command surface behavior."""

    description: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_stdout_fragments: tuple[str, ...]
    expected_stderr_fragments: tuple[str, ...]


@dataclass(frozen=True)
class RemoveColumnSemanticsBuildE2ETestCase:
    """Test case for remove-column mutation semantics."""

    description: str
    mutate_file: str
    before_text: str
    after_text: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_reason: str
    expected_backfill_action: str
    expected_backfill_duration: str | None
    expected_warning_count: int


@dataclass(frozen=True)
class SnapshotReappearingKeyBuildE2ETestCase:
    """Test case for hard-deleted keys that reappear in historical check snapshots."""

    description: str
    repo_files: dict[str, str]
    initial_seed_sql: str
    observation_sql_by_round: tuple[tuple[str, ...], ...]
    history_query_template: str
    expected_incremental_rows_by_round: tuple[tuple[tuple[object, ...], ...], ...]
    expected_full_history_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class DynamicColumnContractE2ETestCase:
    description: str
    expected_rows: tuple[tuple[object, ...], ...]
    name_pattern_entry: str = ""


@dataclass(frozen=True)
class DynamicColumnRejectionE2ETestCase:
    """Test case for a dynamic column name pattern that fails a build."""

    description: str
    name_pattern_entry: str
    expected_output: str
    expected_returncode: int = 1


@dataclass(frozen=True)
class ReplayFullRebuildE2ETestCase:
    """Test case for full-history replay of an existing incremental model."""

    description: str
    target_table: str
    changed_model_path: str
    changed_model_sql: str
    rebuild_command: tuple[str, ...]
    expected_plan_fragments: tuple[str, ...]


@dataclass(frozen=True)
class WindowLimitedReplayE2ETestCase:
    """Test case for incremental runs that must stay window-limited."""

    description: str
    changed_model_sql: str
    source_mutation_sql: str
    expected_plan_fragments: tuple[str, ...]
    changed_rows_sql: str
    expected_changed_rows: int
    expected_total_rows: int


@dataclass(frozen=True)
class SqlHookIdentityBuildE2ETestCase:
    """Test case for model version identity after a SQL hook resource edit."""

    description: str
    removed_hook_path: str
    added_hook_path: str
    added_hook_contents: str
    expected_reason: str


@dataclass(frozen=True)
class PythonNodeIdentityBuildE2ETestCase:
    """Test case for Python node version identity after one decorator-input edit."""

    description: str
    edited_path: str
    original_text: str
    edited_text: str
    expected_asset_identity_status: str
    expected_loader_version_count: int


@dataclass(frozen=True)
class DroppedIncrementalRelationE2ETestCase:
    description: str
    incremental_strategy: str
    expected_rows: tuple[tuple[int, str], ...]


@dataclass(frozen=True)
class PythonHelperPackageIdentityE2ETestCase:
    """Test case for node identity after editing a relative-import helper package."""

    description: str
    edited_path: str
    original_text: str
    edited_text: str
    expected_identity_status: str
    expected_version_count: int


@dataclass(frozen=True)
class ScopedDeclarationBuildE2ETestCase:
    description: str
    expected_exit_code: int
    expected_checks: dict[tuple[str, str | None], str]
    expected_hook_rows: tuple[tuple[str, int], ...]


@dataclass(frozen=True)
class ScopedDeclarationScopeE2ETestCase:
    description: str
    target: str
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SeedAuditBuildE2ETestCase:
    description: str
    seed_csv: str
    expected_exit_code: int
    expected_status: str
    expected_dependent_table: bool


@dataclass(frozen=True)
class AttachedAuditGateBuildE2ETestCase:
    description: str
    target_kind: str
    target_name: str
    audit_name: str
    dependant_name: str
    order_code: str
    expected_exit_code: int
    expected_status: str
    expected_dependant_built: bool
    target_query: str
    expected_target_rows: tuple[tuple[str, ...], ...]
    concurrency: int = 1


@dataclass(frozen=True)
class AttachedAuditGateSummaryE2ETestCase:
    description: str
    target_kind: str
    order_code: str
    expected_exit_code: int
    expected_summary: str


@dataclass(frozen=True)
class AttachedAuditGateCycleE2ETestCase:
    description: str
    target_kind: str
    read: str
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class AttachedAuditGatePartialBuildE2ETestCase:
    description: str
    target_kind: str
    select: str
    replaced_read_sql: str
    expected_exit_code: int
    expected_check: tuple[str, str]
    unselected_asset: str


@dataclass(frozen=True)
class AttachedAuditGateNoAuditsE2ETestCase:
    description: str
    target_kind: str
    order_code: str
    target_query: str
    expected_target_rows: tuple[tuple[str, ...], ...]


@dataclass(frozen=True)
class NestedSourceGateE2ETestCase:
    description: str
    raw_code: str
    expected_exit_code: int
    expected_checks: dict[tuple[str, str], tuple[str, str]]
    expected_orders_built: bool


@dataclass(frozen=True)
class AuditExecutionErrorE2ETestCase:
    description: str
    audit_kind: str
    severity: str
    concurrency: int
    expected_attachment_kind: str
    expected_asset_statuses: dict[str, str]


@dataclass(frozen=True)
class AuditExecutionErrorTerminalE2ETestCase:
    description: str
    audit_kind: str
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class AuditReadPlanE2ETestCase:
    description: str
    select: str
    expected_error_fragment: str


@dataclass(frozen=True)
class ExplicitReferenceBuildE2ETestCase:
    """A build of a project using typed macro references and Python hook reads."""

    description: str
    overrides: dict[str, str]
    expected_exit_code: int
    expected_dag_edges: tuple[tuple[str, str], ...]
    expected_customer_counts: tuple[tuple[object, ...], ...]
    expected_order_before: tuple[str, str]


@dataclass(frozen=True)
class ExplicitReferencePythonDependencyE2ETestCase:
    """Python nodes declaring seed and model dependencies, built and checked end to end."""

    description: str
    select: tuple[str, ...]
    expected_dag_edges: tuple[tuple[str, str], ...]
    expected_checked_asset_ids: tuple[str, ...]
    expected_order_before: tuple[str, str]
    expected_country_counts: tuple[tuple[object, ...], ...]
    expected_check_row_pattern: str


@dataclass(frozen=True)
class CollectedCompileDiagnosticsE2ETestCase:
    """A project with several compile violations, all reported by one compile."""

    description: str
    overrides: dict[str, str]
    expected_output_fragments: tuple[str, ...]
    expected_json_diagnostics: tuple[tuple[str, str], ...]


@dataclass(frozen=True)
class ExplicitReferenceFailureE2ETestCase:
    """A project whose references are not explicit, and the failure it reports."""

    description: str
    overrides: dict[str, str]
    command: tuple[str, ...]
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ExplicitReferenceRuntimeWarningE2ETestCase:
    """A build whose Python SQL hard-codes a project relation only at run time."""

    description: str
    overrides: dict[str, str]
    enforce_explicit: bool
    expected_exit_code: int
    expected_warning_fragments: tuple[str, ...]
    expected_final_line_prefix: str


@dataclass(frozen=True)
class CrossBoundarySelectionE2ETestCase:
    """A both-way selector over tasks that read a model and, separately, its upstream."""

    description: str
    select: tuple[str, ...]
    expected_ran: tuple[str, ...]
    expected_not_ran: tuple[str, ...]


@dataclass(frozen=True)
class PythonSourceDeferralE2ETestCase:
    """Python reads of a managed source under a target that defers source reads."""

    description: str
    select: tuple[str, ...]
    expected_counts: dict[str, int]
    expected_check_pattern: str


@dataclass(frozen=True)
class PythonSourceTaskOnlyE2ETestCase:
    """A task selected alone whose declared source no selected SQL resource reads."""

    description: str
    target_config: str
    expected_task_count: int


@dataclass(frozen=True)
class UnrenderedMacroArgumentE2ETestCase:
    """A model passing typed references its macro does not render."""

    description: str
    model_sql: str
    expected_dag_edges: tuple[tuple[str, str], ...]
    expected_built: tuple[str, ...]


@dataclass(frozen=True)
class SqlHookReadBuildE2ETestCase:
    """SQL hooks whose references a build must resolve and order like model SQL."""

    description: str
    select: tuple[str, ...]
    expected_counts: dict[str, int]
    expected_order_before: tuple[str, str]


@dataclass(frozen=True)
class SqlHookReadCycleE2ETestCase:
    """A SQL hook that reads a model built from the model running the hook."""

    description: str
    overrides: dict[str, str]
    expected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class AwareCursorDateWatermarkE2ETestCase:
    description: str
    plan_args: tuple[str, ...]
    expected_plan_fragment: str
    expected_event_ids: tuple[int, ...]


@dataclass(frozen=True)
class PythonNodeSelectionE2ETestCase:
    """Python nodes selected with or without their SQL upstreams."""

    description: str
    select: tuple[str, ...]
    expected_exit_code: int
    expected_output_patterns: tuple[str, ...]
    expected_row_counts: dict[str, int]
    expected_node_counts: dict[str, int]
    expected_missing_tables: tuple[str, ...] = ()
    orders_sql: str = (
        'MODEL (description "Test model.", '
        'materialized table);\nSELECT order_id FROM __source("raw_orders")\n'
    )
    existing_sql: str = ""


@dataclass(frozen=True)
class QueryFingerprintEditE2ETestCase:
    """One authored edit and the plan reasons the token fingerprint must produce."""

    description: str
    edits: tuple[tuple[str, str, str], ...]
    format_flags: tuple[str, ...]
    expected_format_summary: str
    expected_reasons: dict[str, str]


@dataclass(frozen=True)
class TerminalBuildFailureE2ETestCase:
    description: str
    command: tuple[str, ...]
    model_replacements: tuple[tuple[str, str, str], ...]
    expected_screen_fragments: tuple[str, ...]
    columns: int = 120


@dataclass(frozen=True)
class EmptyCursorInputsFirstBuildE2ETestCase:
    """A first build over empty cursor inputs, followed by a build once data arrives."""

    description: str
    model_sql: str
    rows_sql: str
    expected_reason: str
    expected_rows_after_load: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class EmptyCursorInputsIncrementalE2ETestCase:
    """An incremental run after the cursor inputs were emptied."""

    description: str
    command: tuple[str, ...]
    expected_output_fragments: tuple[str, ...]
    expected_json_empty_inputs: tuple[str, ...] | None


@dataclass(frozen=True)
class CursorModelWithoutInputsCompileE2ETestCase:
    """A cursor-based incremental model that reads no inputs."""

    description: str
    command: tuple[str, ...]
    expected_output_fragments: tuple[str, ...]
    expected_json_location: tuple[int, int, int] | None


@dataclass(frozen=True)
class DeltaFailureRetentionE2ETestCase:
    """A real incremental build failure after the delta relation was created."""

    description: str
    failing_rows_sql: str
    expected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class EmptyCursorInputsRebuildRefusalE2ETestCase:
    """A rebuild that would replace a loaded destination with the result of empty inputs."""

    description: str
    model_sql: str
    changed_model_sql: str
    command: tuple[str, ...]
    rows_sql: str
    expected_rows: tuple[tuple[object, ...], ...]
    expected_output_fragments: tuple[str, ...]
    expected_json_error_code: str | None


@dataclass(frozen=True)
class WaitingOnEmptyInputsE2ETestCase:
    """An incremental run where one cursor input is empty while another has new rows."""

    description: str
    command: tuple[str, ...]
    expected_output_fragments: tuple[str, ...]
    expected_json_empty_inputs: tuple[str, ...] | None


@dataclass(frozen=True)
class EmptyRuntimeInputLabelE2ETestCase:
    """An empty cursor input rebuilt in the same run, labelled by its project name."""

    description: str
    changed_upstream_sql: str
    expected_output_fragments: tuple[str, ...]
    unexpected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SameRunEmptyInputsE2ETestCase:
    """Empty cursor inputs whose staging views are rebuilt earlier in the same run."""

    description: str
    emptying_sql: str
    command: tuple[str, ...]
    expected_output_fragments: tuple[str, ...]
    unexpected_output_fragments: tuple[str, ...]
    expected_json_empty_inputs: tuple[str, ...] | None
    expected_json_waiting_on_empty_inputs: bool | None
    expected_json_warnings: tuple[str, ...]


@dataclass(frozen=True)
class UntypedCursorOverrideSelectionE2ETestCase:
    """Untyped cursor flags over a selection that does or does not mix cursor types."""

    description: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ModelConfigEnginesBuildE2ETestCase:
    """Building a project whose models layer and validate config."""

    description: str
    overrides: dict[str, str]
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]
    expected_relations: tuple[tuple[str, str], ...]


@dataclass(frozen=True)
class WarehouseConnectionFailureE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_stderr_fragments: tuple[str, ...]
    extra_files: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class StandaloneAuditRowE2ETestCase:
    description: str
    status_values: tuple[str, ...]
    expected_exit_code: int
    expected_rows: tuple[str, ...]


@dataclass(frozen=True)
class SnapshotValidityColumnsTestCase:
    """A snapshot's validity column config and the column names its built relation holds."""

    description: str
    validity_config: str
    expected_columns: tuple[str, ...]


@dataclass(frozen=True)
class MaterializationTypeChangeTestCase:
    """A built model's materialization change and the relation type each build leaves."""

    description: str
    materializations: tuple[str, ...]
    expected_table_types: tuple[str, ...]


@dataclass(frozen=True)
class ViewToFullRefreshTestCase:
    """A view model switched to a build-aside materialization, then fully refreshed repeatedly."""

    description: str
    model_header: str
    full_refresh_count: int
    expected_relations: list[tuple[str, str]]
    expected_rows: list[tuple[object, ...]]


@dataclass(frozen=True)
class TargetConcurrencyE2ETestCase:
    description: str
    target_concurrency: str
    local_settings: str
    command: tuple[str, ...]
    expected_fragment: str


@dataclass(frozen=True)
class TargetMicrobatchWarningE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_warning_count: int


@dataclass(frozen=True)
class MicrobatchBatchProgressE2ETestCase:
    """Per-batch microbatch progress lines for captured and terminal output."""

    description: str
    run: Callable[..., subprocess.CompletedProcess[str]]
    expected_batches: tuple[tuple[str, str, str], ...]


@dataclass(frozen=True)
class DeferredMigrationOriginE2ETestCase:
    """A deferred reference to a renamed model under different deferred-target states."""

    description: str
    prod_sql: str
    expected_order_ids: tuple[int, ...]
    expect_note: bool


@dataclass(frozen=True)
class UnbuiltDependencyBuildE2ETestCase:
    """A build whose failed model leaves a reader that must be skipped, not run."""

    description: str
    repo_files: dict[str, str]
    expected_stdout_fragments: tuple[str, ...]
    unexpected_stdout_fragments: tuple[str, ...]
    build_arguments: tuple[str, ...] = ()


@dataclass(frozen=True)
class NoLoadBuildE2ETestCase:
    """A build with loading off over a managed source holding a NULL, and what it must do."""

    description: str
    second_build: tuple[str, ...]
    project_settings: str
    expected_exit_code: int
    expected_audit_reported: bool
    expected_order_rows: int = 2


@dataclass(frozen=True)
class HookSkippedAuditBuildE2ETestCase:
    """A rebuild where a hook skips a model that already exists, and its end audit's outcome."""

    description: str
    hooked_model: str
    expected_exit_code: int
    expected_stdout_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ConnectionStorageKeyBuildE2ETestCase:
    """A DuckDB connection key in the project config, and the storage the warehouse file gets."""

    description: str
    connection_key: str
    expected_storage_version: str


@dataclass(frozen=True)
class SourceEndAuditBuildE2ETestCase:
    """A build over an unread source whose table is missing, and whether its audit runs."""

    description: str
    build_arguments: tuple[str, ...]
    expected_exit_code: int
    expected_audit_reported: bool


@dataclass(frozen=True)
class PythonCheckBuildE2ETestCase:
    """A build whose Python check returns `check_body`, and the footer and exit it must end with."""

    description: str
    check_decorator: str
    check_body: str
    expected_exit_code: int
    expected_footer_fragment: str


@dataclass(frozen=True)
class DuckDbAttachE2ETestCase:
    """A DuckDB project attaching a second database file, built with some connections."""

    description: str
    concurrency: int
    expected_archived_rows: list[tuple[object, ...]]
    attach_path: str = "archive.duckdb"
    expected_archive_root: str = "project"


@dataclass(frozen=True)
class QuotedMixedCaseColumnsBuildE2ETestCase:
    """A source and a model output with quoted mixed-case columns, through every command."""

    description: str
    expected_export_rows: list[tuple[object, ...]]
    expected_test_fragment: str
    expected_scenario_fragment: str


@dataclass(frozen=True)
class QuotedContractE2ETestCase:
    """An enforced contract whose declared name the model produces as a quoted camelCase alias."""

    description: str
    declared_name: str
    expected_rows: list[tuple[object, ...]]


@dataclass(frozen=True)
class QuotedConfiguredColumnsE2ETestCase:
    """A snapshot key and a microbatch cursor configured with quoted camelCase names."""

    description: str
    unique_key: str
    cursor: str
    expected_history_rows: list[tuple[object, ...]]
    expected_activity_rows: list[tuple[object, ...]]


class ChangedWarehouse(NamedTuple):
    """One adapter's changed-builds project: configuration, raw tables and how to query it."""

    name: str
    project_toml: str
    warehouse_file: str
    raw_tables_sql: tuple[str, ...]
    consumption_table: str
    orders_table: str
    customers_table: str
    slow_scan_cte: str
    execute: Callable[[Path, str], list[tuple[object, ...]]]


class ChangedBuildStep(NamedTuple):
    """One mutation of the warehouse or project followed by one build."""

    description: str
    warehouse_sql: tuple[str, ...]
    project_files: dict[str, str]
    arguments: tuple[str, ...]


@dataclass(frozen=True)
class ChangedBuildE2ETestCase:
    description: str
    warehouse: ChangedWarehouse
    expected_ran: tuple[tuple[str, frozenset[str]], ...]


@dataclass(frozen=True)
class ChangedCrashE2ETestCase:
    description: str
    warehouse: ChangedWarehouse
    expected_ran: frozenset[str]
    expected_killed_rows: list[tuple[object, ...]]
