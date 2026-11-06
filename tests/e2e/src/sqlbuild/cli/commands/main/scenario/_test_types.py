"""Test types for scenario command e2e tests."""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path


@dataclass(frozen=True)
class ScenarioNamespaceE2ETestCase:
    description: str
    namespace_a: str = "job-a"
    namespace_b: str = "job-b"
    expected_total: int = 15
    expected_other_total: int = 27
    expected_exit_code: int = 0
    expected_relation_count: int = 3
    expected_error: str = "C458"


@dataclass(frozen=True)
class ScenarioNamespaceSourceE2ETestCase:
    description: str
    expected_namespace: str | None
    expected_source: str
    project_config: str = ""
    local_config: str = ""
    environment: tuple[tuple[str, str], ...] = ()
    cli_args: tuple[str, ...] = ()


@dataclass(frozen=True)
class ScenarioCliE2ETestCase:
    """Test case for sqb scenario command e2e verification."""

    description: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_stdout_fragments: tuple[str, ...] = field(default_factory=tuple)
    expected_stderr_fragments: tuple[str, ...] = field(default_factory=tuple)
    expected_retained_prefix_count: int | None = None
    disabled_setting: str | None = None


@dataclass(frozen=True)
class ScenarioPartialFixtureE2ETestCase:
    """Test case for implicit partial fixture completion in a scenario."""

    description: str
    repo_files: dict[str, str]
    expected_stdout_fragment: str
    expected_artifact_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ScenarioRuntimeArtifactTestCase:
    """Test case for scenario target/run artifact verification."""

    description: str
    command: tuple[str, ...]
    expected_exit_code: int
    artifact_relative_path: Path
    expected_artifact_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ScenarioPythonHooksCliE2ETestCase:
    """Test case for scenario CLI execution with Python lifecycle hooks."""

    description: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_stdout_fragments: tuple[str, ...]
    expected_hook_log_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class ScenarioLocalRuntimeArtifactTestCase:
    """Test case for local scenario target/run artifact verification."""

    description: str
    scenario_name: str
    capture_command: tuple[str, ...]
    command: tuple[str, ...]
    expected_exit_code: int
    artifact_relative_path: Path
    expected_artifact_fragments: tuple[str, ...]
    additional_project_files: tuple[tuple[str, str], ...] = ()


@dataclass(frozen=True)
class ScenarioLocalCliE2ETestCase:
    """Test case for sqb scenario test --local verification."""

    description: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_stdout_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ScenarioLocalRetainE2ETestCase:
    """Test case for retained local scenario DuckDB verification."""

    description: str
    scenario_name: str
    capture_command: tuple[str, ...]
    command: tuple[str, ...]
    expected_exit_code: int
    expected_stdout_fragments: tuple[str, ...]
    retained_duckdb_relative_path: Path
    retained_count_sql: str
    expected_count: int
    retained_rows_sql: str | None = None
    expected_rows: tuple[tuple[object, ...], ...] = ()
    expected_duckdb_exists: bool = True
    corrupt_jsonl: bool = False
    corrupt_capture_dialect: bool = False
    additional_project_files: tuple[tuple[str, str], ...] = ()


@dataclass(frozen=True)
class ScenarioLocalSnapshotSyncE2ETestCase:
    """Test case for local snapshot sync and refresh verification."""

    description: str
    scenario_name: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_stdout_fragments: tuple[str, ...]
    initial_capture: bool = False
    corrupt_jsonl: bool = False
    update_scenario_after_capture: bool = False
    expected_duckdb_exists: bool = True
    query_when_exists: bool = True
    expected_count: int = 2


@dataclass(frozen=True)
class ScenarioLocalCommittedSnapshotE2ETestCase:
    """Test case for local replay from a pre-written snapshot fixture."""

    description: str
    scenario_name: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_stdout_fragments: tuple[str, ...]
    unexpected_stdout_fragments: tuple[str, ...]
    retained_duckdb_relative_path: Path
    retained_count_sql: str
    expected_count: int
    retained_rows_sql: str
    expected_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class ScenarioAuthoredCheckSqlE2ETestCase:
    """Scenario whose check SQL uses DuckDB syntax a generic regeneration cannot parse."""

    description: str
    scenario_sql: str
    expected_stdout_fragment: str


@dataclass(frozen=True)
class ScenarioUnresolvableCheckSqlE2ETestCase:
    """Scenario whose check SQL contains an unclosed string in the target dialect."""

    description: str
    scenario_sql: str
    expected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ScenarioLocalReplayProjectDialectE2ETestCase:
    """Project-dialect scenario SQL replayed locally from a captured snapshot."""

    description: str
    scenario_name: str
    source_fixture_sql: str
    assertion_sql: str
    expected_stdout_fragment: str


@dataclass(frozen=True)
class ScenarioPromotionE2ETestCase:
    """Test case for scenario table promotion matching build configuration."""

    description: str
    defaults_config: str
    settings_config: str
    model_columns: str
    expected_exit_code: int
    expected_stdout_fragments: tuple[str, ...]
    expect_staged_promotion: bool
    unexpected_stdout_fragments: tuple[str, ...] = ("K011", "R004")


