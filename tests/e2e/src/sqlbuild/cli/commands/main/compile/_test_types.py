"""Test case types for compile command performance guards."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING, NamedTuple

import pytest

from scripts.delta_compile_series.types import SequenceEdit
from tests.integration.src.sqlbuild.compiler.pipeline._test_types import PreparedCompile

if TYPE_CHECKING:
    from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import CompileReuseRun


@dataclass(frozen=True)
class SourceSemanticBindingCase:
    description: str
    command: str
    expected_code: str = "B002"


@dataclass(frozen=True)
class RelationStubNameCase:
    description: str
    stub_cte_name: str
    expected_downstream_type: str
    expected_ghost_column_diagnostics: frozenset[tuple[str, str]]


@dataclass(frozen=True)
class SemanticCorpusCase:
    description: str
    category: str
    repo_files: dict[str, str]
    expected_exit_code: int
    expected_codes: tuple[str, ...]
    pending_native: bool
    ignored_rules: tuple[str, ...] = ()


@dataclass(frozen=True)
class VariedCompileCacheTestCase:
    description: str
    model_count: int
    expected_cold_max_seconds: float
    expected_warm_max_seconds: float
    expected_edit_max_seconds: float
    expected_max_rss_bytes: int
    expected_min_operator_shapes: int
    expected_min_component: int
    expected_min_depth: int
    expected_max_depth: int
    expected_min_leaves: int
    expected_max_leaves: int
    expected_min_array_models: int
    expected_min_window_models: int
    expected_fingerprints: tuple[str, str, str, str]


@dataclass(frozen=True)
class CompilePerformanceGuardTestCase:
    description: str
    model_count: int
    expected_max_seconds: float


@dataclass(frozen=True)
class CompileScalingGuardTestCase:
    description: str
    model_count: int
    small_scan_event_lines_per_model: int
    large_scan_event_lines_per_model: int
    expected_min_small_sql_bytes: int
    expected_min_large_sql_bytes: int
    expected_small_scan_events: int
    expected_large_scan_events: int
    expected_max_seconds: float
    expected_max_scaling_ratio: float


@dataclass(frozen=True)
class DbtShapedCompilePerformanceGuardTestCase:
    description: str
    model_count: int
    expected_min_sql_bytes: int
    expected_max_sql_bytes: int
    expected_max_seconds: float
    expected_warm_max_seconds: float


@dataclass(frozen=True)
class SqlTestHeavyCompilePerformanceGuardTestCase:
    description: str
    model_count: int
    test_count: int
    chain_depth: int
    fixture_row_count: int
    expected_min_compiled_test_bytes: int
    expected_max_seconds: float
    expected_warm_max_seconds: float
    expected_edit_max_seconds: float


@dataclass(frozen=True)
class LayeredProductionCompilePerformanceGuardTestCase:
    description: str
    model_count: int
    source_count: int
    seed_count: int
    function_count: int
    macro_count: int
    test_count: int
    expected_audit_count: int
    expected_hook_count: int
    expected_min_model_sql_bytes: int
    expected_max_model_sql_bytes: int
    expected_min_compiled_test_bytes: int
    expected_max_compiled_test_bytes: int
    expected_cold_max_seconds: float
    expected_warm_max_seconds: float
    expected_edit_max_seconds: float
    expected_config_edit_max_seconds: float


@dataclass(frozen=True)
class LayeredProductionCompileTestCase:
    description: str
    model_count: int
    source_count: int
    seed_count: int
    function_count: int
    macro_count: int
    test_count: int
    audit_count: int
    expected_diagnostic_codes: tuple[str, ...]


@dataclass(frozen=True)
class SemanticCompilePerformanceGuardTestCase:
    description: str
    model_count: int
    source_count: int
    seed_count: int
    function_count: int
    macro_count: int
    test_count: int
    audit_count: int
    expected_min_declared_columns: int
    expected_max_declared_columns: int
    expected_min_model_sql_bytes: int
    expected_max_model_sql_bytes: int
    expected_min_compiled_test_bytes: int
    expected_max_compiled_test_bytes: int
    expected_cold_max_seconds: float
    expected_warm_max_seconds: float


@dataclass(frozen=True)
class FreshProcessCompilePerformanceGuardTestCase:
    description: str
    model_count: int
    source_count: int
    seed_count: int
    function_count: int
    macro_count: int
    test_count: int
    audit_count: int
    expected_max_wall_seconds: float
    expected_max_rss_bytes: int
    expected_semantic_fingerprint: str
    # `select = []`: the compiler alone, without the default SQL Rules.
    compiler_only: bool = False


@dataclass(frozen=True)
class FreshProcessCompileCachePerformanceGuardTestCase:
    description: str
    model_count: int
    source_count: int
    seed_count: int
    function_count: int
    macro_count: int
    test_count: int
    audit_count: int
    expected_cold_max_wall_seconds: float
    expected_warm_max_wall_seconds: float
    expected_edit_max_wall_seconds: float
    expected_max_warm_to_cold_ratio: float
    expected_max_edit_to_cold_ratio: float
    expected_max_cache_write_cpu_overhead_ratio: float
    expected_max_cache_write_wall_overhead_ratio: float
    expected_max_rss_bytes: int
    expected_max_cache_bytes: int
    expected_cold_fingerprint: str
    expected_leaf_edit_fingerprint: str
    expected_macro_edit_fingerprint: str
    expected_project_config_fingerprint: str
    macro_call_interval: int
    scoped_macros: bool
    expected_macro_edit_misses: int
    # `select = []`: the compiler alone, without the default SQL Rules.
    compiler_only: bool = False


@dataclass(frozen=True)
class NamespaceCompileTestCase:
    description: str
    repo_files: dict[str, str]
    expected_exit_code: int
    expected_stderr_fragment: str


@dataclass(frozen=True)
class PathDefaultCompileTestCase:
    description: str
    repo_files: dict[str, str]
    expected_exit_code: int
    expected_stderr_fragment: str


@dataclass(frozen=True)
class PythonProjectLayoutCompileTestCase:
    description: str
    repo_files: dict[str, str]
    expected_exit_code: int
    expected_stderr_fragments: tuple[str, ...]


@dataclass(frozen=True)
class CompileSelectionTestCase:
    description: str
    selection_args: tuple[str, ...]
    expected_stdout_fragments: tuple[str, ...]
    unexpected_stdout_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SelectorFileErrorTestCase:
    description: str
    command: tuple[str, ...]
    expected_json_codes: tuple[str, ...]


@dataclass(frozen=True)
class SourceSelectionRulesTestCase:
    description: str
    selection_args: tuple[str, ...]
    expected_stdout_fragment: str


@dataclass(frozen=True)
class DeepSqlAnalysisCompileTestCase:
    description: str
    function_depth: int
    expected_stdout_fragment: str


@dataclass(frozen=True)
class DenseCompileGuardTestCase:
    description: str
    model_count: int
    expected_max_wall_seconds: float
    expected_max_rss_bytes: int
    expected_fingerprint: str


@dataclass(frozen=True)
class DenseWarmEditCompileGuardTestCase:
    description: str
    model_count: int
    edited_model_index: int
    expected_warm_max_seconds: float
    expected_edit_max_seconds: float
    expected_max_rss_bytes: int
    expected_cold_fingerprint: str
    expected_edit_fingerprint: str
    expected_edit_rule_cache_misses: int


@dataclass(frozen=True)
class DenseDeltaEditLoopGuardTestCase:
    description: str
    model_count: int
    edited_model_index: int
    full_pairs: int
    expected_max_full_seconds: float
    expected_max_delta_seconds: float
    expected_max_delta_cpu_ratio: float
    expected_max_fallback_cpu_ratio: float
    expected_max_rss_bytes: int


@dataclass(frozen=True)
class RuleGatedTestDiagnosticsTestCase:
    description: str
    filler_model_count: int
    extra_args: tuple[str, ...]
    expected_diagnostics: tuple[tuple[str, str, str], ...]
    expected_text_lines: tuple[str, ...]


@dataclass(frozen=True)
class EmptyInputTestRuleCompileTestCase:
    description: str
    select: tuple[str, ...]
    expected_diagnostics: frozenset[tuple[str, str, str]]


@dataclass(frozen=True)
class EmptyInputTestRuleOptionErrorTestCase:
    description: str
    allowed_tests_toml: str
    expected_error: str


@dataclass(frozen=True)
class EmptyInputTestRuleCacheTestCase:
    description: str
    replacement_mock: str
    expected_first_codes: tuple[str, ...]
    expected_second_codes: tuple[str, ...]


@dataclass(frozen=True)
class TypeProofRuleCompileTestCase:
    description: str
    project_toml: str
    extra_args: tuple[str, ...]
    expected_returncode: int
    expected_rule_findings: int
    expected_note_count: int


@dataclass(frozen=True)
class FactoryModuleNodePlanTestCase:
    description: str
    repo_files: dict[str, str]
    expected_python_node_names: tuple[str, ...]


@dataclass(frozen=True)
class UnrelatedPythonPackageBuildTestCase:
    description: str
    unrelated_files: dict[str, str]
    repo_files: dict[str, str]
    expected_build_fragments: tuple[str, ...]


@dataclass(frozen=True)
class DiagnosticPerformanceCase:
    description: str
    depth: int = 100
    width: int = 32
    expected_max_wall_seconds: float = 20.0
    expected_timeout_seconds: float = 65.0
    diagnostic_count: int = 100


@dataclass(frozen=True)
class InspectionCommandPerformanceGuardTestCase:
    description: str
    sqb_args: tuple[str, ...]
    expected_fragments: tuple[str, ...]
    expected_max_output_lines: int
    expected_max_wall_seconds: float
    expected_max_rss_bytes: int


@dataclass(frozen=True)
class PlanComparedToCompileGuardTestCase:
    description: str
    expected_max_compile_cpu_ratio: float
    expected_max_compile_wall_ratio: float
    expected_max_plan_wall_seconds: float
    expected_max_rss_bytes: int


@dataclass(frozen=True)
class PlanScalingGuardTestCase:
    description: str
    small_model_count: int
    expected_max_linear_factor: float


@dataclass(frozen=True)
class PlanPhaseScalingGuardTestCase:
    description: str
    small_model_count: int
    runs: int
    expected_phases: tuple[str, ...]
    expected_max_linear_factor: float
    expected_max_large_phase_seconds: float


@dataclass(frozen=True)
class BuildPerformanceGuardTestCase:
    description: str
    expected_max_wall_seconds: float
    expected_max_rss_bytes: int


@dataclass(frozen=True)
class ExistingStatePlanGuardTestCase:
    description: str
    edited_models: tuple[str, ...]
    renamed_models: tuple[tuple[str, str], ...]
    expected_query_changed: tuple[str, ...]
    expected_migrations: tuple[tuple[str, str, str], ...]
    expected_max_wall_seconds: float
    expected_max_rss_bytes: int


@dataclass(frozen=True)
class CompileCacheInvalidationTestCase:
    """One authored input edit applied after a warm compile-cache run."""

    description: str
    edit: Callable[[Path], None]
    edited_env: dict[str, str] = field(default_factory=dict)
    expected_failure: bool = False
    expected_output_change: bool = True
    expected_rewarmed_analysis_cache_misses: int = 0


@dataclass(frozen=True)
class CompileCacheDisabledTestCase:
    """One supported control that disables compile-cache reads and writes."""

    description: str
    compile_args: tuple[str, ...] = ()
    env: dict[str, str] = field(default_factory=dict)
    edit: Callable[[Path], None] = lambda _root: None
    expected_analysis_cache_counts: tuple[int, int] = (0, 0)
    expected_analysis_databases: tuple[Path, ...] = ()


@dataclass(frozen=True)
class FunctionHeaderKeyCompileCase:
    description: str
    repo_files: dict[str, str]
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class FunctionNameCompileTestCase:
    description: str
    project_toml: str
    query_sql: str
    expected_exit_code: int
    expected_diagnostics: tuple[tuple[str, str, int, int], ...]


@dataclass(frozen=True)
class TruthValueTestBuildTestCase:
    description: str
    repo_files: dict[str, str]
    expected_exit_code: int
    expected_stdout_fragment: str


@dataclass(frozen=True)
class TruthValueTestCompileTestCase:
    description: str
    repo_files: dict[str, str]
    expected_exit_code: int
    expected_diagnostics: tuple[tuple[str, str, str, int, int], ...]
    expected_help: str | None = None


@dataclass(frozen=True)
class ModelHeaderKeyCompileCase:
    description: str
    repo_files: dict[str, str]
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ResourceSqlValidationCase:
    description: str
    files: tuple[tuple[str, str], ...]
    expected_diagnostics: tuple[tuple[str, str, str, int], ...]


@dataclass(frozen=True)
class ResourceSqlOptOutCase:
    description: str
    files: tuple[tuple[str, str], ...]
    flags: tuple[str, ...]
    expected_diagnostics: tuple[tuple[str, str, int], ...]
    expected_returncode: int = 0


@dataclass(frozen=True)
class ResourceSqlHelpCase:
    description: str
    files: tuple[tuple[str, str], ...]
    expected_diagnostics: tuple[tuple[str, str, str | None], ...]


@dataclass(frozen=True)
class TypeFindingLocationCase:
    description: str
    adapter: str
    query_sql: str
    expected_diagnostics: tuple[tuple[str, str, int, int], ...]


@dataclass(frozen=True)
class CachedBindingDiagnosticCase:
    description: str
    query_sql: str
    expected_location: tuple[str, int, int]


@dataclass(frozen=True)
class RequireSqlAnalysisCase:
    description: str
    repo_files: tuple[tuple[str, str], ...]
    command: tuple[str, ...]
    expected_returncode: int
    expected_diagnostics: tuple[tuple[str, str | None, int | None], ...]
    expected_text_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SetOperationModel:
    description: str
    name: str
    query_sql: str
    expected_row_count: int | None = None


@dataclass(frozen=True)
class SetOperationLifecycleTestCase:
    description: str
    models: tuple[SetOperationModel, ...]
    expected_column_count: int
    expected_diagnostics: tuple[tuple[str, str], ...]
    expected_rule_summary: str
    expected_indented_set_operation_lines: tuple[str, ...]


@dataclass(frozen=True)
class FromQuerySetOperationTestCase:
    description: str
    adapter: str
    models: tuple[SetOperationModel, ...]
    expected_column_counts: tuple[tuple[str, int], ...]
    lineage_target: str
    expected_lineage: tuple[tuple[str, str, str], ...]
    expected_rule_summary: str


@dataclass(frozen=True)
class SetOperationArityMismatchTestCase:
    description: str
    models: tuple[SetOperationModel, ...]
    expected_exit_code: int
    expected_diagnostics: tuple[tuple[str, str], ...]


@dataclass(frozen=True)
class UnionFixtureCompileScalingTestCase:
    description: str
    sql_test_count: int
    small_fixture_rows: int
    large_fixture_rows: int
    measured_runs: int
    expected_sql_tests: int
    expected_errors: int
    expected_max_scaling_ratio: float
    input_fixture_row_separator: str = " UNION ALL\n"


@dataclass(frozen=True)
class RequiredDescriptionCase:
    description: str
    missing_files: tuple[tuple[str, str], ...]
    described_files: tuple[tuple[str, str], ...]
    expected_diagnostic: tuple[str, str, int]
    expected_message: str
    expected_help_fragments: tuple[str, ...]


@dataclass(frozen=True)
class RequiredDescriptionAggregateCase:
    description: str
    files: tuple[tuple[str, str], ...]
    expected_codes: frozenset[str]
    expected_kinds: frozenset[str]


@dataclass(frozen=True)
class RequiredDescriptionExactOutputCase:
    description: str
    files: tuple[tuple[str, str], ...]
    expected_diagnostics: tuple[tuple[str, str, str, int, int], ...]
    expected_help: tuple[str, ...]


@dataclass(frozen=True)
class RequiredDescriptionPlanCase:
    description: str
    model_sql: str
    expected_returncode: int
    expected_stderr_fragment: str


@dataclass(frozen=True)
class PathDefaultDescriptionCase:
    description: str
    model_files: tuple[tuple[str, str], ...]
    expected_returncode: int
    expected_diagnostics: tuple[object, ...]
    expected_descriptions: tuple[tuple[str, str], ...]


@dataclass(frozen=True)
class FormatDescriptionResolutionE2ECase:
    description: str
    extra_files: tuple[tuple[str, str], ...]
    expected_returncode: int
    expected_faults: tuple[tuple[str, str], ...]


@dataclass(frozen=True)
class EarlyLintCompileTestCase:
    """A rules-enabled project and the diagnostics compile must report for it."""

    description: str
    order_totals_sql: str
    expected_exit_code: int
    expected_diagnostics: tuple[tuple[str, str, int, int], ...]


@dataclass(frozen=True)
class EarlyLintInterruptTestCase:
    """A rules-enabled project whose lint-thread macro expansions are slow enough to interrupt."""

    description: str
    model_count: int
    macro_seconds: float
    expected_max_stop_seconds: float


@dataclass(frozen=True)
class CompileReuseInvalidationTestCase:
    """One compile input change that must make the stored compile miss."""

    description: str
    edit: Callable[[Path], None] = lambda _project_dir: None
    edited_env: dict[str, str] = field(default_factory=dict)
    edited_args: tuple[str, ...] = ()
    expected_returncode: int = 0
    expected_rewarm_hit: bool = True


@dataclass(frozen=True)
class CompileReuseHitTestCase:
    """One change outside the compile inputs that must still reuse the stored compile."""

    description: str
    edit: Callable[[Path], None]
    edited_env: dict[str, str] = field(default_factory=dict)
    expected_reused: bool = True


@dataclass(frozen=True)
class CompileReuseBypassTestCase:
    """One control or input that must always run the full compile."""

    description: str
    edit: Callable[[Path], None] = lambda _project_dir: None
    compile_args: tuple[str, ...] = ()
    global_args: tuple[str, ...] = ()
    env: dict[str, str] = field(default_factory=dict)
    expected_stored_entries: int = 0


def keep_compiled_root(_project_dir: Path) -> None:
    """Leave the compiled folder as compile creates it."""


@dataclass(frozen=True)
class CompiledTreeStampTestCase:
    """An unmanaged file added under a compiled tree the previous compile stamped."""

    description: str
    edit: Callable[[Path], None]
    expected_removed_path: str
    prepare: Callable[[Path], None] = keep_compiled_root
    expected_stamped: bool = True


@dataclass(frozen=True)
class CompileReuseRacyRefreshTestCase:
    """A stored compile whose inputs were racy when stored, refreshed by the next hit."""

    description: str
    expected_reused: tuple[bool, bool, bool]
    expected_output_rewritten: bool


@dataclass(frozen=True)
class CompileReuseCorruptEntryTestCase:
    """One way a stored compile can be damaged on disk."""

    description: str
    corrupt: Callable[[Path], None]
    expected_fallback_reused: bool = False
    expected_restored_reused: bool = True


@dataclass(frozen=True)
class CompileReuseReplayTestCase:
    """One compile whose replay must reproduce output, notes, and exit code."""

    description: str
    compile_args: tuple[str, ...]
    expected_returncode: int
    expected_note_prefix: str = "note:"


@dataclass(frozen=True)
class CompileReuseRedirectTestCase:
    """A compile whose stdout is redirected into a file inside the project."""

    description: str
    report_name: str
    expected_hit_counts: tuple[int, int]


@dataclass(frozen=True)
class CompileReuseEventTestCase:
    """A lifecycle sink that must still receive invocation events on a reuse hit."""

    description: str
    sink_name: str
    expected_event_types: tuple[str, ...]


@dataclass(frozen=True)
class CompileReuseTimingsTestCase:
    """The compile_timings object a reuse hit reports in place of the stored timings."""

    description: str
    expected_timing_names: frozenset[str]


@dataclass(frozen=True)
class CompileReuseProviderSettingsTestCase:
    """A provider settings input change that must miss and match a full compile."""

    description: str
    edited_env: dict[str, str]
    edit: Callable[[Path], None] = lambda _project_dir: None
    expected_returncode: int = 0
    expected_rewarm_hit: bool = True


@dataclass(frozen=True)
class CompileReuseConcurrentWriterTestCase:
    """Another writer touching compiled artifacts between this compile's write and store."""

    description: str
    other_write: Callable[[Path], None]
    expected_stored_entries: int
    expected_rerun_reused: bool


