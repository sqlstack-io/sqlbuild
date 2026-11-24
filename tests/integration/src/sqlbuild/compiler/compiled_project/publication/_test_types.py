"""Publication cases and isolated server credentials."""

import signal
import sqlite3
from collections.abc import Callable
from contextlib import AbstractContextManager
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from subprocess import Popen
from typing import Any, Literal, NamedTuple


class SnapshotApi(NamedTuple):
    url: str
    location_id: str
    bearer: str

    def __repr__(self) -> str:
        return f"SnapshotApi(url={self.url!r}, location_id={self.location_id!r}, bearer=<redacted>)"


class ClaimedWorker(NamedTuple):
    run_id: str
    claim_id: str
    bearer: str


class ScheduledWorker(NamedTuple):
    api: SnapshotApi
    project: Path
    control_store: Path
    process: Popen[bytes]
    log: Path
    docker: "DockerLocation | None" = None


class DockerLocation(NamedTuple):
    image_id: str
    container: str
    socket: Path


class WorkerDeployment(NamedTuple):
    kind: str
    configuration: dict[str, Any]
    publication_arguments: tuple[str, ...] = ()
    docker: DockerLocation | None = None


@dataclass(frozen=True)
class SnapshotPublicationTestCase:
    description: str
    target: str
    expected_generation: int
    expected_nodes: frozenset[str]


@dataclass(frozen=True)
class PlainHttpWarningTestCase:
    description: str
    host: str
    expected_warnings: int


@dataclass(frozen=True)
class SnapshotWorkerTestCase:
    description: str
    model_file: str
    original_sql: str
    replacement_sql: str
    expected_error: str


@dataclass(frozen=True)
class LocationReloadTestCase:
    description: str
    target: str
    expected_generation: int
    expected_stop_code: int
    database: str
    expected_connection: bool


@dataclass(frozen=True)
class LocationShutdownTestCase:
    description: str
    signal: int
    expected_exit_code: int
    minimum_heartbeats: int
    blocked_command: str


@dataclass(frozen=True)
class LocationDatabaseTestCase:
    description: str
    database: str
    expected_generation: int
    connection_toml: str


@dataclass(frozen=True)
class LocationWorkTestCase:
    description: str
    database: str
    expected_connection: bool


@dataclass(frozen=True)
class WorkerExecutionTestCase:
    description: str
    model_sql: str
    warehouse_query: str
    expected_status: str
    expected_rows: list[tuple[str | int | bool | None, ...]]
    expected_node_count: int = 1
    # Node counts by status when the nodes don't all share the run's status.
    expected_node_counts: dict[str, int] | None = None
    expected_origin: Literal["schedule", "sensor"] = "schedule"
    files: dict[str, str] = field(default_factory=dict)
    # The scheduled job written before `files`; cases that declare their own jobs set it to {}.
    default_job_files: dict[str, str] = field(
        default_factory=lambda: {
            "orchestration/jobs/annual_orders_job.toml": (
                'name = "annual_orders_job"\n'
                'description = "Build orders."\n'
                'select = "orders"\n'
                'schedule = { cron = "0 0 * * *", timezone = "UTC", '
                'starts_at = "2000-01-01T00:00:00Z" }\n'
            ),
        }
    )
    files_after_publication: dict[str, str] = field(default_factory=dict)
    store_setup: str = ""
    store_edit: Callable[[sqlite3.Connection], None] = lambda _: None
    pools: dict[str, int] = field(default_factory=dict)
    launcher: Literal["native", "uncooperative", "docker"] = "native"
    # A Docker launcher's [launchers.docker.run] settings and the worker HostConfig they produce.
    launcher_settings: dict[str, Any] = field(default_factory=dict)
    expected_host_config: dict[str, Any] = field(default_factory=dict)
    warehouse_setup: Callable[[Path], None] = lambda _: None
    # Each run node as (key, status, error code, skipped because of, error message).
    expected_run_nodes: list[tuple[str, str, str | None, str | None, str | None]] = field(
        default_factory=list
    )
    # Whether the build compiles again instead of planning from the compile the worker verified.
    expected_compiles_again: bool = False


@dataclass(frozen=True)
class NodeResultsTestCase(WorkerExecutionTestCase):
    description: str
    # Each run node as (key, status, rows affected, table rows, rows so far, skipped because of).
    expected_node_results: list[tuple[str, str, int | None, int | None, int | None, str | None]] = (
        field(default_factory=list)
    )


