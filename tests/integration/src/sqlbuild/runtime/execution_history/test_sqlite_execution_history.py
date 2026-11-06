"""SQLite migration, reopen, reconciliation, and atomicity integration tests."""

import sqlite3
import subprocess
import sys
from concurrent.futures import Future, ThreadPoolExecutor, TimeoutError
from datetime import UTC, datetime
from pathlib import Path
from textwrap import dedent
from threading import Event
from typing import Any, cast
from unittest.mock import Mock

import pytest

from sqlbuild.execution_history import (
    EventFilter,
    EventPage,
    ExecutionHistoryStorageError,
    InvalidCursorError,
    ProjectionConsistencyError,
    RunFilter,
    RunPage,
    RunRecord,
    StoredEvent,
    UnsupportedSchemaVersionError,
    canonical_event_content,
)
from sqlbuild.observability import LifecycleEvent, OpaqueLifecycleEvent
from sqlbuild.sqlite_history import SQLiteExecutionHistory
from tests.integration.src.sqlbuild.runtime.execution_history._test_types import (
    SQLiteInterruptCase,
    SQLiteMigrationCase,
    SQLitePathCase,
    SQLitePersistenceCase,
    SQLiteTimeoutCase,
    SQLiteTransactionFailureCase,
)
from tests.integration.src.sqlbuild.runtime.execution_history.helpers import (
    append_atomically,
    lifecycle_event,
    reconcile_with_coordination,
)


class _TransactionFailureConnection:
    def __init__(
        self,
        connection: sqlite3.Connection,
        *,
        fail_commit_once: bool = False,
        fail_rollback_once: bool = False,
        fail_projection_once: bool = False,
    ) -> None:
        self._connection: sqlite3.Connection = connection
        self._fail_commit_once: bool = fail_commit_once
        self._fail_rollback_once: bool = fail_rollback_once
        self._fail_projection_once: bool = fail_projection_once

    @property
    def in_transaction(self) -> bool:
        return self._connection.in_transaction

    def execute(self, sql: str, parameters: Any = ()) -> sqlite3.Cursor:
        if sql == "DELETE FROM run_projection" and self._fail_projection_once:
            self._fail_projection_once = False
            raise sqlite3.OperationalError("controlled body failure")
        if sql == "COMMIT" and self._fail_commit_once:
            self._fail_commit_once = False
            raise sqlite3.OperationalError("controlled commit failure")
        if sql == "ROLLBACK" and self._fail_rollback_once:
            self._fail_rollback_once = False
            _ = self._connection.execute(sql)
            raise sqlite3.OperationalError("controlled rollback failure")
        return self._connection.execute(sql, parameters)

    def executemany(self, sql: str, parameters: Any) -> sqlite3.Cursor:
        return self._connection.executemany(sql, parameters)

    def close(self) -> None:
        self._connection.close()