@dataclass(frozen=True)
class CompileReuseLargeFileTestCase:
    """A large data file that reuse must not read unless its stat identity moves."""

    description: str
    size_bytes: int
    relative_path: str
    expected_reads: tuple[int, int, int, int, int]
    expected_reused: tuple[bool, bool, bool, bool, bool]


@dataclass(frozen=True)
class CompileReuseStoreFailureTestCase:
    """An unexpected error while storing a finished compile."""

    description: str
    error: BaseException
    expected_returncode: int = 0
    expected_stored_entries: int = 0
    expected_raised: type[BaseException] | None = None


@dataclass(frozen=True)
class IncrementalEditStep:
    """One authored edit, the command run over it before its compile, and whether it replays."""

    description: str
    edit: Callable[[Path], None]
    between: Callable[[Path], None]
    expected_replayed: bool = False


@dataclass(frozen=True)
class IncrementalEditSequenceTestCase:
    """Edits applied in order, each compiled incrementally and compared with --no-cache."""

    description: str
    steps: tuple[IncrementalEditStep, ...]
    expected_matches_uncached: bool = True


@dataclass(frozen=True)
class RandomEditChainTestCase:
    """A seeded random chain of edits over a generated project."""

    description: str
    seed: int
    model_count: int
    step_count: int
    expected_matches_uncached: bool = True


