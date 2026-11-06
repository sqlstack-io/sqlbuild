"""Dataclass-backed execution history integration cases."""

from dataclasses import dataclass


@dataclass(frozen=True)
class SQLitePathCase:
    description: str
    expected_filename: str | None


@dataclass(frozen=True)
class SQLiteMigrationCase:
    description: str
    initial_version: int
    expected_version: int


@dataclass(frozen=True)
class SQLitePersistenceCase:
    description: str
    expected_event_count: int
    expected_run_count: int


@dataclass(frozen=True)
class SQLiteTimeoutCase:
    description: str
    timeout: object
    expected_error: str


@dataclass(frozen=True)
class SQLiteTransactionFailureCase:
    description: str
    expected_event_count: int
    expected_run_count: int


@dataclass(frozen=True)
class SQLiteInterruptCase:
    description: str
    expected_stdout: str


@dataclass(frozen=True)
class PostgresHistoryCase:
    description: str
    expected_event_count: int
    expected_run_count: int


@dataclass(frozen=True)
class ConcurrentSQLiteHistoryCase:
    description: str
    expected_resource_start_count: int
    expected_resource_terminal_count: int