@dataclass(frozen=True)
class ClaimedLoadTestCase(WorkerExecutionTestCase):
    description: str
    expected_write_set: frozenset[str] = frozenset()


@dataclass(frozen=True)
class UnclaimedLoadTestCase:
    description: str
    files: dict[str, str]
    store_edit: Callable[[sqlite3.Connection], None]
    refused_writer: str
    expected_write_set: frozenset[str]


@dataclass(frozen=True)
class CheckExecutionTestCase(WorkerExecutionTestCase):
    description: str
    expected_check_outcome: str = "success"


@dataclass(frozen=True)
class RunOutputTestCase(WorkerExecutionTestCase):
    description: str
    expected_lines: int = 0
    expected_error_lines: list[int] = field(default_factory=list)
    expected_logs: list[tuple[str, str, str]] = field(default_factory=list)


@dataclass(frozen=True)
class NulLogTestCase(WorkerExecutionTestCase):
    description: str
    backend: Literal["sqlite", "postgres"] = "sqlite"
    expected_logs: list[tuple[str, str]] = field(default_factory=list)


@dataclass(frozen=True)
class PhaseEventsTestCase(WorkerExecutionTestCase):
    description: str
    backend: Literal["sqlite", "postgres"] = "sqlite"
    # Each phase in order, and whether it reports a duration.
    expected_phases: tuple[tuple[str, bool], ...] = ()


@dataclass(frozen=True)
class NulFailureTestCase(WorkerExecutionTestCase):
    description: str
    backend: Literal["sqlite", "postgres"] = "sqlite"
    expected_error_end: str = ""


@dataclass(frozen=True, kw_only=True)
class PersonJourneyTestCase(WorkerExecutionTestCase):
    description: str
    default_role: Literal["none", "viewer"]
    expected_metadata_access: Callable[[], AbstractContextManager[Any]]


@dataclass(frozen=True)
class GroupExecutionTestCase(WorkerExecutionTestCase):
    description: str
    expected_child_statuses: dict[str, tuple[str, ...]] = field(default_factory=dict)
    expected_totals: tuple[str, str | None] = ("skipped", "model:broken_orders")
    expected_marts_started: bool = True


@dataclass(frozen=True)
class WorkerRestartTestCase(WorkerExecutionTestCase):
    description: str
    shutdown_signal: int = 15
    expected_scheduler_exit: int = 0