@dataclass(frozen=True)
class BrokenEditInvalidationTestCase:
    """A cache invalidation deliberately broken, and an edit whose output it must change."""

    description: str
    edit: Callable[[Path], None]
    sabotage: Callable[[pytest.MonkeyPatch], None]
    expected_matches_uncached: bool


@dataclass(frozen=True)
class StoredMacroDeltaFallbackTestCase:
    """A project edit whose delta compile falls back after compiling its round."""

    description: str
    edited_path: str
    old: str
    new: str
    expected_outcome: tuple[int, int, bool, int]


@dataclass(frozen=True)
class StoredMacroModuleEditTestCase:
    """An outside module behind a stored macro call, edited after a compile that reused the call."""

    description: str
    import_name: str
    module_files: dict[str, str]
    edited_files: dict[str, str]
    expected_compiled_value: str


@dataclass(frozen=True)
class ExternalModuleEditTestCase:
    """An outside module a macro imports while rendering, rewritten in place after a compile."""

    description: str
    initial_value: str
    edited_value: str
    expected_matches_uncached: bool
    expected_compiled_value: str


@dataclass(frozen=True)
class NativeAnalysisStoreTestCase:
    """A change between two compiles, and how many stored native model analyses it misses."""

    description: str
    change: Callable[[Path, pytest.MonkeyPatch], None]
    expected_misses: int


