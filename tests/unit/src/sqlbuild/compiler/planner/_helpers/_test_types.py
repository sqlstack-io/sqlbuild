from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path

from sqlbuild._native import (
    BackfillAction,
    ChangeKind,
    CompiledObjectKey,
    CompileSqlReference,
    CursorBounds,
    Fingerprint,
    InferredColumn,
    MaterializationType,
    MissingUpstream,
    OnSchemaChange,
    PlanAction,
    PlanReason,
    PlanWarning,
    ScenarioArtifactIdentity,
    ScenarioGraphPlan,
    ScenarioRelationMap,
    SchemaAction,
    SchemaFinding,
    SourceEntry,
    SourceLoadPlanEntry,
    SqlLexicalSyntax,
    SqlReferenceKind,
    WarningSeverity,
)
from sqlbuild.compiler.auditing.types import (
    AuditAttachmentKind,
    AuditRunScope,
)
from sqlbuild.compiler.compile.types import AttachedAuditTargetKind, CompiledProjectView
from tests.unit.src.sqlbuild.compiler.planner._helpers.sql_test_assembly._test_types import (
    PlanTestChainTestCase as PlanTestChainTestCase,
)


@dataclass(frozen=True)
class FixturePlanningWorkTestCase:
    description: str
    test_count: int
    expected_metadata_builds: int
    expected_analysis_calls: int
    expected_topology_builds: int


@dataclass(frozen=True)
class CompleteFixturePlanningTestCase:
    description: str
    fixture_sql: str
    expected_analysis_calls: int
    expected_inference_calls: int
    expected_fixture_sql: str


@dataclass(frozen=True)
class CompleteModelChangesWorkTestCase:
    description: str
    expected_warnings: tuple[PlanWarning, ...]


@dataclass(frozen=True)
class BuildUpstreamDepsTestCase:
    description: str
    model_deps: dict[str, tuple[str, ...]]
    source_names: tuple[str, ...]
    seed_names: tuple[str, ...]
    expected_upstream_keys: dict[str, tuple[str, ...]]
    audit_model_source_deps: dict[str, tuple[str, ...]] = field(default_factory=dict)


@dataclass(frozen=True)
class SqlTestFunctionGraphDepsTestCase:
    description: str
    expected_test_upstream_keys: tuple[CompiledObjectKey, ...]


@dataclass(frozen=True)
class SourceColumnsTestCase:
    description: str
    source_entry: SourceEntry
    adapter_column_names: tuple[str, ...]
    expected_queried_sql: tuple[str, ...]
    expected_source_column_names: tuple[str, ...]


@dataclass(frozen=True)
class MultiDatabaseSourceColumnsTestCase:
    description: str
    expected_source_types: dict[str, str]
    expected_database_queries: tuple[str | None, ...]


@dataclass(frozen=True)
class KnownSourceColumnsReuseTestCase:
    description: str
    known_source_columns: dict[str, tuple[str, ...]] | None
    adapter_column_names: tuple[str, ...]
    expected_queried_sql_count: int
    expected_source_column_names: dict[str, tuple[str, ...]]


@dataclass(frozen=True)
class SourceMetadataClosureTestCase:
    description: str
    expected_source_names: tuple[str, ...]


@dataclass(frozen=True)
class MetadataNameFilterTestCase:
    description: str
    expected_physical_names: frozenset[str]


@dataclass(frozen=True)
class MicrobatchCursorEndPlanTestCase:
    description: str
    cursor_type: str
    cursor_grain: str | None
    authored_cursor_grain: dict[str, object]
    batch_size: str
    cursor_end: str
    target_max: str
    upstream_min: str
    upstream_max: str
    expected_microbatch_range: CursorBounds


@dataclass(frozen=True)
class SeedIdentityTestCase:
    description: str
    seed_contents: str
    comparison_contents: str
    expected_same_identity: bool
    seed_relative_path: Path = Path("sql/seeds/orders.csv")


@dataclass(frozen=True)
class SeedIdentityCsvConfigTestCase:
    description: str
    seed_contents: str
    expected_same_identity: bool


@dataclass(frozen=True)
class SelectionStalenessWarningTestCase:
    description: str
    upstream_key: CompiledObjectKey
    execution_selected_keys: frozenset[CompiledObjectKey]
    expected_warning_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SelectionStalenessGraphWarningTestCase:
    description: str
    upstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]
    selected_keys: frozenset[CompiledObjectKey]
    execution_selected_keys: frozenset[CompiledObjectKey]
    changed_model_names: frozenset[str]
    changed_seed_names: frozenset[str]
    changed_source_names: frozenset[str]
    expected_warning_fragments: tuple[str, ...]
    unexpected_warning_fragments: tuple[str, ...] = ()


