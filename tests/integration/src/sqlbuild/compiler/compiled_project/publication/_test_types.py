"""Publication cases and isolated server credentials."""

from dataclasses import dataclass
from typing import NamedTuple


class SnapshotApi(NamedTuple):
    url: str
    location_id: str
    bearer: str


class ClaimedWorker(NamedTuple):
    run_id: str
    claim_id: str
    bearer: str


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
