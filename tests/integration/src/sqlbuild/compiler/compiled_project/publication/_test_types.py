"""Publication cases and isolated server credentials."""

from dataclasses import dataclass, field
from pathlib import Path
from subprocess import Popen
from typing import Any, Literal, NamedTuple


class SnapshotApi(NamedTuple):
    url: str
    location_id: str
    bearer: str


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
    files: dict[str, str] = field(default_factory=dict)
    files_after_publication: dict[str, str] = field(default_factory=dict)
    store_setup: str = ""
    concurrency_limits: dict[str, int] = field(default_factory=dict)
    launcher: Literal["native", "uncooperative", "docker"] = "native"


@dataclass(frozen=True)
class GroupExecutionTestCase(WorkerExecutionTestCase):
    description: str
    on_failure: Literal["skip_dependents", "skip_downstream_jobs", "stop_group"] = "skip_dependents"
    expected_child_statuses: dict[str, tuple[str, ...]] = field(default_factory=dict)
    expected_totals: tuple[str, str | None] = ("skipped", "model:broken_orders")
    expected_marts_started: bool = True


@dataclass(frozen=True)
class WorkerRestartTestCase(WorkerExecutionTestCase):
    description: str
    shutdown_signal: int = 15
    expected_scheduler_exit: int = 0