@dataclass(frozen=True)
class SqlTestScanStoreTestCase:
    """A change between two compiles, and how many stored SQL-test scans it must invalidate."""

    description: str
    change: Callable[[Path, pytest.MonkeyPatch], None]
    expected_rescans: int
    arrange: Callable[[pytest.MonkeyPatch], None] = lambda _monkeypatch: None
    expected_matches_uncached: bool = True


@dataclass(frozen=True)
class RetiredCompilerCacheTestCase:
    """A compile over a project holding cache files older releases wrote."""

    description: str
    compile_args: tuple[str, ...]
    expected_removed: bool


type DensePreparedCompile = PreparedCompile


@dataclass(frozen=True)
class SharedCacheKeyTestCase:
    """Expected parity when identical SQL models share native cached analysis."""

    description: str
    expected_matches_uncached: bool = True


@dataclass(frozen=True)
class NativeTypeSystemTestCase:
    """A compile of one project whose contracts compare types or fail."""

    description: str
    project_files: dict[str, str]
    expected_exit_code: int
    expected_report_text: str


@dataclass(frozen=True)
class NativeAnalysisSeamTestCase:
    """Compiling one project while the analysis stage seams are recorded."""

    description: str
    expected_stage_work: dict[str, int]


@dataclass(frozen=True)
class ReplaceableReferenceCallTestCase:
    """A project whose double-quoted reference calls must build end to end."""

    description: str
    expected_total_cents: str