@dataclass(frozen=True)
class ModelClosureTestCase:
    description: str
    expected_downstream_model_names: frozenset[str]


@dataclass(frozen=True)
class PlannerSourceFreshnessReadMapTestCase:
    description: str
    expected_observed_data_version: str


@dataclass(frozen=True)
class SourceLoadNodesTestCase:
    description: str
    expected_map_names: tuple[str, ...]
    expected_entries: tuple[SourceLoadPlanEntry, ...]


@dataclass(frozen=True)
class LoaderDagExpansionTestCase:
    description: str
    selected_names: frozenset[str]
    execute_dependency_names: frozenset[str]
    expected_selected_names: frozenset[str]
    expected_upstream_names: dict[str, tuple[str, ...]]
    expected_intermediate_source_names: tuple[str, ...]
    expected_intermediate_loader_flags: tuple[bool, ...]


@dataclass(frozen=True)
class LoaderDestinationPlanningTestCase:
    description: str
    destination: str
    expected_parts: tuple[str | None, str | None, str | None]


@dataclass(frozen=True)
class SourceCursorInputColumnsTestCase:
    description: str
    reference_kind: SqlReferenceKind
    reference_name: str
    cursor_column: str | None
    cursor_inputs: dict[str, object] | None
    source_columns: dict[str, tuple[str, ...]]
    expected_valid: bool
    expected_error_fragment: str | None = None
    upstream_contract: str | None = None
    upstream_declared_columns: tuple[str, ...] = ()
    upstream_inferred_columns: tuple[str, ...] | None = None

    @property
    def compiled_inferred_columns(self) -> tuple[InferredColumn, ...] | None:
        if self.upstream_inferred_columns is None:
            return None
        return tuple(InferredColumn(name=name) for name in self.upstream_inferred_columns)


@dataclass(frozen=True)
class WatermarkLineageTestCase:
    description: str
    watermark_name: str
    watermark_column: str
    expected_valid: bool
    expected_error_fragment: str | None = None


@dataclass(frozen=True)
class RuntimeCursorProducerNamesTestCase:
    description: str
    selected_keys: frozenset[CompiledObjectKey]
    source_map: dict[str, SourceEntry]
    expected_names: frozenset[str]


@dataclass(frozen=True)
class CursorRuntimeOwnershipTestCase:
    description: str
    is_model_backed: bool
    is_runtime_produced: bool
    expected_runtime_owned: bool


@dataclass(frozen=True)
class BuildDownstreamDepsTestCase:
    description: str
    upstream: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]
    expected_downstream_keys: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]


@dataclass(frozen=True)
class TopologicalOrderTestCase:
    description: str
    upstream: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]
    expected_order: tuple[CompiledObjectKey, ...]


@dataclass(frozen=True)
class CycleDetectionTestCase:
    description: str
    upstream: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]
    expected_error_type: type[Exception]
    injected_edge_origins: dict[tuple[CompiledObjectKey, CompiledObjectKey], str] | None = None
    expected_error_fragment: str | None = None


@dataclass(frozen=True)
class ExecutionEdgeOriginsTestCase:
    description: str
    model_deps: dict[str, tuple[str, ...]]
    source_names: tuple[str, ...]
    audit_model_source_deps: dict[str, tuple[str, ...]]
    expected_origin_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ExpandUpstreamTestCase:
    description: str
    upstream: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]
    key: CompiledObjectKey
    expected_keys: frozenset[CompiledObjectKey]


@dataclass(frozen=True)
class ExpandDownstreamTestCase:
    description: str
    downstream: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]
    key: CompiledObjectKey
    expected_keys: frozenset[CompiledObjectKey]


@dataclass(frozen=True)
class CheckBuildabilityTestCase:
    description: str
    selected_keys: frozenset[CompiledObjectKey]
    upstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]
    existing_relation_names: tuple[str, ...]
    expected_missing: tuple[MissingUpstream, ...]
    deferred_relation_names: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class ResolveModelPlanActionTestCase:
    description: str
    materialized: str
    incremental_strategy: str | None
    change_kind: ChangeKind
    query_changed: bool
    backfill_action: BackfillAction
    full_refresh: bool
    expected_action: PlanAction
    expected_reason: PlanReason
    schema_findings: tuple[SchemaFinding, ...] = field(default_factory=tuple)
    backfill_duration: str | None = None
    enabled: bool | None = None


