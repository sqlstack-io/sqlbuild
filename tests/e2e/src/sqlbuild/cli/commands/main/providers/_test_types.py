from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class EventExporterE2ETestCase:
    description: str
    expected_first_event: str
    expected_context: dict[str, object]


@dataclass(frozen=True)
class CommandOutputE2ETestCase:
    description: str
    expected_context: dict[str, object]


@dataclass(frozen=True)
class NoExporterCommandE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_exit_code: int


@dataclass(frozen=True)
class ReadOnlyProviderE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_exit_code: int


@dataclass(frozen=True)
class ProviderCommandE2ETestCase:
    description: str
    expected_exit_code: int
    expected_marker_entries: tuple[str, ...]
    expected_loaded_rows: tuple[tuple[object, ...], ...]


@dataclass(frozen=True)
class ProviderCommandFailureE2ETestCase:
    description: str
    command: tuple[str, ...]
    repo_files: dict[str, str]
    expected_marker_entries: tuple[str, ...]


@dataclass(frozen=True)
class ProviderCommandSideEffectE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_marker_exists: bool


@dataclass(frozen=True)
class ProviderCommandDiagnosticE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_error_fragment: str


@dataclass(frozen=True)
class ProviderCustomMaterializationE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_marker_entries: tuple[str, ...]
    expected_exit_code: int


@dataclass(frozen=True)
class ProviderCommandConcurrencyE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_marker_entries: tuple[str, ...]
    expected_exit_code: int


@dataclass(frozen=True)
class ProviderHookE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_marker_entries: tuple[str, ...]
    expected_exit_code: int


@dataclass(frozen=True)
class ProviderHookDiagnosticE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_error_fragment: str


@dataclass(frozen=True)
class ProviderHookMaterializationE2ETestCase:
    description: str
    command: tuple[str, ...]
    model_relative_path: str
    model_sql: str
    extra_repo_files: dict[str, str]
    expected_marker_entries: tuple[str, ...]
    expected_exit_code: int


@dataclass(frozen=True)
class ProviderHookContextConflictE2ETestCase:
    description: str
    command: tuple[str, ...]
    context_parameter_name: str
    expected_error_fragment: str


@dataclass(frozen=True)
class ProviderPlanOutputE2ETestCase:
    description: str
    expected_text_fragments: tuple[str, ...]
    expected_provider_name: str
    expected_used_by: tuple[tuple[str, str, str], ...]


@dataclass(frozen=True)
class FilteredSinkFailureE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_exit_code: int
    expected_runs_only: tuple[str, ...]
    expected_errors_only: tuple[str, ...]
    expected_warning: str


@dataclass(frozen=True)
class SlowSinkExitE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_marker_entries: tuple[str, ...]


@dataclass(frozen=True)
class ProviderSecretE2ETestCase:
    description: str
    command: tuple[str, ...]
    env: dict[str, str]
    extra_repo_files: dict[str, str]
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]
    expected_marker_entries: tuple[str, ...]
    unexpected_output_fragments: tuple[str, ...] = ()


@dataclass(frozen=True)
class SharedSinkProviderE2ETestCase:
    description: str
    unused_provider_name: str
    expected_setups: tuple[str, ...]


@dataclass(frozen=True)
class SnowflakeKeyWarningE2ETestCase:
    description: str
    command: tuple[str, ...]
    expected_warnings: int = 1


@dataclass(frozen=True)
class EnvFileE2ETestCase:
    """A project env file, the command run beside it, and what the provider saw."""

    description: str
    # Written after `name` and `adapter`, before the `[connection]` table.
    project_toml: str
    # Files written beside sqlbuild_project.toml, each with its permission bits.
    files: dict[str, tuple[str, int]]
    command: tuple[str, ...]
    env: dict[str, str]
    expected_exit_code: int
    expected_output_fragments: tuple[str, ...]
    # The region the provider read; empty when the task did not run.
    expected_region: str
    unexpected_output_fragments: tuple[str, ...] = ()


@dataclass(frozen=True)
class ProjectDirSpellingTestCase:
    description: str
    # The `--project-dir` argument for the project, such as a symlink to it or a relative path.
    project_argument: Callable[[Path], Path]
    expected_output_fragment: str


@dataclass(frozen=True)
class ChangedCantRunTestCase:
    description: str
    # Selectors built once with the secret set, then planned with --changed without it.
    select: str
    expected_cant_run_nodes: list[str]
    expected_cant_run_readers: list[str]
    expected_unchanged: list[str]