@dataclass(frozen=True)
class RejectedReferenceCallTestCase:
    """A reference call form compile cannot replace and the error it must report first."""

    description: str
    staging_from: str
    mart_from: str
    expected_diagnostics: tuple[tuple[str, str, str, int, int, str], ...]


@dataclass(frozen=True)
class NativeDeclarationErrorTestCase:
    """A failing project whose declaration error is reported once."""

    description: str
    project_files: dict[str, str]
    expected_report_text: str
    expected_macro_calls: int


@dataclass(frozen=True)
class ReferenceScanErrorTestCase:
    """A project whose reference scan error must be located and typed."""

    description: str
    project_files: dict[str, str]
    expected_message: str
    expected_macro_calls: int
    expected_error_type: str


@dataclass(frozen=True)
class NativeMacroErrorTestCase:
    """A failing macro project reported once, running each call once."""

    description: str
    project_files: dict[str, str]
    expected_report_text: str
    expected_macro_runs: list[str]


@dataclass(frozen=True)
class UnkeyableMacroCallTestCase:
    """Compile arguments holding text that is not valid Unicode, which compile rejects."""

    description: str
    project_files: dict[str, str]
    compile_args: tuple[str, ...]
    expected_returncode: int
    expected_report_fragment: str


@dataclass(frozen=True)
class MacroErrorLifecycleTestCase:
    """A failing macro project and the error type its invocation lifecycle reports."""

    description: str
    project_files: dict[str, str]
    expected_error_type: str


