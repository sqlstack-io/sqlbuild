"""Test case types for Rules performance guards."""

from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path


@dataclass(frozen=True)
class EvaluatedPredicateCase:
    description: str
    sql: str
    expected_evaluated_models: int = 1


@dataclass(frozen=True)
class EvaluatedModelRulesCase:
    description: str
    sql: str
    rules: tuple[str, ...]
    selector: str
    expected_rule_findings: tuple[tuple[str, int, int], ...]
    expected_compile_findings: tuple[tuple[str, int, int], ...]


@dataclass(frozen=True)
class GroupedRulesCase:
    description: str
    arguments: tuple[str, ...]
    result_key: str
    expected_rules: tuple[str, ...]
    configuration: str = ""


@dataclass(frozen=True)
class UnevaluatedRuleCase:
    description: str
    model_options: str = ""
    configuration: str = ""
    expected_exit: int = 1


@dataclass(frozen=True)
class UnevaluatedResourceCase:
    description: str
    path: str
    template: str
    expected_evaluated_models: int = 1
    expected_reason: str = "E_GUARD_FUNCTION_NESTING_DEPTH_EXCEEDED"
    adapter: str = "duckdb"
    extra_files: tuple[tuple[str, str], ...] = ()
    rule: str = "SQLCLEAN035"


@dataclass(frozen=True)
class RulesPerformanceGuardTestCase:
    description: str
    model_count: int
    hard_ceiling_seconds: int
    expected_max_elapsed_seconds: float
    expected_returncode: int


@dataclass(frozen=True)
class UnusedCteContextTestCase:
    """Project whose framework context keeps a CTE reachable."""

    description: str
    files: dict[str, str]
    expected_returncode: int
    expected_finding_count: int


@dataclass(frozen=True)
class HermeticRuleCase:
    description: str
    code: str
    files: tuple[tuple[str, str], ...]
    expected_rule_codes: tuple[str, ...]


@dataclass(frozen=True)
class NonHermeticRuleCase:
    description: str
    code: str
    source: str
    expected_line: int
    expected_action: str


@dataclass(frozen=True)
class ScrubbedEnvironmentCase:
    description: str
    code: str
    source: str
    parent_environment: tuple[tuple[str, str], ...]
    expected_returncode: int
    expected_rule_codes: tuple[str, ...]


@dataclass(frozen=True)
class HashSeedCase:
    description: str
    code: str
    names: tuple[str, ...]
    expected_hash_seed: str


@dataclass(frozen=True)
class ProjectTreeCacheCase:
    description: str
    code: str
    source: str
    policy_path: str
    added_model_path: str
    expected_paths_after_glob_change: tuple[str, ...]
    expected_minimum_cache_hits: int


@dataclass(frozen=True)
class ModuleStatementEditCase:
    description: str
    code: str
    source: str
    appended_statement: str
    expected_rule_codes_before_edit: tuple[str, ...]
    expected_rule_codes_after_edit: tuple[str, ...]


@dataclass(frozen=True)
class WorkingDirectoryCase:
    description: str
    code: str
    source: str
    invocation_directories: tuple[str, ...]
    expected_working_directory: str


@dataclass(frozen=True)
class HelperImportCase:
    description: str
    code: str
    rule_source: str
    helper_path: str
    helper_source: str
    expected_error: str


@dataclass(frozen=True)
class SqlRulePathParityCase:
    description: str
    files: dict[str, str]
    expected_findings: tuple[tuple[str, int, int, str], ...]
    expected_exit: int = 1


@dataclass(frozen=True)
class SqlQualityRuleCase:
    description: str
    upstream_header: str
    query_sql: str
    expected_findings: tuple[tuple[str, int], ...]
    expected_after_fix: tuple[tuple[str, int], ...]


@dataclass(frozen=True)
class RankingKeyProofCase:
    description: str
    upstream_header: str
    upstream_sql: str
    query_sql: str
    expected_codes: tuple[str, ...]
    extra_files: tuple[tuple[str, str], ...] = ()


@dataclass(frozen=True)
class RankingSortDefaultCase:
    description: str
    order_by: str
    expected_codes: tuple[str, ...]


@dataclass(frozen=True)
class ReplayedFindingExitCase:
    description: str
    code: str
    expected_exit: int


@dataclass(frozen=True)
class FixableReportingCase:
    description: str
    summary_sql: str
    expected_fixability: tuple[tuple[str, bool], ...]
    expected_notes: tuple[tuple[str, tuple[str, ...]], ...]
    expected_help_fragments: tuple[tuple[str, str], ...]


@dataclass(frozen=True)
class ConstantLiteralCase:
    description: str
    summary_sql: str
    expected_exit: int
    expected_codes: tuple[str, ...]