@dataclass(frozen=True)
class ScenarioConcurrencyE2ETestCase:
    """Test case for concurrent scenario execution through the CLI."""

    description: str
    command_args: tuple[str, ...]
    settings_config: str
    expected_header: str


@dataclass(frozen=True)
class ScenarioEmptyFixtureE2ETestCase:
    """Test case for `__empty_fixture()` mocks inside SQL scenarios."""

    description: str
    customer_columns_yaml: str
    expected_exit_code: int
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class ScenarioInterruptE2ETestCase:
    """Test case for interrupting a running scenario command through the real CLI."""

    description: str
    args: tuple[str, ...]
    trigger: str
    trigger_stream: str = "stderr"
    long_model: bool = False
    long_fixture: bool = False
    slow_hook_seconds: float = 0.0
    interrupt_count: int = 1
    expected_notice_counts: tuple[int, int] = (1, 0)
    expected_max_stop_seconds: float = 15.0


@dataclass(frozen=True)
class PromotionConflictE2ETestCase:
    """Test case for the compile-time enforced-contract promotion conflict."""

    description: str
    settings_config: str
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class PromotionConflictReuseE2ETestCase:
    """Test case for the promotion conflict across warm reused compiles."""

    description: str
    settings_steps: tuple[str, ...]
    expected_exit_codes: tuple[int, ...]
    expected_reused: tuple[bool, ...]


@dataclass(frozen=True)
class ScenarioExpectedColumnsE2ETestCase:
    """Test case for comparing a scenario's expected CTE with its model's output columns."""

    description: str
    expected_sql: str
    expected_exit_code: int
    expected_stdout_fragments: tuple[str, ...]
    header_options: str = ""
    unexpected_stdout_fragments: tuple[str, ...] = field(default_factory=tuple)
    expected_json_results: tuple[dict[str, object], ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class ScenarioExpectedColumnsHeaderE2ETestCase:
    """Test case for validating the SCENARIO() expected_columns option."""

    description: str
    header_options: str
    expected_exit_code: int
    expected_stderr_fragment: str


@dataclass(frozen=True)
class ScenarioHelperCteE2ETestCase:
    """Test case for scenario checks that read helper CTEs declared in the same file."""

    description: str
    helper_ctes: str
    check_ctes: str
    expected_exit_code: int
    expected_stdout_fragments: tuple[str, ...]
    unexpected_stdout_fragments: tuple[str, ...] = field(default_factory=tuple)
    expected_json_expected_results: tuple[dict[str, object], ...] = field(default_factory=tuple)
    expected_json_assertion_results: tuple[dict[str, object], ...] = field(default_factory=tuple)


@dataclass(frozen=True)
class ScenarioHelperShadowingE2ETestCase:
    """Test case for a scenario check that redefines a helper CTE name."""

    description: str
    check_ctes: str
    expected_output_fragment: str
    unexpected_output_fragment: str


@dataclass(frozen=True)
class UnmockedInputScenarioE2ETestCase:
    """A scenario that mocks one model input and leaves another, source-backed, real."""

    description: str
    expected_fragments: tuple[str, ...]
    unexpected_fragment: str


@dataclass(frozen=True)
class ScenarioInlineProjectE2ETestCase:
    """One scenario command over an inline project and the output it must print."""

    description: str
    repo_files: dict[str, str]
    command: tuple[str, ...]
    expected_exit_code: int
    expected_stdout_fragments: tuple[str, ...]


@dataclass(frozen=True)
class LocalVariantScenarioTestCase:
    """A captured scenario whose fixture holds a VARIANT, replayed locally in strict mode."""

    description: str
    repo_files: dict[str, str]
    scenario_name: str
    expected_stdout_fragment: str
    expected_returncode: int = 0


@dataclass(frozen=True)
class SnapshotHelperEditTestCase:
    """A captured scenario, one edit to its files, and what strict local replay reports."""

    description: str
    repo_files: dict[str, str]
    scenario_name: str
    edited_path: str
    edited_from: str
    edited_to: str
    expected_exit_code: int
    expected_stdout_fragment: str


@dataclass(frozen=True)
class CapturedVariantTextScenarioTestCase:
    """A Snowflake-captured VARIANT stored as JSON text, compared with a PARSE_JSON expected."""

    description: str
    captured_payload_text: str
    expected_payload_json: str
    expected_returncode: int
    expected_stdout_fragment: str


@dataclass(frozen=True)
class EscapedQuoteScenarioTestCase:
    """A Snowflake project whose SQL holds backslash escapes before compile markers."""

    description: str
    scenario_name: str
    expected_compiled_fragments: tuple[str, ...]
    expected_stdout_fragment: str


@dataclass(frozen=True)
class CapturedLambdaScenarioTestCase:
    """A Snowflake-captured scenario whose model uses an arrow lambda, replayed in DuckDB."""

    description: str
    scenario_name: str
    expected_stdout_fragment: str