@dataclass(frozen=True)
class NativeConfigErrorTestCase:
    """A failing project whose model config error is reported without Python fallbacks."""

    description: str
    project_files: dict[str, str]
    expected_report_text: str
    expected_macro_calls: int
    expected_python_fallbacks: list[str]


@dataclass(frozen=True)
class MacroExpandedValidatorErrorTestCase:
    """A validator error from macro-expanded SQL, reported after running macros once."""

    description: str
    project_files: dict[str, str]
    expected_report_text: str
    expected_macro_calls: int


@dataclass(frozen=True)
class DeclarationErrorLifecycleTestCase:
    """A failing project and the error type its invocation lifecycle reports."""

    description: str
    project_files: dict[str, str]
    expected_error_type: str


@dataclass(frozen=True)
class MacroReferenceCallStoreTestCase:
    """Repeated compiles of models sharing a macro that returns a rejected reference call."""

    description: str
    expected_logged_calls: tuple[int, ...]


@dataclass(frozen=True)
class MacroCallStoreEditStep:
    """One edit compiled with the macro call store, and the executions it expects."""

    description: str
    edit: Callable[[Path], object]
    expected_logged_calls: int
    args: tuple[str, ...] = ()
    env: tuple[tuple[str, str], ...] = ()


@dataclass(frozen=True)
class MacroCallStoreEditSequenceTestCase:
    """Edits applied in order, each compiled with the macro call store and with --no-cache."""

    description: str
    project_reuse: bool
    steps: tuple[MacroCallStoreEditStep, ...]
    expected_matches_uncached: bool = True


