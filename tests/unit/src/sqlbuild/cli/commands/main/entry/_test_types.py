from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path

from sqlbuild._native import (
    CliUserError,
    ColumnLineageMode,
    CompileLineageMode,
    ProjectConfigError,
    SpecConfigError,
)
from sqlbuild.compiler.compile.exceptions import CompileInputError


@dataclass(frozen=True)
class NoExporterFastPathTestCase:
    description: str
    expected_thread_delta: int


@dataclass(frozen=True)
class CliCaptureOutcomeTestCase:
    description: str
    expected_exit_code: int
    expected_complete: bool
    expected_error_type: type[BaseException] | None = None


@dataclass(frozen=True)
class InterruptedCommandTestCase:
    description: str
    argv: list[str]
    expected_error_type: type[BaseException]


@dataclass(frozen=True)
class ProjectCreationRoutingTestCase:
    description: str
    argv: tuple[str, ...]
    handler_name: str
    expected_exit_code: int


@dataclass(frozen=True)
class ScopeArgumentMisuseCase:
    """One invalid scope argument combination."""

    description: str
    argv: tuple[str, ...]
    expected_exit_code: int = 2


@dataclass(frozen=True)
class ScopeEntryCase:
    description: str
    expected_exit_code: int


@dataclass(frozen=True)
class SkillFreshnessNoticeTestCase:
    description: str
    expected_exit_code: int
    expected_stderr_fragment: str


@dataclass(frozen=True)
class MainTestCase:
    description: str
    argv: list[str]
    expected_exit_code: int
    expected_project_dir: Path | None = None
    expected_no_sql_validation: bool = False
    expected_no_cache: bool = False
    expected_full_refresh: bool = False
    expected_selection_diagnostics: bool = False
    expected_load_sources: bool | None = None
    expected_reload: bool = False
    expected_allow_snapshot_full_refresh: bool = False
    expected_allow_snapshot_schema_change: bool = False
    expected_no_color: bool = False
    expected_debug: bool = False
    expected_verbose: bool = False
    expected_json: bool = False
    expected_json_output_path: Path | None = None
    expected_run_tests: bool = True
    expected_run_audits: bool = True
    expected_fail_on_error: bool = False
    expected_fail_on_stale: bool = False
    expected_state: bool = False
    expected_manifest: bool = False
    expected_dag: str | None = None
    expected_compile_lineage_mode: CompileLineageMode = CompileLineageMode.FAST
    expected_column_lineage_mode: ColumnLineageMode = ColumnLineageMode.RICH
    expected_scenario_selectors: tuple[str, ...] = ()
    expected_select: tuple[str, ...] = ()
    expected_selector_count: int = 0
    expected_exclude: tuple[str, ...] = ()
    expected_dbt_args: tuple[str, ...] = ()
    expected_skills_global: bool = False
    expected_skills_targets: tuple[str, ...] = ()
    expected_skills_force: bool = False
    expected_playground_template: str = "waffle_shop"
    expected_state_command: str | None = None
    expected_state_checkpoint_command: str | None = None
    expected_state_checkpoint_id: str | None = None
    expected_state_backup_id: str | None = None
    expected_auto_approve: bool = False
    expected_vars: dict[str, object] | None = None
    expected_command_vars: dict[str, object] = field(default_factory=dict)
    expected_direct_state_history_versions: int | None = None
    expected_dbt_init_project_dir: str | None = None
    expected_dbt_init_profiles_dir: str | None = None
    expected_dbt_init_profile_name: str | None = None
    expected_dbt_init_target_name: str | None = None
    expected_dbt_init_sqb_output_dir: str | None = None
    expected_dbt_init_dry_run: bool = False
    expected_dbt_init_overwrite: bool = False
    expected_dbt_init_skip_dbt_debug: bool = False


@dataclass(frozen=True)
class CompileJsonErrorRenderingTestCase:
    description: str
    argv: list[str]
    error_factory: Callable[[], Exception]
    expected_stdout_diagnostics: tuple[dict[str, object], ...]
    expected_stderr_fragment: str


@dataclass(frozen=True)
class MainErrorRenderingTestCase:
    description: str
    argv: list[str]
    error_type: (
        type[CliUserError]
        | type[ProjectConfigError]
        | type[CompileInputError]
        | type[SpecConfigError]
        | type[ValueError]
    )
    error_factory: Callable[[Path], Exception]
    expected_stderr_fragment: str
    expected_exit_code: int


@dataclass(frozen=True)
class AuditAggregateRenderingTestCase:
    description: str
    expected_exit_code: int
    expected_failure_ids: tuple[str, ...]
    expected_json_filename: str


@dataclass(frozen=True)
class ObservabilityCleanupCase:
    description: str
    expected_original_error: str


@dataclass(frozen=True)
class MachineProgressRoutingCase:
    description: str
    json: bool
    json_output: Path | None
    expected_stdout_fragment: str
    expected_stderr_fragment: str
    command: str | None = None
    dbt_args: tuple[str, ...] = ()
    expected_stdout_excludes: str = "Project compile  FAIL"


@dataclass(frozen=True)
class UnreadableErrorTestCase:
    description: str
    expected_error_type: str
    expected_message: str
