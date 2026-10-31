"""Test types for test command e2e tests."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass, field
from pathlib import Path


@dataclass(frozen=True)
class SqlTestE2ETestCase:
    """Test case for sqb test e2e verification."""

    description: str
    expected_exit_code: int
    expected_stdout_fragment: str
    expected_stdout_fragments: tuple[str, ...] = field(default_factory=tuple)
    expected_ordered_stdout_fragments: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class SqlAnalysisChainSqlTestE2ETestCase:
    """Test case for SQL analysis SQL unit-test chain execution and artifacts."""

    description: str
    sql_analysis_enabled: bool
    expected_artifact_fragments: tuple[str, ...]
    unexpected_artifact_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ParameterCaseSelectionE2ETestCase:
    """Test case for one parameterized SQL-test case selection."""

    description: str
    case_name: str
    expected_exit_code: int
    expected_stdout_fragments: tuple[str, ...] = field(default_factory=tuple)
    expected_stderr_fragments: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class SqlTestPlanInspectionE2ETestCase:
    """Test case for offline SQL-test plan inspection."""

    description: str
    repo_files: dict[str, str]
    expected_stdout_fragments: tuple[str, ...]
    expected_exit_code: int = 0
    unexpected_stdout_fragments: tuple[str, ...] = ()


@dataclass(frozen=True)
class SqlTestFixtureValidationE2ETestCase:
    """Test case for static SQL-test fixture validation."""

    description: str
    repo_files: dict[str, str]
    expected_stderr_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ComplexValuesFixtureE2ETestCase:
    """Test case for complex string literals inside VALUES fixtures."""

    description: str
    adapter_name: str
    command: tuple[str, ...]
    expected_stdout_fragment: str


@dataclass(frozen=True)
class FixtureCompatibilityE2ETestCase:
    """Test case for valid fixture shapes that require conservative analysis."""

    description: str
    repo_files: dict[str, str]
    expected_stdout_fragment: str


@dataclass(frozen=True)
class PartialFixtureE2ETestCase:
    """Test case for implicit schema-aware partial relation fixtures."""

    description: str
    repo_files: dict[str, str]
    expected_stdout_fragment: str
    expected_artifact_fragments: tuple[str, ...]
    unexpected_artifact_fragments: tuple[str, ...] = field(default_factory=tuple)
    artifact_filename: str = "test_orders.sql"
    single_artifact_fragments: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class SqlTestInspectConflictE2ETestCase:
    """Test case for mutually exclusive inspection output modes."""

    description: str
    expected_exit_code: int
    expected_stderr_fragment: str


@dataclass(frozen=True)
class TableFunctionFixtureE2ETestCase:
    """Test case for model-mode table-function fixture execution."""

    description: str
    expected_stdout_fragment: str
    expected_artifact_fragments: tuple[str, ...]
    unexpected_artifact_fragments: tuple[str, ...]


@dataclass(frozen=True)
class UnknownTableFunctionFixtureE2ETestCase:
    """Test case for rejecting an unknown model-mode table-function fixture."""

    description: str
    expected_stderr_fragment: str


@dataclass(frozen=True)
class SharedGraphChainE2ETestCase:
    """Test case for chained SQL tests over a shared (diamond) upstream graph."""

    description: str
    sql_analysis_enabled: bool
    expected_stdout_fragments: tuple[str, ...]
    once_rendered_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SharedGraphMissingMockE2ETestCase:
    """Test case for one missing mock reached on every path of a deep shared graph."""

    description: str
    layers: int
    expected_error: str
    expected_stdout_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ExpectedColumnSubsetE2ETestCase:
    """Test case for SQL tests that compare only the expected CTE's listed columns."""

    description: str
    sql_analysis_enabled: bool
    command: tuple[str, ...]
    expected_tests: dict[str, str]
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]
    unexpected_output_fragments: tuple[str, ...] = ()