@dataclass(frozen=True)
class IncrementalStrategyErrorTestCase:
    description: str
    materialized: str
    incremental_strategy: str | None
    change_kind: ChangeKind
    expected_error_type: type[Exception]
    expected_error_fragment: str


@dataclass(frozen=True)
class ResolveSchemaActionsTestCase:
    description: str
    schema_findings: tuple[SchemaFinding, ...]
    on_schema_change: OnSchemaChange | None
    expected_actions: tuple[SchemaAction, ...]


@dataclass(frozen=True)
class BuildModelWarningsTestCase:
    description: str
    model_name: str
    materialization_type: MaterializationType
    change_kind: ChangeKind
    query_changed: bool
    backfill_action: BackfillAction
    schema_findings: tuple[SchemaFinding, ...]
    schema_actions: tuple[SchemaAction, ...]
    on_schema_change: OnSchemaChange | None
    type_enforcement: bool
    expected_warning_count: int
    expected_severities: tuple[WarningSeverity, ...]


@dataclass(frozen=True)
class BuildPathIndexTestCase:
    description: str
    model_paths: dict[str, str]
    expected_folders: dict[str, str]


@dataclass(frozen=True)
class PlanScenarioGraphTestCase:
    description: str
    model_deps: dict[str, tuple[str, ...]]
    source_names: tuple[str, ...]
    seed_names: tuple[str, ...]
    expected_model_names: tuple[str, ...] = field(default_factory=tuple)
    assertion_sql_bodies: tuple[str, ...] = field(default_factory=tuple)
    source_fixture_names: tuple[str, ...] = field(default_factory=tuple)
    ref_fixture_names: tuple[str, ...] = field(default_factory=tuple)
    seed_fixture_names: tuple[str, ...] = field(default_factory=tuple)
    dbt_ref_fixture_names: tuple[str, ...] = field(default_factory=tuple)
    expected_plan: ScenarioGraphPlan | None = None


@dataclass(frozen=True)
class PlanScenarioGraphErrorTestCase:
    description: str
    model_deps: dict[str, tuple[str, ...]]
    source_names: tuple[str, ...]
    seed_names: tuple[str, ...]
    expected_model_names: tuple[str, ...] = field(default_factory=tuple)
    assertion_sql_bodies: tuple[str, ...] = field(default_factory=tuple)
    source_fixture_names: tuple[str, ...] = field(default_factory=tuple)
    ref_fixture_names: tuple[str, ...] = field(default_factory=tuple)
    seed_fixture_names: tuple[str, ...] = field(default_factory=tuple)
    dbt_ref_fixture_names: tuple[str, ...] = field(default_factory=tuple)
    expected_error_fragment: str = ""


@dataclass(frozen=True)
class ScenarioHashPrefixTestCase:
    description: str
    project_name: str
    scenario_name: str
    expected_hash_prefix: str
    run_namespace: str | None = None


@dataclass(frozen=True)
class ScenarioHashCollisionTestCase:
    description: str
    scenario_names: tuple[str, ...]
    prefix_length: int
    expected_error_fragment: str
    run_namespace: str | None = None


@dataclass(frozen=True)
class ScenarioArtifactNameTestCase:
    description: str
    hash_prefix: str
    kind: str
    logical_name: str
    identifier_limit: int
    expected_physical_name: str


@dataclass(frozen=True)
class ScenarioArtifactNameRecognitionTestCase:
    description: str
    physical_name: str
    expected_is_scenario_artifact: bool
    expected_hash_prefix: str | None = None
    expected_kind: str | None = None
    expected_logical_name: str | None = None


@dataclass(frozen=True)
class ScenarioRelationMapTestCase:
    description: str
    artifacts: tuple[ScenarioArtifactIdentity, ...]
    expected_relation_map: ScenarioRelationMap
    normalize_identifier: Callable[[str], str] | None = None


@dataclass(frozen=True)
class ScenarioCliPlanIdentifierLimitTestCase:
    description: str
    model_name: str
    expected_model_physical_name: str


@dataclass(frozen=True)
class ScenarioRelationMapErrorTestCase:
    description: str
    artifacts: tuple[ScenarioArtifactIdentity, ...]
    expected_error_fragment: str
    normalize_identifier: Callable[[str], str] | None = None


@dataclass(frozen=True)
class ScenarioRelationPlanTestCase:
    description: str
    graph_plan: ScenarioGraphPlan
    expected_model_target_names: dict[str, str]
    expected_seed_target_names: dict[str, str]
    expected_source_expressions: dict[str, str | None]
    expected_dbt_ref_target_names: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class ScenarioCheckSqlResolutionTestCase:
    description: str
    sql: str
    expected_sql: str