@dataclass(frozen=True)
class BackfillExecutionTestCase(WorkerExecutionTestCase):
    description: str
    window_arguments: tuple[str, ...] = ()
    expected_window: dict[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class CalendarRejectionTestCase:
    description: str
    expression: str
    expected_message: str
    horizon: str = "8s"
    timezone: str = "UTC"


@dataclass(frozen=True)
class CalendarRefreshTestCase:
    description: str
    expected_timezone: str


@dataclass(frozen=True)
class WarehouseHistoryTestCase:
    description: str
    adapter_name: str
    expected_page_lengths: tuple[int, ...]
    connection_key: str
    journal_modes: tuple[str, ...]
    append_fingerprints: Callable[..., None]


@dataclass(frozen=True)
class NewestVersionsTestCase:
    description: str
    adapter_name: str
    connection_key: str
    newest_versions: int | None
    expected_runs: dict[str, list[str]]


@dataclass(frozen=True)
class PageBytesTestCase:
    description: str
    adapter_name: str
    connection_key: str
    wide_rows: int
    definition_bytes: int
    expected_min_pages: int


@dataclass(frozen=True)
class CatalogConnectionTestCase:
    description: str
    adapter_name: str
    connection_key: str
    expected_run_id: str


@dataclass(frozen=True)
class StateEventsTestCase(CatalogConnectionTestCase):
    description: str
    expected_run_id: str
    fixture_name: str
    append_history: Callable[..., None]
    legacy_columns: tuple[str, ...] = ()


@dataclass(frozen=True)
class SyncPacingTestCase:
    """A tick-paced sync waits `added_rows / 100` ticks of 250 ms; git is asked every 2 s."""

    description: str
    adapter_name: str
    connection_key: str
    expected_run_id: str
    added_rows: int
    idle_seconds: float
    expected_max_seconds: float
    expected_max_git_calls: int


@dataclass(frozen=True)
class AutomaticHistoryTestCase:
    description: str
    adapter_name: str
    connection_key: str
    expected_run_id: str
    wide: bool = False
    initial_seconds: int = 8
    expected_average_bytes: int = 0


@dataclass(frozen=True)
class FingerprintHistoryTestCase:
    description: str
    adapter_name: str
    connection_key: str
    observed_at: datetime
    journal_modes: tuple[str, ...]
    fixture_paths: tuple[Path, ...]
    expected_legacy_runs: tuple[str, ...]


@dataclass(frozen=True)
class DevelopmentTestCase:
    description: str
    target: str
    arguments: tuple[str, ...]
    control_path: str
    expected_rows: list[tuple[int, int]]
    local_config: str = ""


@dataclass(frozen=True)
class DevelopmentExposureTestCase:
    description: str
    arguments: tuple[str, ...]
    expected_status: int
    expected_lines: tuple[str, ...]


@dataclass(frozen=True)
class DevelopmentAllowHostRefusalTestCase:
    description: str
    value: str
    expected_exit_code: int


@dataclass(frozen=True)
class DevelopmentDiscoveryTestCase:
    description: str
    expected_imports: int


@dataclass(frozen=True)
class DevelopmentWarningTestCase:
    description: str
    expected_line: str
    hidden_line: str


@dataclass(frozen=True)
class DevelopmentTerminalTestCase:
    description: str
    arguments: list[str]
    unset: frozenset[str]
    # How the client reads the code: `sys.stdin.readline()` or `getpass.getpass(...)` on /dev/tty.
    read_code: str
    expected_ready: str
    expected_sign_ins: list[str]
    # Whether the typed code is visible: a plain prompt echoes it, a getpass prompt never does.
    expected_code_shown: bool


@dataclass(frozen=True)
class DevelopmentStartupStopTestCase:
    description: str
    keys: bytes
    expected_exit_code: int
    expected_result: str


@dataclass(frozen=True)
class DevelopmentHangUpTestCase:
    description: str
    expected_exit_code: int
    expected_status: str


@dataclass(frozen=True)
class ForcedWorkerStopTestCase:
    description: str
    first: signal.Signals
    second: signal.Signals
    expected_exit_code: int
    expected_worker_exit_code: int


@dataclass(frozen=True)
class WorkerHangUpTestCase:
    description: str
    expected_exit_code: int
    expected_status: str


@dataclass(frozen=True)
class DevelopmentActiveRunStopTestCase:
    description: str
    keys: bytes
    expected_exit_code: int
    expected_lines: list[str]
    expected_status: str


@dataclass(frozen=True)
class DevelopmentKeyStopTestCase:
    description: str
    keys: bytes
    expected_exit_code: int
    expected_result: str


@dataclass(frozen=True)
class DevelopmentSignalStopTestCase:
    description: str
    stop: signal.Signals
    expected_exit_code: int
    expected_result: str


@dataclass(frozen=True)
class DevelopmentStartFailureTestCase:
    description: str
    configuration: str
    expected_exit_code: int
    expected_lines: list[str]


@dataclass(frozen=True)
class DevelopmentEnvFileTestCase:
    description: str
    first_env_file: str
    second_env_file: str
    expected_card_row: str
    # The parts of the restart event line, in order of appearance.
    expected_restart: tuple[str, ...]
    # The `--project-dir` argument for the project, such as a symlink to it.
    project_argument: Callable[[Path], Path] = lambda project: project


@dataclass(frozen=True)
class WorkerReportTestCase:
    description: str
    execution: WorkerExecutionTestCase
    # The ad-hoc command an admin launches once the scheduled run ends; that run is the one checked.
    adhoc: list[str]
    expected_status: str
    expected_nodes: list[tuple[str, str]]
    # A line of the run's human results, which its printed output must hold.
    expected_results: str
    # Recall event types the worker always records; the report never fed them.
    expected_recall_events: frozenset[str]


@dataclass(frozen=True)
class DevelopmentMissingSecretsTestCase:
    description: str
    first_env_file: str
    second_env_file: str
    # The target's env file, relative to the project.
    env_file_name: str
    # What the printed warning says while the secrets are missing.
    expected_warning: tuple[str, ...]


@dataclass(frozen=True)
class ComposeDeploymentTestCase:
    description: str
    expected_warehouse_rows: list[list[int]]


@dataclass(frozen=True)
class InstalledRecoveryTestCase:
    description: str
    executable: str
    expected_rejection: str


@dataclass(frozen=True)
class CatalogInspectionTestCase:
    description: str
    adapter_name: str
    connection_key: str
    expected_table_type: str
    expected_view_type: str
    expected_metadata_rows: int | None = None


@dataclass(frozen=True)
class CatalogBoundaryTestCase:
    description: str
    expected_page_delivered: bool


@dataclass(frozen=True)
class ReadFailureTestCase:
    description: str
    table: str
    expected_stderr: str
    expected_reply: dict[str, Any]


@dataclass(frozen=True)
class StatisticsRecoveryTestCase:
    description: str
    expected_log: str


@dataclass(frozen=True)
class StatisticsHorizonTestCase:
    description: str
    server_arguments: tuple[str, ...]
    expected_relations: tuple[str, ...]


@dataclass(frozen=True)
class AuditHorizonTestCase:
    description: str
    fixture: str
    expected_retained: int


@dataclass(frozen=True)
class RetentionRejectionTestCase:
    description: str
    arguments: tuple[str, ...]
    expected_error: str


@dataclass(frozen=True)
class StatisticsHorizonRejectionTestCase:
    description: str
    days: str
    expected_error: str


@dataclass(frozen=True)
class DenialMetadataTestCase:
    description: str
    expected_denial_count: int


@dataclass(frozen=True)
class CliLoginTestCase:
    description: str
    authentication: str
    expected_person_name: str


@dataclass(frozen=True)
class CliCleanupTestCase:
    description: str
    server: str
    expected_exit: int


@dataclass(frozen=True)
class SchedulerCrashTestCase:
    description: str
    fixture: str
    expected_anomalies: dict[str, int]
    expected_minimum_kills: int


@dataclass(frozen=True)
class SharedAuditPublicationTestCase:
    description: str
    create_project: Callable[[Path], None]
    target: str
    expected_audits: frozenset[str]
    expected_count: int


@dataclass(frozen=True)
class ProjectRolesTestCase:
    description: str
    roles_file: str
    expected_warning: str
    expected_denial: str


@dataclass(frozen=True)
class ProjectRoleRunTestCase:
    description: str
    role: str
    expected_response: tuple[int, Any]


@dataclass(frozen=True, kw_only=True)
class ChangedJobTestCase(WorkerExecutionTestCase):
    description: str
    project_toml: str
    warehouse_file: str
    consumption_table: str
    raw_tables_sql: tuple[str, ...]
    new_order_sql: str
    execute: Callable[[Path, str], list[tuple[Any, ...]]]
    expected_runs: tuple[tuple[str, frozenset[str], dict[str, int], frozenset[str]], ...]


@dataclass(frozen=True)
class AdhocVerbTestCase:
    """An ad-hoc verb a worker runs after its prerequisites, and the run's node and check rows."""

    description: str
    prerequisites: list[list[str]]
    command: list[str]
    expected_selection: list[str]
    expected_run_nodes: list[tuple[str, str, str]]
    # Files the worker's checkout gains after it verified the final run, which must not run.
    rewrites: dict[str, str] = field(default_factory=dict)
    expected_status: str = "success"
    # Files that replace the published project's own before publication.
    files: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class AdhocRunTestCase:
    """An ad-hoc command a person checks and launches through `sqb server`, and what follows."""

    description: str
    person: Literal["admin", "developer"]
    command: list[str]
    expected_permissions: list[list[Any]]
    expected_status: str
    expected_reason: str
    expected_statements: bool
    expected_customers_tables: int
    job_settings: str = ""
    expected_scheduled_status: str = "success"
    expected_worker_log: str = ""


@dataclass(frozen=True)
class AdhocLoadVarsTestCase:
    """An ad-hoc load or run with variables, who may launch it, and what the worker did."""

    description: str
    person: Literal["admin", "developer"]
    command: list[str]
    expected_permissions: list[list[Any]]
    # The HTTP status a refused launch returns; None when it launches and succeeds.
    expected_refusal: int | None
    expected_inventory_tables: int
    day_query: str = "SELECT count(*) FROM duckdb_tables() WHERE table_name = 'daily_customers'"
    expected_day_rows: list[tuple[int]] = field(default_factory=lambda: [(0,)])
    # One entry per recorded compile: whether its hash differs from the published one.
    expected_compiled: list[bool] = field(default_factory=list)


@dataclass(frozen=True)
class ChangedPlanPreviewTestCase:
    description: str
    edited_model: str
    edit: tuple[str, str]
    expected_changed: dict[str, Any]


@dataclass(frozen=True)
class PlanPreviewTestCase:
    description: str
    edited_files: dict[str, str]
    injected_sql: tuple[str, ...]
    expected_window_models: tuple[str, ...]
    expected_unplanned_cursor_models: tuple[str, ...]
    expected_added_unknowns: tuple[str, ...]


@dataclass(frozen=True)
class HeldPlanTestCase:
    description: str
    settings: tuple[dict[str, object], ...]
    fresh: frozenset[int]
    expected_runs: tuple[tuple[str, ...], ...]
    expected_compiles: int
    expected_held_plans: int
    expected_held_test_plannings: int
    expected_reused: tuple[bool, ...]


@dataclass(frozen=True)
class RulesAtLoadTestCase:
    description: str
    model_sql: str
    expected_health: dict[str, Any] = field(default_factory=dict)
    expected_terminal: str = ""
    expected_rule_evaluations: int = 0


@dataclass(frozen=True)
class UnknownSourceAccessTestCase:
    description: str
    added_files: dict[str, str]
    selection: list[str]
    expected_unknown: str


@dataclass(frozen=True)
class SyncedFreshnessTestCase:
    description: str
    files: dict[str, str]
    expected_absent: str
    expected_line: str


@dataclass(frozen=True)
class FailedPlanTestCase:
    description: str
    files: dict[str, str]
    edited_file: str
    edit: tuple[str, str]
    selection: list[str]
    expected_code: str
    expected_reason: str


@dataclass(frozen=True)
class ChangedBuildPublicationTestCase:
    description: str
    edit: tuple[str, str]
    expected_published: dict[str, list[str]]


@dataclass(frozen=True)
class NoDynamicAdapterSyncTestCase:
    description: str
    fixture: str
    expected_open_holds: tuple[str, ...]


@dataclass(frozen=True)
class ChangedBuildReplanTestCase:
    description: str
    edit: tuple[str, str]
    expected_published: dict[str, list[str]]
    expected_rebuilt: dict[str, list[str]]


@dataclass(frozen=True)
class ProviderSecretsHealthTestCase:
    description: str
    # Providers the target's allow_missing_secrets lists.
    allowed: tuple[str, ...]
    expected_missing: int
    expected_allowed: bool
    # What the location prints about them, and what it must not print.
    expected_output_fragments: tuple[str, ...]
    unexpected_output_fragments: tuple[str, ...]


@dataclass(frozen=True)
class DevelopmentSecretReloadTestCase:
    description: str
    # The project's .env before and after the secret is added.
    first_env_file: str
    second_env_file: str
    expected_first_status: str
    # (node key, status, error code) of the node that uses the provider in the first run.
    expected_first_node: tuple[str, str, str | None]
    expected_second_status: str


@dataclass(frozen=True)
class LocationCantRunTestCase:
    description: str
    # The plan request's settings, such as variables that make the location compile again.
    settings: dict[str, object]
    expected_cant_run: list[dict[str, object]]


@dataclass(frozen=True)
class DevelopmentInvalidEnvFileTestCase:
    description: str
    # The project's .env at start, and the edit the parser rejects.
    first_env_file: str
    invalid_env_file: str
    expected_status: str
    expected_reason: str
    # A value in the invalid line, which no output may show.
    secret_text: str


@dataclass(frozen=True)
class SensorTrialCase:
    """One `sqb sensor try`: the function's answer, the event choice and what the command says."""

    description: str
    mode: str
    # `{event}` and `{run}` stand for the watched run's event ID and run ID.
    choice: tuple[str, ...]
    expected_exit: int
    expected_stdout: str


@dataclass(frozen=True)
class SensorFunctionsTestCase:
    description: str
    worker: WorkerExecutionTestCase
    expected_failure_code: str
    expected_failure: str
    expected_started_job: str
    expected_trials: tuple[SensorTrialCase, ...]
    # Provider secret values the location's environment sets; no stored tick or trial output
    # may show them.
    expected_masked: dict[str, str]
    # Provider secret values only a `.env` file sets, masked the same way.
    dotenv_secrets: dict[str, str]


class SensorHarness(NamedTuple):
    """A sensor test's files, sign-in and CLI environment."""

    root: Path
    origin: str
    certificate: Path
    modes: Path
    calls: Path
    binary: str
    environment: dict[str, str]


@dataclass(frozen=True)
class CheckSensorsTestCase:
    description: str
    worker: WorkerExecutionTestCase
    # Each failed or warned check the check sensor is called with, as (check, status).
    expected_check_events: list[tuple[str, str]]
    # The run event's failed checks and failed nodes.
    expected_failed_checks: list[str]
    expected_failed_nodes: list[str]


@dataclass(frozen=True)
class PythonFailureTestCase(WorkerExecutionTestCase):
    description: str
    expected_error_start: str = ""
    expected_error_end: str = ""
    expected_error_below: int = 0


@dataclass(frozen=True)
class SecretAdviceTestCase:
    description: str
    command: tuple[str, ...]
    # The target's env file, relative to the project.
    env_file_name: str
    expected_help: str
    expected_flag_suggested: bool
    expected_failure: bool = True


@dataclass(frozen=True)
class DevelopmentRepublishTestCase:
    description: str
    release: str
    expected_exit_code: int
    expected_items: int


@dataclass(frozen=True)
class SessionRepublishTestCase:
    description: str
    reason: str
    expected_exit_code: int
    expected_state: str


@dataclass(frozen=True)
class NodeStatusTestCase:
    description: str
    first_depends_on: str
    job_select: str
    job_settings: str
    files: dict[str, str]
    expected_statuses_during_second: dict[str, str]
    expected_rerun_selection: frozenset[str]


@dataclass(frozen=True)
class HungCheckTestCase:
    description: str
    check_every: str
    check_starts_within_seconds: float
    expected_message: str


@dataclass(frozen=True)
class EmptyDiscoveryTestCase:
    description: str
    first_items: tuple[str, ...]
    expected_nodes: frozenset[str]
    expected_kept_items: tuple[str, ...]


@dataclass(frozen=True)
class ParallelItemsTestCase:
    description: str
    first_depends_on: str
    job_select: str
    expected_status: str
    expected_statuses: dict[str, str]


@dataclass(frozen=True)
class ItemLogsTestCase:
    description: str
    depends_on: str
    job_select: str
    job_settings: str
    max_parallel: int
    outcomes: dict[str, str]
    files: dict[str, str]
    expected_statuses: dict[str, str]


@dataclass(frozen=True)
class RunningItemsTestCase:
    description: str
    depends_on: str
    job_select: str
    job_settings: str
    max_parallel: int
    files: dict[str, str]
    expected_running: int
    expected_pending: int


@dataclass(frozen=True)
class WaitingCheckTestCase:
    description: str
    check_seconds: int
    expected_finished: bool
    expected_restarts: int


@dataclass(frozen=True)
class CancelledChainTestCase:
    description: str
    max_parallel: int
    expected_first_run: list[str]
    expected_second_run: list[str]
    expected_statuses: dict[str, str]
    expected_torn_down: bool


@dataclass(frozen=True)
class InterruptedStepsTestCase:
    description: str
    expected_status: str
    expected_statuses: dict[str, str]
    expected_notice: str


@dataclass(frozen=True)
class WaitingRepublishTestCase:
    description: str
    check_seconds: int
    release: str
    expected_exit_code: int
    expected_check_finished: bool


@dataclass(frozen=True)
class KeyedRepublishTestCase:
    description: str
    role: str
    sign_in_arguments: tuple[str, ...]
    expected_exit_code: int
    expected_stderr: str
    expected_generation: int
    expected_release: str


@dataclass(frozen=True)
class CancelledWriteTestCase:
    description: str
    job_select: str
    job_settings: str
    files: dict[str, str]
    expected_statuses: dict[str, str]
    expected_rerun: list[str]


@dataclass(frozen=True)
class UnknownDependencyTestCase:
    description: str
    first: str
    then: str
    removed: tuple[str, ...]
    missing: str
    expected_nodes: frozenset[str]


@dataclass(frozen=True)
class UnknownDependencyDiscoverTestCase:
    description: str
    dependency: str
    expected_error: str


@dataclass(frozen=True)
class UnknownTagTestCase:
    description: str
    backend: str
    first: str
    edits: dict[str, str]
    expected_nodes: frozenset[str]
    expected_error: str