@dataclass(frozen=True)
class BrokenMacroCallStoreKeyTestCase:
    """An edit the store key no longer covers, which the --no-cache oracle must catch."""

    description: str
    edit: Callable[[Path], None]
    expected_matches_uncached: bool


@dataclass(frozen=True)
class EngineMacroCallGateTestCase:
    """Repeated full compiles and whether the bridge and store took part."""

    description: str
    expected_logged_calls: tuple[int, ...]
    expected_store_files: tuple[str, ...]


class StaleStoreArrangement(NamedTuple):
    """What an arrangement step may use to compile and edit before the fresh-process check."""

    project_dir: Path
    extlib: Path
    monkeypatch: pytest.MonkeyPatch
    capsys: pytest.CaptureFixture[str]


@dataclass(frozen=True)
class StaleMacroModuleStoreTestCase:
    """An outside module a macro imported, edited after this process imported it."""

    description: str
    arrange: Callable[[StaleStoreArrangement], None]
    compile_afresh: Callable[[StaleStoreArrangement], CompileReuseRun]
    expected_flavor: str


@dataclass(frozen=True)
class SecondCompileStoreTestCase:
    """Two compiles in one process; the second must run its macros without the store."""

    description: str
    expected_second_matches_first: bool


@dataclass(frozen=True)
class DeepHeaderNestingCompileCase:
    description: str
    path: str
    prefix: str
    suffix: str
    depth: int
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class DeepCallNestingCompileCase:
    description: str
    depth: int
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class DollarQuotedProjectVarCompileCase:
    description: str
    model_sql: str
    expected_fragment: str


@dataclass(frozen=True)
class EarlyBuiltinRulesTestCase:
    """A rules-enabled project, the compile arguments and the early rules outcome it must report."""

    description: str
    repo_files: dict[str, str]
    compile_arguments: tuple[str, ...]
    expected_exit_code: int
    expected_diagnostics: tuple[tuple[str, str, int, int], ...]
    expected_early_hits: int
    expected_early_misses: int


@dataclass(frozen=True)
class EarlyBuiltinRulesEditTestCase:
    """A model edit between compiles and the early rules and rule cache outcomes of each compile."""

    description: str
    edited_path: str
    edited_sql: str
    expected_diagnostics: tuple[tuple[str, str, int, int], ...]
    expected_early_outcomes: tuple[tuple[int, int], ...]
    expected_rule_cache_outcomes: tuple[tuple[int, int], ...]


@dataclass(frozen=True)
class DeltaCompileSeriesTestCase:
    description: str
    seed: int
    json_output: bool
    steps: int
    adapter: str = "duckdb"
    expected_unexpected_outcomes: tuple[str, ...] = ()
    expected_minimum_deltas: int = 3


@dataclass(frozen=True)
class DeltaCompileFullCopyTestCase:
    description: str
    edits: tuple[tuple[str, str, str], ...]
    expected_delta_line: bool
    stored_edits: tuple[tuple[str, str, str], ...] = ()
    json_output: bool = False
    adapter: str = "duckdb"
    expected_difference: None = None


@dataclass(frozen=True)
class DeltaCompileProjectShapeTestCase:
    description: str
    files: tuple[tuple[str, str], ...]
    edits: tuple[tuple[str, str, str], ...]
    expected_delta_line: bool
    stored_edits: tuple[tuple[str, str, str], ...] = ()
    expected_difference: None = None


@dataclass(frozen=True)
class DeltaCompileVerifyTestCase:
    description: str
    files: tuple[tuple[str, str], ...]
    edits: tuple[tuple[str, str, str], ...]
    env: dict[str, str]
    expected_stderr_line: str
    stored_edits: tuple[tuple[str, str, str], ...] = ()


@dataclass(frozen=True)
class DeltaCompileSequenceTestCase:
    description: str
    edits: tuple[SequenceEdit, ...]
    expected_delta_lines: tuple[bool, ...]
    expected_replayed_last: bool
    expected_same_as_full: bool


@dataclass(frozen=True)
class DeltaCompileOptOutTestCase:
    description: str
    settings: str
    env: dict[str, str]
    expected_delta_line: bool = False
    expected_images: tuple[str, ...] = ()