@dataclass(frozen=True)
class ScenarioDialectCheckSqlResolutionTestCase:
    description: str
    lexical_syntaxes: tuple[SqlLexicalSyntax, ...]
    sql: str
    expected_sql: str


@dataclass(frozen=True)
class ScenarioCheckSqlResolutionErrorTestCase:
    description: str
    lexical_syntaxes: tuple[SqlLexicalSyntax, ...]
    sql: str
    expected_error_code: str
    expected_error_fragment: str


@dataclass(frozen=True)
class ScenarioFixtureSqlResolutionErrorTestCase:
    description: str
    lexical_syntax: SqlLexicalSyntax
    fixture_sql: str
    expected_error_code: str
    expected_error_fragment: str


@dataclass(frozen=True)
class ScenarioRelationPlanErrorTestCase:
    description: str
    graph_plan: ScenarioGraphPlan
    expected_error_fragment: str


@dataclass(frozen=True)
class ScenarioFixturePlanTestCase:
    description: str
    graph_plan: ScenarioGraphPlan
    expected_fixture_sql: dict[str, str]
    expected_fixture_targets: dict[str, str]
    fixture_sql_body: str | None = None


@dataclass(frozen=True)
class ScenarioExecutionPlanTestCase:
    description: str
    graph_plan: ScenarioGraphPlan
    expected_model_entry_targets: dict[str, str]
    expected_model_entry_sql_fragments: dict[str, tuple[str, ...]]
    expected_fixture_targets: dict[str, str]
    expected_seed_entry_targets: dict[str, str]
    expected_function_entry_targets: dict[str, str]
    expected_function_entry_sql_fragments: dict[str, tuple[str, ...]]
    expected_expected_actual_destinations: dict[str, str]
    expected_expected_sql: dict[str, str]
    expected_assertion_sql: dict[str, str]
    expected_hook_names: tuple[str, ...] = ()


@dataclass(frozen=True)
class ScenarioUnmockedSeedExecutionPlanTestCase:
    description: str
    graph_plan: ScenarioGraphPlan
    project: CompiledProjectView
    expected_seed_fixture_names: frozenset[str]
    expected_seed_entry_targets: dict[str, str]


@dataclass(frozen=True)
class PlanAuditTestCase:
    description: str
    sql_body: str
    model_locations: dict[str, str]
    source_map_entries: dict[str, tuple[str | None, str, str | None]]
    expected_sql_fragment: str
    expected_error_fragment: str = ""
    always_run: bool = False
    expected_always_run: bool = False


@dataclass(frozen=True)
class PlanAttachedAuditTestCase:
    description: str
    attached_target_name: str
    referenced_model_names: tuple[str, ...]
    upstream_edges: dict[str, tuple[str, ...]]
    expected_attachment_kind: AuditAttachmentKind
    expected_reads_outside_target_lineage: bool


@dataclass(frozen=True)
class ApplyDeferredTargetsTestCase:
    description: str
    model_target_names: tuple[str, ...]
    seed_target_names: tuple[str, ...]
    deferred_target_names: tuple[str, ...]
    selected_key_names: tuple[str, ...]
    expected_model_qualified_names: dict[str, str | None]
    expected_seed_qualified_names: dict[str, str | None]


@dataclass(frozen=True)
class CursorOverrideResolutionTestCase:
    description: str
    cursor_type: str | None
    start_ts: str | None
    end_ts: str | None
    start_int: str | None
    end_int: str | None
    generic_start: str | None
    generic_end: str | None
    expected_start: str | None
    expected_end: str | None


@dataclass(frozen=True)
class MicrobatchLookbackTestCase:
    description: str
    incremental_strategy: str
    batch_size: str
    lookback: str | None
    expected_lookback: str | None
    effective_grain: str | None = None


@dataclass(frozen=True)
class MicrobatchLimitPlanningTestCase:
    description: str
    max_batches: int
    action: str
    expected_count: int | None
    expected_warning: bool
    plan_action: str = "incremental_delete_insert"
    range_start: str = "0"
    range_end: str = "30"
    cursor_type: str = "integer"
    batch_size: str = "10"
    expected_range_start: str = "0"
    expected_range_end: str = "30"


@dataclass(frozen=True)
class MicrobatchLimitPlanningErrorTestCase:
    description: str
    max_batches: int
    action: str
    expected_error_fragment: str