@dataclass(frozen=True)
class MissingMockCliE2ETestCase:
    """Test case for planning errors reported by plain test and compile commands."""

    description: str
    expected_error: str
    compile_error_line: str
    compile_summary_fragment: str


@dataclass(frozen=True)
class HelperScopeE2ETestCase:
    """Test case for helper CTEs referenced by expected and assertion SQL."""

    description: str
    command: tuple[str, ...]
    expected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class CursorWindowE2ETestCase:
    """Test case for SQL tests of models that use cursor intrinsics."""

    description: str
    tests: dict[str, str]
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]
    expected_compiled_fragments: tuple[str, ...] = ()


@dataclass(frozen=True)
class SetOperationExpectedE2ETestCase:
    """SQL tests whose expected CTEs combine branches with set operations."""

    description: str
    test_files: dict[str, str]
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class TerminalTestProgressE2ETestCase:
    description: str
    columns: int
    concurrency_levels: tuple[int, ...]
    expected_group_headers: tuple[str, ...]
    expected_test_row_count: int
    expected_screen_fragments: tuple[str, ...]
    expected_summary: str


@dataclass(frozen=True)
class TerminalStatementRowsE2ETestCase:
    description: str
    columns: int
    command: tuple[str, ...]
    expected_statement_rows_visible: bool


@dataclass(frozen=True)
class ChainCteScopeTestCase:
    description: str
    queries: tuple[str, ...]
    expected_sql: str
    expected_exit_code: int = 0


@dataclass(frozen=True)
class AuthoredSqlParityTestCase:
    description: str
    compiled_test_name: str
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class OptionalTestCeremonyE2ETestCase:
    """SQL test files written with and without their optional mode and SELECT 1."""

    description: str
    test_files: dict[str, str]
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class CommentedFixtureE2ETestCase:
    """SQL tests whose fixture CTEs carry comments around set-operation branches."""

    description: str
    test_files: dict[str, str]
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class TerminalStepRowsE2ETestCase:
    description: str
    columns: int
    command: tuple[str, ...]
    expected_absent_fragments: tuple[str, ...]
    expected_absent_line_prefixes: tuple[str, ...]
    expected_screen_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ModelInliningE2ETestCase:
    """A two-model chain whose models share CTE names, tested with and without SQL analysis."""

    description: str
    sql_analysis_enabled: bool
    expected_order_lines_sql: str
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class JsonDurationE2ETestCase:
    """A SQL test whose structured JSON output must report its duration."""

    description: str
    expected_order_lines_sql: str
    expected_exit_code: int
    expected_status: str


@dataclass(frozen=True)
class ParameterizedJsonE2ETestCase:
    """Parameterized SQL tests whose cases must each appear in structured JSON output."""

    description: str
    expected_checks: tuple[tuple[str, str], ...]


@dataclass(frozen=True)
class HelperIsolationE2ETestCase:
    """A helper named like a physical table or a model CTE, read only by a fixture."""

    description: str
    sql_analysis_enabled: bool
    expected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class IsolatedCompileE2ETestCase:
    """A project whose compiled SQL tests must keep helper and model CTE names apart."""

    description: str
    project_files: Mapping[str, str]
    compile_arguments: tuple[str, ...]
    helper_names: tuple[str, ...]
    expected_minimum_tests: int


@dataclass(frozen=True)
class FixtureProjectIsolationE2ETestCase:
    """A fixture project whose compiled SQL tests must keep CTE names apart."""

    description: str
    project_dir: Path
    expected_minimum_tests: int


@dataclass(frozen=True)
class SelectTestByNameE2ETestCase:
    """One command selecting SQL unit tests by name through the real CLI."""

    description: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]
    unexpected_output_fragments: tuple[str, ...] = ()


@dataclass(frozen=True)
class HelperReferenceE2ETestCase:
    """Test case for relation references inside SQL-test helper CTEs."""

    description: str
    tests: tuple[str, ...]
    command: tuple[str, ...]
    sql_analysis: bool
    expected_output_fragments: tuple[str, ...]
    expected_compiled_fragments: tuple[str, ...]
    expected_absent_compiled_fragments: tuple[str, ...]


