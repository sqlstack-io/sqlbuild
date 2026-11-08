"""Publication cases and isolated server credentials."""

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
    expected_rows: list[tuple[int]]
    expected_node_count: int = 1
    expected_origin: Literal["schedule", "sensor"] = "schedule"
    files: dict[str, str] = field(default_factory=dict)
    files_after_publication: dict[str, str] = field(default_factory=dict)
    store_setup: str = ""
    concurrency_limits: dict[str, int] = field(default_factory=dict)
    launcher: Literal["native", "uncooperative", "docker"] = "native"


@dataclass(frozen=True)
class CheckExecutionTestCase(WorkerExecutionTestCase):
    description: str
    expected_check_outcome: str = "success"


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


@dataclass(frozen=True, kw_only=True)
class ChangedJobTestCase(WorkerExecutionTestCase):
    description: str
    project_toml: str
    warehouse_file: str
    consumption_table: str
    raw_tables_sql: tuple[str, ...]
    new_order_sql: str
    execute: Callable[[Path, str], list[tuple[Any, ...]]]
    expected_runs: tuple[tuple[str, frozenset[str], dict[str, int]], ...]


@dataclass(frozen=True)
class AdhocVerbTestCase:
    """An ad-hoc verb a worker runs after its prerequisites, and the run's node and check rows."""

    description: str
    prerequisites: list[list[str]]
    command: list[str]
    expected_selection: list[str]
    expected_run_nodes: list[tuple[str, str, str]]


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