@dataclass(frozen=True)
class DuplicateLiteralCase:
    description: str
    literals: tuple[tuple[str, str], ...]
    expected_hints: tuple[tuple[str, str | None], ...]


@dataclass(frozen=True)
class RulesCacheEditCase:
    """One authored input edit applied after a warm compile with every rule selected."""

    description: str
    edit: Callable[[Path], None]
    expected_exit_code: int = 1
    expected_diagnostics_fragment: str = ""


@dataclass(frozen=True)
class SplitHostStateCase:
    """Custom rules sharing helper tables, one with cross-model state, across host processes."""

    description: str
    model_count: int
    selected_rules: tuple[str, ...]
    files: tuple[tuple[str, str], ...]
    expected_findings: tuple[tuple[str, str], ...]


@dataclass(frozen=True)
class RulesEditChainCase:
    """A chain of authored edits, each compiled warm and compared with a cache-free compile."""

    description: str
    edits: tuple[RulesCacheEditCase, ...]
    expected_cold_codes: tuple[str, ...]
    expected_exit_code: int = 1


@dataclass(frozen=True)
class OneModelCustomRuleEditCase:
    """One model edited after a warm compile of a model-subject custom rule."""

    description: str
    edited_model: str
    before: str
    after: str
    expected_warm_counts: tuple[int, int]
    expected_edit_counts: tuple[int, int]
    expected_edit_exit_code: int


@dataclass(frozen=True)
class OneRowCrossJoinCase:
    """One model whose CROSS JOIN reads a relation that may hold at most one row."""

    description: str
    seed_rows: tuple[str, ...]
    model_sql: str
    expected_messages: tuple[str, ...]
    expected_exit: int


@dataclass(frozen=True)
class RuleExceptionConfigCase:
    """One `sqlbuild_project.toml` exception setup compiled against two cross joins."""

    description: str
    rules_toml: str
    model_sql: str
    expected_exit: int
    expected_messages: tuple[str, ...] = ()
    expected_fragments: tuple[str, ...] = ()


@dataclass(frozen=True)
class PublishedLayerCase:
    """One set of published models checked by the published-layer Rules."""

    description: str
    files: tuple[tuple[str, str], ...]
    rules_toml: str
    expected_findings: tuple[tuple[str, str], ...]
    expected_exit: int


@dataclass(frozen=True)
class SelectedProjectWideRuleCase:
    """One selected plan whose project-wide Rules must agree with a whole-project compile."""

    description: str
    rules_toml: str
    selection: tuple[str, ...]
    expected_compile_findings: tuple[tuple[str, str], ...]
    expected_plan_findings: tuple[tuple[str, str], ...]
    expected_plan_exit: int


@dataclass(frozen=True)
class SelectedRuleCacheCase:
    """One compile, selected plan and edited recompile that must share the built-in Rules cache."""

    description: str
    rules_toml: str
    selection: tuple[str, ...]
    edited_path: str
    expected_entries: tuple[str, ...]
    expected_recompile_cache_hits: int


@dataclass(frozen=True)
class SelectedGraphRuleCase:
    """One selected plan whose graph-reading model Rule must agree with a whole-project compile."""

    description: str
    view_sql: str
    selection: tuple[str, ...]
    expected_compile_findings: tuple[tuple[str, str], ...]
    expected_plan_findings: tuple[tuple[str, str], ...]
    expected_plan_exit: int


@dataclass(frozen=True)
class CustomRuleTimingCase:
    """Custom Rules compiled cold, warm, with `--no-cache`, and warm again."""

    description: str
    selected_rules: tuple[str, ...]
    files: tuple[tuple[str, str], ...]
    expected_subjects: int


@dataclass(frozen=True)
class ModuleStateAttributionCase:
    """Rules whose module state changes persist or are undone within one model."""

    description: str
    selected_rules: tuple[str, ...]
    files: tuple[tuple[str, str], ...]
    edited_model: str
    expected_misses_after_edit: dict[str, int]


@dataclass(frozen=True)
class RulePositionE2ETestCase:
    """A model file whose query starts somewhere or expands macros, and where a rule must point."""

    description: str
    model_sql: str
    expected_location: str
    rule: str = "SQBRMODEL102"
    macro_files: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class ClickHouseCommentE2ETestCase:
    """A ClickHouse model with a line comment holding a quote, and the finding rules report."""

    description: str
    comment: str
    expected_location: str


@dataclass(frozen=True)
class DefaultRuleSelectionCase:
    description: str
    rules_section: str
    sql: str
    command: tuple[str, ...]
    expected_exit: int
    expected_finding: bool
    expected_notices: int


@dataclass(frozen=True)
class SqlRuleTargetVarsCase:
    description: str
    commands: tuple[tuple[str, ...], ...]
    expected_outcomes: tuple[tuple[int, bool, bool], ...]
    extra_files: dict[str, str] = field(default_factory=dict)