@dataclass(frozen=True)
class HelperReferenceErrorE2ETestCase:
    """Test case for helper CTE references compile cannot resolve."""

    description: str
    test_name: str
    expected_message: str
    expected_line: int
    expected_code: str
    expected_column: int
    expected_help_fragment: str


@dataclass(frozen=True)
class HelperRedefinitionE2ETestCase:
    """Test case for a nested CTE that redefines a helper CTE's name."""

    description: str
    expected_output_fragments: tuple[str, ...]
    expected_absent_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class DirectLogicExtractionE2ETestCase:
    """One direct-logic test project variant run through `sqb test`."""

    description: str
    overrides: dict[str, str]
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class RealModelLimitE2ETestCase:
    """One chain length and limit setting run through `sqb test`."""

    description: str
    local_files: dict[str, str]
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SingleReadComparisonE2ETestCase:
    """One unit test whose comparison reads its stored actual output."""

    description: str
    test_sql: str
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class UnitTestTimeoutE2ETestCase:
    """One slow unit test run under a time limit."""

    description: str
    test_sql: str
    command: tuple[str, ...]
    expected_output_fragments: tuple[str, ...]
    expected_absent_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ModelTimeoutE2ETestCase:
    """One slow model with a max_duration built next to an independent model."""

    description: str
    max_duration: str
    expected_output_fragments: tuple[str, ...]
    expected_tables: tuple[tuple[str, bool], ...]


@dataclass(frozen=True)
class ScenarioTimeoutE2ETestCase:
    """One slow scenario run under a time limit."""

    description: str
    header_timeout: str
    command: tuple[str, ...]
    expected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ModelTimeoutJsonE2ETestCase:
    """One slow model's build reported as JSON."""

    description: str
    expected_status: str
    expected_summary_counts: dict[str, int]


@dataclass(frozen=True)
class TransactionTimeoutE2ETestCase:
    """A model whose limit expires inside its promotion transaction."""

    description: str
    max_duration: str
    expected_output_fragments: tuple[str, ...]
    expected_published_rows: tuple[tuple[int, int], ...]
    expected_later_model_rows: tuple[tuple[int], ...]


@dataclass(frozen=True)
class TemporaryFunctionE2ETestCase:
    """Test case for unit tests that call an edited project function."""

    description: str
    built_body: str
    edited_body: str
    expected_exit_code: int
    expected_stdout_fragment: str
    expected_built_result: bool


@dataclass(frozen=True)
class TemporaryCalleeE2ETestCase:
    """Test case for unit tests whose function calls another project function."""

    description: str
    build_first: bool
    callee_body: str
    expected_exit_code: int
    expected_stdout_fragment: str


@dataclass(frozen=True)
class TemporaryFunctionConnectionE2ETestCase:
    """Test case for a test connection that cannot create the temporary copies."""

    description: str
    expected_exit_code: int
    expected_stderr_fragment: str


@dataclass(frozen=True)
class OutputEvaluationErrorE2ETestCase:
    """How a unit test ends when storing its tested model's output fails for a given reason."""

    description: str
    adapter_source: str
    extra_files: dict[str, str]
    expected_exit_code: int
    expected_fragments: tuple[str, ...]
    expected_json_fragments: tuple[str, ...]
    unexpected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SharedPartialFixtureE2ETestCase:
    """A partial mock read by two real models, with each reader's column spelling."""

    description: str
    enriched_column: str
    report_column: str
    expected_padding_count: int


@dataclass(frozen=True)
class SinceSelectionE2ETestCase:
    """Edits made after the base commit and what `--since` must select and say."""

    description: str
    edits: Mapping[str, str]
    command: tuple[str, ...]
    expected_exit_code: int
    expected_fragments: tuple[str, ...]
    unexpected_fragments: tuple[str, ...] = field(default_factory=tuple)
    deletions: tuple[str, ...] = field(default_factory=tuple)
    # The git command run after the edits; `commit --all` makes them part of HEAD.
    git_after_edits: tuple[str, ...] = ("status", "--short")