@dataclass(frozen=True)
class StarBesideCastConstantTestCase:
    """A model selecting a star beside a cast constant and the columns it must compile and build."""

    description: str
    query_sql: str
    expected_columns: tuple[str, ...]


@dataclass(frozen=True)
class DialectLineCommentTestCase:
    """A dialect's own line comment holding quotes and reference text, and the columns it keeps."""

    description: str
    project_toml: str
    query_sql: str
    expected_column_count: int


@dataclass(frozen=True)
class MissingTargetDatabaseTestCase:
    """A Snowflake project without a target database and the S101 error it must print."""

    description: str
    project_toml: str
    expected_error: str
    expected_example_count: int


@dataclass(frozen=True)
class UndecodableSelectorTestCase:
    """An output mode for a selector holding undecodable bytes and the error it must print."""

    description: str
    output_flags: tuple[str, ...]
    expected_code: str
    expected_escaped_selector: str


@dataclass(frozen=True)
class BenchmarkProjectCompileTestCase:
    """A benchmark project in the repository and the diagnostics its compile may report."""

    description: str
    project_dir: Path
    expected_errors: int
    expected_warnings: int


@dataclass(frozen=True)
class BenchmarkCoreCopyCompileTestCase:
    """A script that copies a benchmark project without an area, and the copy's compiled output.

    `expected_compiled_checksum` is the script's `--checksum` of the copy's compiled files; a core
    model change updates it deliberately.
    """

    description: str
    copy_script: Path
    expected_absent_folder: str
    expected_errors: int
    expected_warnings: int
    expected_compiled_checksum: str


@dataclass(frozen=True)
class GeneratedBenchmarkCompileTestCase:
    """A generated benchmark project, its recorded checksum and its cold compile budget."""

    description: str
    generator: Path
    expected_min_models: int
    expected_max_wall_seconds: float
    expected_max_rss_bytes: int


@dataclass(frozen=True)
class RelationshipsTargetCompileTestCase:
    """A relationships audit target and how compiling it must end."""

    description: str
    target: str
    expected_exit_code: int
    expected_output_fragment: str


@dataclass(frozen=True)
class ContractCastTypeTestCase:
    """An outer cast on a contracted Snowflake output, compile's exit code and report fragments."""

    description: str
    projection: str
    expected_exit_code: int
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ParameterisedTestLintTestCase:
    """A parameterised SQL test linted by SQL rules, and the commands that must succeed on it."""

    description: str
    selected_rules: str
    command: tuple[str, ...]
    expected_fragment: str


@dataclass(frozen=True)
class BigQueryStringContractTestCase:
    """A BigQuery contract column of a STRING type and the cast that fills it."""

    description: str
    declared_type: str
    cast_type: str
    expected_exit_code: int


@dataclass(frozen=True)
class TypedCallContractTestCase:
    """A typed SQL macro filling a Snowflake contract column, and how SQBRCONTRACT105 judges it."""

    description: str
    returns: str
    body: str
    projection: str
    expected_exit_code: int
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class MacroDictKeyCompileE2ETestCase:
    """A macro call whose dict argument has a key, and the compile outcome it must give."""

    description: str
    argument: str
    expected_exit_code: int
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SnowflakeStarContractE2ETestCase:
    """A Snowflake model reading a star under a lowercase contract, and the K005 it reports."""

    description: str
    upstream_columns: str
    expected_exit_code: int
    expected_k005_columns: tuple[str, ...]


@dataclass(frozen=True)
class DialectSyntaxCompileE2ETestCase:
    """A project using dialect syntax the SQL parser must analyse, and the compile outcome."""

    description: str
    adapter: str
    models: dict[str, str]
    expected_exit_code: int
    expected_codes: tuple[str, ...]
    expected_column_counts: dict[str, int]
    ignored_rules: tuple[str, ...] = ()


@dataclass(frozen=True)
class SqlTestCacheMissE2ETestCase:
    """A change to compiled SQL test artifacts and the miss reasons a warm compile logs."""

    description: str
    change: Callable[[Path], None]
    expected_debug_line: str


@dataclass(frozen=True)
class SqlMacroStringEscapeTestCase:
    """A model whose SQL macro call follows a string the adapter ends with its own rules."""

    description: str
    adapter_config: str
    string_sql: str
    expected_compiled_fragment: str


@dataclass(frozen=True)
class PythonMacroArgumentErrorTestCase:
    """An invalid `@py` argument span on Snowflake and the error compile reports for it."""

    description: str
    call_sql: str
    expected_message: str
