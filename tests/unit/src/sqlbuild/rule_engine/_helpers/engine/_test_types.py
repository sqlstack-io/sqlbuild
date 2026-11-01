"""Test case declarations for rules engine helpers."""

from dataclasses import dataclass, field

from sqlbuild._native import RuleExemption, RuleIgnore


@dataclass(frozen=True)
class RulesConfigErrorTestCase:
    description: str
    source: str
    expected_error_pattern: str


@dataclass(frozen=True)
class RelationExceptionConfigTestCase:
    """One `[[rules.rule_exceptions]]` configuration that loads, with its relation scope."""

    description: str
    source: str
    expected_relation: str | None
    expected_cte: str | None


@dataclass(frozen=True)
class RuleIgnoreConfigTestCase:
    """One scoped-ignore configuration expectation."""

    description: str
    source: str
    expected_paths: tuple[str, ...]
    expected_selectors: tuple[str, ...]


@dataclass(frozen=True)
class RuleCodeTestCase:
    """One custom-rule code grammar expectation."""

    description: str
    code: str
    expected_family: str | None = None
    expected_error_pattern: str | None = None


@dataclass(frozen=True)
class RuleSignatureTestCase:
    """One annotation-derived subject expectation."""

    description: str
    expected_subject_parameter: str | None = None
    expected_error_pattern: str | None = None


@dataclass(frozen=True)
class PolicySelectionTestCase:
    description: str
    select: tuple[str, ...] | None
    ignore: tuple[str, ...]
    expected_codes: tuple[str, ...]


@dataclass(frozen=True)
class PolicyGuidanceTestCase:
    description: str
    expected_snippets: tuple[str, ...]


@dataclass(frozen=True)
class CustomRuleTestCase:
    description: str
    body: str
    expected_fault_codes: tuple[str, ...] = ()
    expected_fault_lines: tuple[int, ...] = ()
    expected_error_pattern: str | None = None
    expected_cache_hits: int = 0
    minimum_custom_rule_cases: int = 0
    select: tuple[str, ...] = ("XSQBRT101",)
    enabled_by_default: bool = False
    project_wide: bool = False


@dataclass(frozen=True)
class CustomRuleCacheDependencyTestCase:
    """One project-file change and expected custom-rule cache outcome."""

    description: str
    body: str
    changed_path: str
    expected_cache_hits: int


@dataclass(frozen=True)
class CustomRuleSuppressionTestCase:
    description: str
    rule_exceptions: tuple[RuleExemption, ...]
    rule_ignores: tuple[RuleIgnore, ...]
    expected_fault_codes: tuple[str, ...]


@dataclass(frozen=True)
class CustomRuleEvidenceTestCase:
    description: str
    test_source: str
    expected_count: int
    test_path: str = "tests/test_custom.py"
    expected_fault_codes_before: tuple[str, ...] = ()
    expected_fault_codes_after: tuple[str, ...] = ()


@dataclass(frozen=True)
class TypedConstantPayloadTestCase:
    description: str
    raw_value: object
    expected_value: object
    expected_type: str


@dataclass(frozen=True)
class ScopePayloadTestCase:
    description: str
    expected_result: bool


@dataclass(frozen=True)
class NativeFactPayloadTestCase:
    description: str
    expected_test_count: int
    expected_scenario_count: int


@dataclass(frozen=True)
class SqlTestRulesConfigTestCase:
    description: str
    source: str
    expected_pipeline_directory: str


@dataclass(frozen=True)
class SqlTestPolicyGuidanceTestCase:
    description: str
    pipeline_directory: str
    expected_path: str


@dataclass(frozen=True)
class PolicyLayoutConfigTestCase:
    description: str
    source: str
    expected_levels: tuple[str, ...]
    expected_thresholds: dict[str, int]


@dataclass(frozen=True)
class CustomRuleImportTestCase:
    """One static import-allowlist expectation over the fingerprinted rule closure."""

    description: str
    module_import: str
    extra_files: tuple[tuple[str, str], ...] = ()
    expected_rule_codes: tuple[str, ...] = ("XSQBRT101",)
    expected_error_pattern: str = ""


@dataclass(frozen=True)
class CustomRuleReadTrackingTestCase:
    """One model-subject rule, an edit to the second model, and the expected reuse."""

    description: str
    body: str
    expected_hits_after_edit: int
    module_prelude: str = ""
    edited_customers_sql: str = "SELECT 2 AS customer_id"
    edited_customers_config: dict[str, object] = field(default_factory=dict)


@dataclass(frozen=True)
class CustomRuleUncacheableTestCase:
    """One model-subject rule whose fact access cannot be attributed to recorded reads."""

    description: str
    body: str
    expected_warm_hits: int = 0


@dataclass(frozen=True)
class CustomRuleProjectFileTestCase:
    """One rule reading a project file whose contents change from customers to orders."""

    description: str
    file_path: str
    body: str
    expected_rerun_hits: int
    expected_finding_paths: tuple[str, ...]
    module_prelude: str = ""


@dataclass(frozen=True)
class NativeMemoTestCase:
    """One warm built-in evaluation followed by an edit that must invalidate the memo."""

    description: str
    original_config: dict[str, object]
    edited_config: dict[str, object]
    expected_original_codes: tuple[str, ...]
    expected_edited_codes: tuple[str, ...]


@dataclass(frozen=True)
class NativeBuildIdentityTestCase:
    """One cached evaluation repeated after the native extension reports another build."""

    description: str
    expected_rebuilt_evaluations: int
    expected_warm_evaluations: int


@dataclass(frozen=True)
class IncrementalRulesModeTestCase:
    """Evaluation options under which incremental results must equal full evaluation."""

    description: str
    defer_suppressions: bool
    expected_observed_codes: tuple[str, ...]
    initial_codes: tuple[str, ...] = ()


@dataclass(frozen=True)
class IncrementalCacheCountsTestCase:
    """One warm evaluation after editing a single model."""

    description: str
    expected_hits: int
    expected_misses: int