@dataclass(frozen=True)
class SinceSharedBaseE2ETestCase:
    """A test run, then a scenario run, against the same base ref."""

    description: str
    edits: Mapping[str, str]
    test_command: tuple[str, ...]
    scenario_command: tuple[str, ...]
    expected_test_fragments: tuple[str, ...]
    expected_scenario_fragments: tuple[str, ...]
    unexpected_scenario_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SinceJsonOutputE2ETestCase:
    """A machine-output `--since` run and where its parts must go."""

    description: str
    edits: Mapping[str, str]
    command: tuple[str, ...]
    expected_stderr_fragments: tuple[str, ...]
    expected_test_names: tuple[str, ...]
    unexpected_stdout_fragments: tuple[str, ...]
    # The `since` section's selected items and their reasons, by name.
    expected_selected: Mapping[str, tuple[str, ...]]


@dataclass(frozen=True)
class SinceShallowCloneE2ETestCase:
    """A shallow clone that lacks the merge base of the compared ref."""

    description: str
    clone_arguments: tuple[str, ...]
    fetch_arguments: tuple[str, ...]
    command: tuple[str, ...]
    expected_exit_code: int
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class SinceBrokenBaseE2ETestCase:
    """A base revision that does not compile, and the error `--since` must report."""

    description: str
    base_files: Mapping[str, str]
    deletions: tuple[str, ...]
    command: tuple[str, ...]
    expected_exit_code: int
    expected_fragments: tuple[str, ...]
    unexpected_fragments: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class SinceUnwritableCacheE2ETestCase:
    """A `--since` run whose base snapshot cache cannot be written."""

    description: str
    blocking_file: str
    command: tuple[str, ...]
    expected_stderr_fragments: tuple[str, ...]
    expected_test_names: tuple[str, ...]


@dataclass(frozen=True)
class FixtureDeclaredTypeE2ETestCase:
    """A source fixture whose bare NULL column must take the source's declared type."""

    description: str
    fixture_sql: str
    expected_sql: str
    expected_artifact_fragment: str
    expected_stdout_fragment: str = "PASS=1  FAIL=0  TOTAL=1"


@dataclass(frozen=True)
class MacroMockAliasTestCase:
    """A macro mock's select item, with SQL rules selected, and what one command makes of it."""

    description: str
    mock_select: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_fragment: str


@dataclass(frozen=True)
class ParenthesisedJoinMockTestCase:
    """A model reading mocked references through a parenthesised join, and the test outcome."""

    description: str
    model_from: str
    expected_exit_code: int
    expected_fragment: str


@dataclass(frozen=True)
class PythonUdfConnectionsE2ETestCase:
    """Unit tests calling a DuckDB Python UDF over several test connections to one database."""

    description: str
    database: str
    concurrency: int
    expected_stdout_fragment: str


@dataclass(frozen=True)
class LocalEngineE2ETestCase:
    """Test case for `sqb test --local` through a stand-in SQLQuack executable."""

    description: str
    arguments: tuple[str, ...]
    expected_exit_code: int
    expected_stdout_fragments: tuple[str, ...] = field(default_factory=tuple)
    expected_stderr_fragments: tuple[str, ...] = field(default_factory=tuple)
    unexpected_stdout_fragments: tuple[str, ...] = field(default_factory=tuple)
    converter_version: str = "0.2.0"
    converter_name: str = "sqlquack"
    expected_conversion_count: int = 0
    expected_converter_arguments: tuple[str, ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class RealLocalEngineE2ETestCase:
    """Test case for `sqb test --local` through the installed SQLQuack executable."""

    description: str
    expected_stdout_fragments: tuple[str, ...]