@pytest.mark.parametrize(
    "test_case",
    (
        SQLitePersistenceCase(
            description="renamed lifecycle table persists facts",
            expected_event_count=1,
            expected_run_count=0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_real_sqlite_history_when_appending_then_renamed_table_persists_readable_fact(
    tmp_path: Path, test_case: SQLitePersistenceCase
) -> None:
    path: Path = tmp_path / "history.sqlite3"
    storage: SQLiteExecutionHistory = SQLiteExecutionHistory(path=path)

    stored: StoredEvent = storage.append_event(lifecycle_event("persisted"))
    page: EventPage = storage.get_events(event_filter=EventFilter())
    storage.close()

    inspection: sqlite3.Connection = sqlite3.connect(path)
    table_names: set[str] = {
        row[0]
        for row in inspection.execute(
            "SELECT name FROM sqlite_master WHERE type = 'table'"
        ).fetchall()
    }
    persisted_count: int = inspection.execute(
        "SELECT COUNT(*) FROM lifecycle_event_log"
    ).fetchone()[0]
    inspection.close()

    assert stored.event == page.records[0].event
    assert persisted_count == test_case.expected_event_count
    assert "lifecycle_event_log" in table_names
    assert "event_log" not in table_names
    assert test_case.expected_run_count == 0


@pytest.mark.parametrize(
    "test_case",
    (SQLitePathCase(description="project local default", expected_filename="history.sqlite3"),),
    ids=lambda case: case.description,
)
def test_given_project_directory_when_opening_sqlite_history_then_local_wal_is_healthy(
    tmp_path: Path, test_case: SQLitePathCase
) -> None:
    storage: SQLiteExecutionHistory = SQLiteExecutionHistory(project_dir=tmp_path)

    assert storage.check_health() is True
    history_path: Path | None = storage.path
    assert history_path == tmp_path / ".sqlbuild" / "history.sqlite3"
    assert history_path is not None
    assert history_path.name == test_case.expected_filename
    inspection: sqlite3.Connection = sqlite3.connect(history_path)
    journal_mode: str = inspection.execute("PRAGMA journal_mode").fetchone()[0]
    inspection.close()
    assert journal_mode == "wal"
    storage.close()


@pytest.mark.skipif(sys.platform == "win32", reason="Windows filenames cannot contain a colon")
@pytest.mark.parametrize(
    "test_case",
    (
        SQLitePathCase(
            description="literal file scheme prefix", expected_filename="file:history.sqlite3"
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_uri_shaped_filename_when_opening_history_then_exact_literal_file_is_used(
    tmp_path: Path, test_case: SQLitePathCase, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.chdir(tmp_path)
    assert test_case.expected_filename is not None
    path: Path = Path(test_case.expected_filename)

    with SQLiteExecutionHistory(path=path) as storage:
        assert storage.check_health() is True
        assert storage.path == path

    assert path.is_file()
    assert not Path("history.sqlite3").exists()


@pytest.mark.parametrize(
    "test_case",
    (SQLitePathCase(description="explicit memory database", expected_filename=None),),
    ids=lambda case: case.description,
)
def test_given_memory_path_when_opening_sqlite_history_then_no_file_is_created(
    test_case: SQLitePathCase,
) -> None:
    storage: SQLiteExecutionHistory = SQLiteExecutionHistory(path=":memory:")

    assert storage.check_health() is True
    assert storage.path == test_case.expected_filename
    storage.close()


@pytest.mark.parametrize(
    "test_case",
    (
        SQLitePersistenceCase(
            description="reopen preserves facts and reconciles projection",
            expected_event_count=2,
            expected_run_count=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_durable_events_without_projection_when_reopening_then_reconciliation_catches_up(
    tmp_path: Path, test_case: SQLitePersistenceCase
) -> None:
    path: Path = tmp_path / "history.sqlite3"
    first: SQLiteExecutionHistory = SQLiteExecutionHistory(path=path)
    _ = first.append_events((lifecycle_event("start"), lifecycle_event("end", "run_completed")))
    first.close()

    reopened: SQLiteExecutionHistory = SQLiteExecutionHistory(path=path)
    events: EventPage = reopened.get_events(event_filter=EventFilter())
    runs: RunPage = reopened.get_runs(run_filter=RunFilter())

    assert len(events.records) == test_case.expected_event_count
    assert len(runs.records) == test_case.expected_run_count
    assert runs.records[0].is_complete is True
    reopened.close()


@pytest.mark.parametrize(
    "test_case",
    (
        SQLiteMigrationCase(
            description="empty revision zero", initial_version=0, expected_version=1
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_supported_old_revision_when_opening_then_forward_migration_reaches_v1(
    tmp_path: Path, test_case: SQLiteMigrationCase
) -> None:
    path: Path = tmp_path / "history.sqlite3"
    connection: sqlite3.Connection = sqlite3.connect(path)
    connection.close()

    storage: SQLiteExecutionHistory = SQLiteExecutionHistory(path=path)

    assert test_case.initial_version == 0
    assert storage.get_schema_version() == test_case.expected_version
    storage.close()


@pytest.mark.parametrize(
    "test_case",
    (SQLiteMigrationCase(description="newer revision two", initial_version=2, expected_version=1),),
    ids=lambda case: case.description,
)
def test_given_newer_schema_revision_when_opening_then_history_is_rejected_without_recreation(
    tmp_path: Path, test_case: SQLiteMigrationCase
) -> None:
    path: Path = tmp_path / "history.sqlite3"
    connection: sqlite3.Connection = sqlite3.connect(path)
    _ = connection.execute(
        "CREATE TABLE execution_history_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL)"
    )
    _ = connection.execute(
        "INSERT INTO execution_history_metadata(key, value) VALUES (?, ?)",
        ("schema_version", str(test_case.initial_version)),
    )
    connection.commit()
    connection.close()

    with pytest.raises(UnsupportedSchemaVersionError, match=str(test_case.initial_version)):
        SQLiteExecutionHistory(path=path)

    inspection: sqlite3.Connection = sqlite3.connect(path)
    stored_version: str = inspection.execute(
        "SELECT value FROM execution_history_metadata WHERE key = 'schema_version'"
    ).fetchone()[0]
    inspection.close()
    assert stored_version == str(test_case.initial_version)
    assert test_case.expected_version == 1


@pytest.mark.parametrize(
    "test_case",
    (
        SQLitePersistenceCase(
            description="projection failure rolls back accepted event inserts",
            expected_event_count=0,
            expected_run_count=0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_conflicting_projection_when_appending_atomically_then_no_fact_or_run_commits(
    tmp_path: Path, test_case: SQLitePersistenceCase
) -> None:
    storage: SQLiteExecutionHistory = SQLiteExecutionHistory(path=tmp_path / "history.sqlite3")

    with pytest.raises(ProjectionConsistencyError):
        storage.append_and_project(
            (
                lifecycle_event("first", invocation_id="invocation-1"),
                lifecycle_event("second", invocation_id="invocation-2"),
            )
        )

    assert (
        len(storage.get_events(event_filter=EventFilter()).records)
        == test_case.expected_event_count
    )
    assert len(storage.get_runs(run_filter=RunFilter()).records) == test_case.expected_run_count
    storage.close()


@pytest.mark.parametrize(
    "test_case",
    (
        SQLiteTransactionFailureCase(
            description="commit failure rolls back and leaves history reusable",
            expected_event_count=1,
            expected_run_count=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_commit_failure_when_appending_atomically_then_next_append_succeeds(
    tmp_path: Path, test_case: SQLiteTransactionFailureCase
) -> None:
    storage: SQLiteExecutionHistory = SQLiteExecutionHistory(path=tmp_path / "history.sqlite3")
    storage._connection = cast(
        sqlite3.Connection,
        _TransactionFailureConnection(storage._connection, fail_commit_once=True),
    )

    with pytest.raises(ExecutionHistoryStorageError, match="atomic append and projection failed"):
        storage.append_and_project((lifecycle_event("failed-commit"),))

    _ = storage.append_and_project((lifecycle_event("successful-commit"),))
    events: EventPage = storage.get_events(event_filter=EventFilter())
    runs: RunPage = storage.get_runs(run_filter=RunFilter())
    storage.close()

    assert len(events.records) == test_case.expected_event_count
    assert cast(LifecycleEvent, events.records[0].event).event_id == "successful-commit"
    assert len(runs.records) == test_case.expected_run_count


@pytest.mark.parametrize(
    "test_case",
    (
        SQLiteTransactionFailureCase(
            description="body failure remains primary when rollback also fails",
            expected_event_count=1,
            expected_run_count=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_body_and_rollback_failures_when_transaction_exits_then_body_error_is_preserved(
    tmp_path: Path, test_case: SQLiteTransactionFailureCase
) -> None:
    storage: SQLiteExecutionHistory = SQLiteExecutionHistory(path=tmp_path / "history.sqlite3")
    storage._connection = cast(
        sqlite3.Connection,
        _TransactionFailureConnection(
            storage._connection, fail_rollback_once=True, fail_projection_once=True
        ),
    )

    with pytest.raises(ExecutionHistoryStorageError) as raised:
        storage.append_and_project((lifecycle_event("failed-body"),))
    assert str(raised.value.__cause__) == "controlled body failure"

    _ = storage.append_and_project((lifecycle_event("after-rollback-failure"),))
    events: EventPage = storage.get_events(event_filter=EventFilter())
    runs: RunPage = storage.get_runs(run_filter=RunFilter())
    storage.close()

    assert len(events.records) == test_case.expected_event_count
    assert len(runs.records) == test_case.expected_run_count


@pytest.mark.parametrize(
    "test_case",
    (
        SQLitePersistenceCase(
            description="writer follows locked reconciliation",
            expected_event_count=2,
            expected_run_count=2,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_reconcile_has_read_events_when_concurrent_writer_starts_then_projection_never_loses_writer(
    tmp_path: Path, test_case: SQLitePersistenceCase
) -> None:
    path: Path = tmp_path / "history.sqlite3"
    initial: SQLiteExecutionHistory = SQLiteExecutionHistory(path=path)
    _ = initial.append_and_project((lifecycle_event("run-1", run_id="run-1"),))
    initial.close()
    read_started: Event = Event()
    release_reconcile: Event = Event()

    with ThreadPoolExecutor(max_workers=2) as executor:
        reconcile_future: Future[tuple[RunRecord, ...]] = executor.submit(
            reconcile_with_coordination,
            path=path,
            read_started=read_started,
            release_reconcile=release_reconcile,
        )
        assert read_started.wait(timeout=5) is True
        writer_future: Future[tuple[StoredEvent, ...]] = executor.submit(
            append_atomically,
            path=path,
            events=(lifecycle_event("run-2", run_id="run-2"),),
        )
        with pytest.raises(TimeoutError):
            writer_future.result(timeout=0.1)
        release_reconcile.set()
        _ = reconcile_future.result(timeout=5)
        _ = writer_future.result(timeout=5)

    reopened: SQLiteExecutionHistory = SQLiteExecutionHistory(path=path)
    assert (
        len(reopened.get_events(event_filter=EventFilter()).records)
        == test_case.expected_event_count
    )
    assert len(reopened.get_runs(run_filter=RunFilter()).records) == test_case.expected_run_count
    reopened.close()


@pytest.mark.parametrize(
    "test_case",
    (
        SQLitePersistenceCase(
            description="adjacent microseconds filter inclusively",
            expected_event_count=1,
            expected_run_count=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_adjacent_subsecond_events_when_filtering_exact_boundary_then_only_boundary_event_matches(
    tmp_path: Path, test_case: SQLitePersistenceCase
) -> None:
    boundary: datetime = datetime(2026, 1, 1, 0, 0, 0, 100_001, tzinfo=UTC)
    storage: SQLiteExecutionHistory = SQLiteExecutionHistory(path=tmp_path / "history.sqlite3")
    _ = storage.append_events(
        (
            lifecycle_event(
                "before", occurred_at=datetime(2026, 1, 1, 0, 0, 0, 100_000, tzinfo=UTC)
            ),
            lifecycle_event("boundary", occurred_at=boundary),
        )
    )

    page: EventPage = storage.get_events(
        event_filter=EventFilter(occurred_at_start=boundary, occurred_at_end=boundary)
    )

    assert len(page.records) == test_case.expected_event_count
    assert page.records[0].event == lifecycle_event("boundary", occurred_at=boundary)
    assert page.records[0].received_at.tzinfo is UTC
    storage.close()


@pytest.mark.parametrize(
    "test_case",
    (
        SQLitePersistenceCase(
            description="adjacent run creation microseconds filter inclusively",
            expected_event_count=2,
            expected_run_count=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_adjacent_subsecond_run_creation_when_filtering_exact_boundary_then_only_boundary_run_matches(
    tmp_path: Path, test_case: SQLitePersistenceCase
) -> None:
    before: datetime = datetime(2026, 1, 1, 0, 0, 0, 200_000, tzinfo=UTC)
    boundary: datetime = datetime(2026, 1, 1, 0, 0, 0, 200_001, tzinfo=UTC)
    storage: SQLiteExecutionHistory = SQLiteExecutionHistory(path=tmp_path / "history.sqlite3")
    projected: tuple[RunRecord, ...] = storage.project(
        (
            StoredEvent(
                storage_order=1,
                cursor="event-1",
                received_at=before,
                event=lifecycle_event("run-before", run_id="run-before"),
            ),
            StoredEvent(
                storage_order=2,
                cursor="event-2",
                received_at=boundary,
                event=lifecycle_event("run-boundary", run_id="run-boundary"),
            ),
        )
    )

    page: RunPage = storage.get_runs(
        run_filter=RunFilter(created_at_start=boundary, created_at_end=boundary)
    )

    assert len(projected) == test_case.expected_event_count
    assert len(page.records) == test_case.expected_run_count
    assert page.records[0].run_id == "run-boundary"
    assert page.records[0].created_at.tzinfo is UTC
    storage.close()


@pytest.mark.parametrize(
    "test_case",
    (
        SQLitePersistenceCase(
            description="event and run cursors survive reopen",
            expected_event_count=1,
            expected_run_count=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_persisted_cursors_when_reopening_same_and_foreign_databases_then_affinity_is_enforced(
    tmp_path: Path, test_case: SQLitePersistenceCase
) -> None:
    path: Path = tmp_path / "history.sqlite3"
    first: SQLiteExecutionHistory = SQLiteExecutionHistory(path=path)
    _ = first.append_and_project(
        (
            lifecycle_event("first", run_id="run-1"),
            lifecycle_event("second", run_id="run-2"),
        )
    )
    event_cursor: str | None = first.get_events(event_filter=EventFilter(), limit=1).next_cursor
    run_cursor: str | None = first.get_runs(run_filter=RunFilter(), limit=1).next_cursor
    first.close()

    reopened: SQLiteExecutionHistory = SQLiteExecutionHistory(path=path)
    event_page: EventPage = reopened.get_events(
        event_filter=EventFilter(), after_cursor=event_cursor, limit=1
    )
    run_page: RunPage = reopened.get_runs(run_filter=RunFilter(), after_cursor=run_cursor, limit=1)
    foreign: SQLiteExecutionHistory = SQLiteExecutionHistory(path=tmp_path / "foreign.sqlite3")
    _ = foreign.append_and_project((lifecycle_event("foreign", run_id="foreign-run"),))

    assert len(event_page.records) == test_case.expected_event_count
    assert len(run_page.records) == test_case.expected_run_count
    with pytest.raises(InvalidCursorError):
        foreign.get_events(event_filter=EventFilter(), after_cursor=event_cursor)
    with pytest.raises(InvalidCursorError):
        foreign.get_runs(run_filter=RunFilter(), after_cursor=run_cursor)
    reopened.close()
    foreign.close()


@pytest.mark.parametrize(
    "test_case",
    (
        SQLitePersistenceCase(
            description="opaque nested fields roundtrip and retry",
            expected_event_count=1,
            expected_run_count=0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_opaque_nested_extra_fields_when_reopening_and_retrying_then_canonical_content_is_exact(
    tmp_path: Path, test_case: SQLitePersistenceCase
) -> None:
    path: Path = tmp_path / "history.sqlite3"
    event: OpaqueLifecycleEvent = OpaqueLifecycleEvent(
        raw={
            "event_id": "opaque-nested",
            "schema_version": 3,
            "event_type": "future_event",
            "occurred_at": "2026-01-01T00:00:00.123456Z",
            "nested": {"items": [1, {"extra": True}]},
            "future_extra": "preserve-me",
        }
    )
    first: SQLiteExecutionHistory = SQLiteExecutionHistory(path=path)
    original: StoredEvent = first.append_event(event)
    first.close()

    reopened: SQLiteExecutionHistory = SQLiteExecutionHistory(path=path)
    page: EventPage = reopened.get_events(event_filter=EventFilter())
    retried: StoredEvent = reopened.append_event(event)

    assert len(page.records) == test_case.expected_event_count
    assert len(reopened.get_runs(run_filter=RunFilter()).records) == test_case.expected_run_count
    assert canonical_event_content(page.records[0].event) == canonical_event_content(event)
    assert retried == original
    reopened.close()


@pytest.mark.parametrize(
    "test_case",
    (
        SQLiteTimeoutCase(description="zero timeout", timeout=0, expected_error="positive integer"),
        SQLiteTimeoutCase(
            description="negative timeout", timeout=-1, expected_error="positive integer"
        ),
        SQLiteTimeoutCase(
            description="boolean timeout", timeout=True, expected_error="positive integer"
        ),
        SQLiteTimeoutCase(
            description="float timeout", timeout=1.5, expected_error="positive integer"
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_busy_timeout_when_constructing_then_typed_error_is_raised(
    tmp_path: Path, test_case: SQLiteTimeoutCase
) -> None:
    with pytest.raises(ExecutionHistoryStorageError, match=test_case.expected_error):
        SQLiteExecutionHistory(
            path=tmp_path / "history.sqlite3",
            busy_timeout_ms=test_case.timeout,  # ty: ignore[invalid-argument-type]
        )


@pytest.mark.parametrize(
    "test_case",
    (
        SQLitePersistenceCase(
            description="post-connect setup failure closes connection",
            expected_event_count=0,
            expected_run_count=0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_post_connect_setup_failure_when_constructing_then_open_connection_is_closed(
    tmp_path: Path,
    test_case: SQLitePersistenceCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    opened: list[Any] = []

    def reject_migration(storage: SQLiteExecutionHistory) -> None:
        opened.append(storage._connection)
        raise OSError("controlled post-connect setup failure")

    monkeypatch.setattr(SQLiteExecutionHistory, "_migrate", reject_migration)

    with pytest.raises(ExecutionHistoryStorageError, match="cannot open SQLite history"):
        SQLiteExecutionHistory(path=tmp_path / "history.sqlite3")

    with pytest.raises(sqlite3.ProgrammingError, match="closed"):
        opened[0].execute("SELECT 1")
    assert len(opened) == test_case.expected_event_count + 1
    assert test_case.expected_run_count == 0


@pytest.mark.parametrize(
    "test_case",
    (
        SQLitePersistenceCase(
            description="native driver persists and reopens without Python connect",
            expected_event_count=1,
            expected_run_count=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_python_driver_unavailable_when_persisting_then_native_history_reopens(
    tmp_path: Path,
    test_case: SQLitePersistenceCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    path: Path = tmp_path / "history.sqlite3"
    event: LifecycleEvent = lifecycle_event("native-driver")
    monkeypatch.setattr(
        sqlite3, "connect", Mock(side_effect=RuntimeError("Python driver unavailable"))
    )

    with SQLiteExecutionHistory(path=path) as storage:
        stored: StoredEvent = storage.append_event(event)
        storage.project((stored,))

    with SQLiteExecutionHistory(path=path) as reopened:
        page: EventPage = reopened.get_events(event_filter=EventFilter())
        runs: RunPage = reopened.get_runs(run_filter=RunFilter())
        assert reopened.check_health() is True

    assert len(page.records) == test_case.expected_event_count
    assert page.records[0].event == stored.event
    assert len(runs.records) == test_case.expected_run_count


@pytest.mark.skipif(sys.platform == "win32", reason="subprocess SIGINT delivery requires POSIX")
@pytest.mark.parametrize(
    "test_case",
    (
        SQLiteInterruptCase(
            description="interrupt remains observable while another thread owns history",
            expected_stdout="interrupted before owner release; storage closed cleanly\n",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_contended_history_when_interrupted_then_interrupt_precedes_owner_release(
    test_case: SQLiteInterruptCase,
) -> None:
    script: str = dedent(
        """
        import os
        import signal
        import threading

        from sqlbuild.execution_history import EventFilter
        from sqlbuild.sqlite_history import SQLiteExecutionHistory

        signal.signal(signal.SIGINT, signal.default_int_handler)

        class BlockingHistory(SQLiteExecutionHistory):
            def __init__(self):
                self.block = False
                self.entered = threading.Event()
                self.release = threading.Event()
                super().__init__(path=":memory:")

            def _read_all_events(self):
                rows = super()._read_all_events()
                if self.block:
                    self.entered.set()
                    assert self.release.wait(10)
                return rows

        history = BlockingHistory()
        history.block = True
        owner = threading.Thread(target=history.reconcile)
        owner.start()
        assert history.entered.wait(5)
        interrupt = threading.Timer(0.25, lambda: os.kill(os.getpid(), signal.SIGINT))
        fallback = threading.Timer(3, history.release.set)
        interrupt.start()
        fallback.start()
        try:
            try:
                history.get_events(event_filter=EventFilter())
            except KeyboardInterrupt:
                assert not history.release.is_set(), "interrupt waited for the owner"
            else:
                raise AssertionError("expected KeyboardInterrupt")
        finally:
            history.release.set()
            interrupt.cancel()
            fallback.cancel()
            owner.join(5)
            assert not owner.is_alive()
            history.close()
        print("interrupted before owner release; storage closed cleanly")
        """
    )
    completed: subprocess.CompletedProcess[str] = subprocess.run(
        [sys.executable, "-c", script],
        capture_output=True,
        text=True,
        timeout=15,
        check=False,
    )

    assert (completed.returncode, completed.stdout) == (
        0,
        test_case.expected_stdout,
    ), completed.stderr