@dataclass(frozen=True)
class ConfiguredMicrobatchPolicyTestCase:
    description: str
    configured_limit: int
    configured_action: str
    is_cli_override: bool
    expected_limit: int
    expected_action: str
    expected_range_start: str
    expected_warning_count: int
    expected_safety_limit: int | None = None


@dataclass(frozen=True)
class EffectiveMicrobatchLimitPlanningTestCase:
    description: str
    batch_size: str
    expected_count: int
    expected_warning: bool


@dataclass(frozen=True)
class CursorOverridesValidationTestCase:
    description: str
    start_ts: str | None = None
    end_ts: str | None = None
    start_int: str | None = None
    end_int: str | None = None
    expected_valid: bool = True
    expected_error_fragment: str | None = None


@dataclass(frozen=True)
class CursorStartBoundsTestCase:
    description: str
    target_max: str | None
    upstream_mins: tuple[str, ...]
    upstream_maxes: tuple[str, ...]
    cursor_type: str
    cursor_start: str | None
    lookback: str | None
    backfill_duration: str | None
    start_cursor_override: str | None
    end_cursor_override: str | None
    expected_start: str | None
    expected_end: str | None


@dataclass(frozen=True)
class PlanEntryCursorOverrideTestCase:
    description: str
    start_cursor_override: str
    end_cursor_override: str
    expected_bounds: CursorBounds


@dataclass(frozen=True)
class AuthoritativeCursorOverrideTestCase:
    description: str
    cursor_type: str
    cursor_grain: str | None
    authored_cursor_grain: dict[str, object]
    batch_size: str
    start_override: str
    end_override: str
    expected_bounds: CursorBounds


@dataclass(frozen=True)
class PlannerOwnedMixedGrainRangeTestCase:
    description: str
    target_max: str
    upstream_max: str
    downstream_grain: str
    upstream_grain: str
    batch_size: str
    expected_bounds: CursorBounds


@dataclass(frozen=True)
class PlanEntryCursorGrainTestCase:
    description: str
    cursor_grain: str
    upstream_max: str
    expected_end: str


@dataclass(frozen=True)
class CursorTypeCheckTestCase:
    description: str
    cursor_column: str | None
    cursor_type: str | None
    warehouse_columns: tuple[tuple[str, str], ...]
    sql_analysis_enabled: bool
    expected_warning: bool
    expected_severity: WarningSeverity | None = None
    expected_message_fragment: str | None = None


@dataclass(frozen=True)
class DetectFunctionChangeTestCase:
    description: str
    body_sql: str
    existing_function_fingerprints: dict[str, Fingerprint]
    expected_reason: PlanReason
    target_schema: str = "main"


@dataclass(frozen=True)
class ResolveAttachmentTestCase:
    description: str
    references: tuple[CompileSqlReference, ...]
    attached_target_kind: AttachedAuditTargetKind | None
    attached_target_name: str | None
    upstream_edges: dict[str, tuple[str, ...]]
    expected_attachment_kind: AuditAttachmentKind
    expected_attached_name: str | None
    expected_reads_outside_target_lineage: bool = False


@dataclass(frozen=True)
class ResolveAttachmentErrorTestCase:
    description: str
    references: tuple[CompileSqlReference, ...]
    attached_target_kind: AttachedAuditTargetKind | None
    attached_target_name: str | None
    upstream_edges: dict[str, tuple[str, ...]]
    expected_error_fragment: str


@dataclass(frozen=True)
class ResolveEffectiveRunScopeTestCase:
    description: str
    requested_run_scope: AuditRunScope
    attached_model_materialization: str | None
    expected_effective_run_scope: AuditRunScope


@dataclass(frozen=True)
class PlannerStaleWarningScopeTestCase:
    description: str
    selected_model_name: str
    expected_inspected_names: frozenset[str]
    expected_execution_names: tuple[str, ...]


@dataclass(frozen=True)
class MissingUpstreamMessageTestCase:
    description: str
    missing: tuple[MissingUpstream, ...]
    edge_origins: dict[tuple[CompiledObjectKey, CompiledObjectKey], str]
    expected_message: str


@dataclass(frozen=True)
class LoaderDependencyNamesTestCase:
    """Expected first-seen loader names after resolving Python function dependencies."""

    description: str
    expected_names: tuple[str, ...]


@dataclass(frozen=True)
class RollingWindowProjectLimitTestCase:
    """A rolling-window entry's plan action and whether the project limit rejects its range."""

    description: str
    action: str
    reason: str
    expected_error_fragment: str | None
