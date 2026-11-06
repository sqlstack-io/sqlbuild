"""Test-only in-memory execution history backend and fixture builders."""

from __future__ import annotations

import base64
import binascii
import json
import os
import tempfile
import uuid
from collections.abc import Callable, Iterable, Iterator
from datetime import UTC, datetime, timedelta
from pathlib import Path
from types import TracebackType
from typing import Any, Self, cast

import pytest

from sqlbuild._native import (
    CURRENT_EVENT_LOG_SCHEMA_VERSION,
    CURRENT_RUN_STORAGE_SCHEMA_VERSION,
    DEFAULT_PAGE_LIMIT,
)
from sqlbuild.execution_history import (
    CanonicalLifecycleEvent,
    EventFilter,
    EventPage,
    ExecutionHistoryStorageError,
    IntegrityConflictError,
    InvalidCursorError,
    LifecycleEventLogStorage,
    RunFilter,
    RunPage,
    RunRecord,
    RunStorage,
    StoredEvent,
    UnsupportedSchemaVersionError,
    canonical_event_content,
    canonical_event_id,
    project_runs,
    validate_page_limit,
)
from sqlbuild.runtime.observability.models import LifecycleEvent, OpaqueLifecycleEvent
from sqlbuild.runtime.observability.types import JSONValue
from sqlbuild.sqlite_history import SQLiteExecutionHistory
from tests.unit.src.sqlbuild.runtime.execution_history.conformance._test_types import BackendCase

BASE_TIME: datetime = datetime(2026, 1, 1, tzinfo=UTC)
RUN_CURSOR_PREFIX: str = "run:"


class InMemoryLifecycleEventLogStorage:
    """Small test-only implementation used to execute the backend contract."""

    def __init__(self) -> None:
        self._records: list[StoredEvent] = []
        self._by_id: dict[str, StoredEvent] = {}
        self._cursor_namespace = uuid.uuid4().hex
        self._closed = False
        self.fail_append = False

    def append_event(self, event: CanonicalLifecycleEvent) -> StoredEvent:
        self._ensure_open()
        return self.append_events((event,))[0]

    def append_events(self, events: Iterable[CanonicalLifecycleEvent]) -> tuple[StoredEvent, ...]:
        self._ensure_open()
        if self.fail_append:
            raise ExecutionHistoryStorageError("injected append failure")
        pending: tuple[CanonicalLifecycleEvent, ...] = tuple(events)
        self._validate_batch(events=pending)
        for event in pending:
            self._validate_duplicate(event=event)
        stored: list[StoredEvent] = []
        for event in pending:
            event_id: str = canonical_event_id(event)
            existing: StoredEvent | None = self._by_id.get(event_id)
            if existing is not None:
                stored.append(existing)
                continue
            storage_order: int = len(self._records) + 1
            record: StoredEvent = StoredEvent(
                storage_order=storage_order,
                cursor=f"event:{self._cursor_namespace}:{storage_order}",
                received_at=BASE_TIME + timedelta(microseconds=storage_order),
                event=event,
            )
            self._records.append(record)
            self._by_id[event_id] = record
            stored.append(record)
        return tuple(stored)

    def get_events(
        self,
        event_filter: EventFilter,
        *,
        after_cursor: str | None = None,
        limit: int = DEFAULT_PAGE_LIMIT,
    ) -> EventPage:
        self._ensure_open()
        validate_page_limit(limit)
        start: int = self._event_start(after_cursor=after_cursor)
        matching: tuple[StoredEvent, ...] = tuple(
            record
            for record in self._records[start:]
            if event_matches(record=record, event_filter=event_filter)
        )
        records: tuple[StoredEvent, ...] = matching[:limit]
        next_cursor: str | None = records[-1].cursor if records else None
        return EventPage(records=records, next_cursor=next_cursor, has_more=len(matching) > limit)

    def get_schema_version(self) -> int:
        self._ensure_open()
        return CURRENT_EVENT_LOG_SCHEMA_VERSION

    def upgrade_schema(self, *, target_version: int | None = None) -> int:
        self._ensure_open()
        target: int = CURRENT_EVENT_LOG_SCHEMA_VERSION if target_version is None else target_version
        if target != CURRENT_EVENT_LOG_SCHEMA_VERSION:
            raise UnsupportedSchemaVersionError(f"unsupported event log schema version {target}")
        return target

    def close(self) -> None:
        self._closed = True

    def dispose(self) -> None:
        self.close()

    def __enter__(self) -> Self:
        self._ensure_open()
        return self

    def __exit__(
        self,
        exception_type: type[BaseException] | None,
        exception: BaseException | None,
        traceback: TracebackType | None,
    ) -> bool | None:
        self.close()
        return None

    def _validate_duplicate(self, *, event: CanonicalLifecycleEvent) -> None:
        event_id: str = canonical_event_id(event)
        existing: StoredEvent | None = self._by_id.get(event_id)
        if existing is not None and canonical_event_content(
            existing.event
        ) != canonical_event_content(event):
            raise IntegrityConflictError(f"event_id {event_id!r} has different canonical content")

    def _validate_batch(self, *, events: tuple[CanonicalLifecycleEvent, ...]) -> None:
        batch_content: dict[str, str] = {}
        for event in events:
            event_id: str = canonical_event_id(event)
            canonical_content: str = canonical_event_content(event)
            previous_content: str | None = batch_content.get(event_id)
            if previous_content is not None and previous_content != canonical_content:
                raise IntegrityConflictError(
                    f"event_id {event_id!r} has different canonical content within batch"
                )
            batch_content[event_id] = canonical_content

    def _event_start(self, *, after_cursor: str | None) -> int:
        if after_cursor is None:
            return 0
        cursor_positions: dict[str, int] = {
            record.cursor: index + 1 for index, record in enumerate(self._records)
        }
        try:
            return cursor_positions[after_cursor]
        except KeyError as error:
            raise InvalidCursorError("event cursor is not valid for this storage") from error

    def _ensure_open(self) -> None:
        if self._closed:
            raise ExecutionHistoryStorageError("event log storage is closed")


class InMemoryRunStorage:
    """Small test-only run projection implementation for conformance execution."""

    def __init__(self) -> None:
        self._runs: tuple[RunRecord, ...] = ()
        self.project_calls = 0
        self.fail_project = False
        self.fail_after_compute_at_attempt: int | None = None
        self._publication_attempts = 0
        self._closed = False

    def get_run(self, run_id: str) -> RunRecord | None:
        self._ensure_open()
        return next((run for run in self._runs if run.run_id == run_id), None)

    def get_runs(
        self,
        run_filter: RunFilter,
        *,
        after_cursor: str | None = None,
        limit: int = DEFAULT_PAGE_LIMIT,
    ) -> RunPage:
        self._ensure_open()
        validate_page_limit(limit)
        after_key: tuple[datetime, str] | None = run_cursor_key(
            runs=self._runs, after_cursor=after_cursor
        )
        matching: tuple[RunRecord, ...] = tuple(
            run
            for run in self._runs
            if run_matches(run=run, run_filter=run_filter)
            and (after_key is None or run_sort_key(run=run) > after_key)
        )
        records: tuple[RunRecord, ...] = matching[:limit]
        next_cursor: str | None = run_cursor(run=records[-1]) if records else None
        return RunPage(records=records, next_cursor=next_cursor, has_more=len(matching) > limit)

    def project(self, stored_events: Iterable[StoredEvent]) -> tuple[RunRecord, ...]:
        self._ensure_open()
        self.project_calls += 1
        if self.fail_project:
            raise ExecutionHistoryStorageError("injected projection failure")
        computed: tuple[RunRecord, ...] = project_runs(
            stored_events=stored_events, current_runs=self._runs
        )
        self._raise_at_injected_publication_attempt()
        self._runs = computed
        return self._runs

    def rebuild_from_events(self, stored_events: Iterable[StoredEvent]) -> tuple[RunRecord, ...]:
        self._ensure_open()
        computed: tuple[RunRecord, ...] = project_runs(stored_events=stored_events)
        self._raise_at_injected_publication_attempt()
        self._runs = computed
        return self._runs

    def get_schema_version(self) -> int:
        self._ensure_open()
        return CURRENT_RUN_STORAGE_SCHEMA_VERSION

    def upgrade_schema(self, *, target_version: int | None = None) -> int:
        self._ensure_open()
        target: int = (
            CURRENT_RUN_STORAGE_SCHEMA_VERSION if target_version is None else target_version
        )
        if target != CURRENT_RUN_STORAGE_SCHEMA_VERSION:
            raise UnsupportedSchemaVersionError(f"unsupported run schema version {target}")
        return target

    def close(self) -> None:
        self._closed = True

    def dispose(self) -> None:
        self.close()

    def __enter__(self) -> Self:
        self._ensure_open()
        return self

    def __exit__(
        self,
        exception_type: type[BaseException] | None,
        exception: BaseException | None,
        traceback: TracebackType | None,
    ) -> bool | None:
        self.close()
        return None

    def _ensure_open(self) -> None:
        if self._closed:
            raise ExecutionHistoryStorageError("run storage is closed")

    def _raise_at_injected_publication_attempt(self) -> None:
        self._publication_attempts += 1
        if self._publication_attempts == self.fail_after_compute_at_attempt:
            raise ExecutionHistoryStorageError("injected atomic projection publication failure")


class FailingSQLiteEventLog(SQLiteExecutionHistory):
    """SQLite event storage with deterministic append failure."""

    def append_events(self, events: Iterable[CanonicalLifecycleEvent]) -> tuple[StoredEvent, ...]:
        raise ExecutionHistoryStorageError("injected append failure")


class FailingSQLiteRunStorage(SQLiteExecutionHistory):
    """SQLite run storage with deterministic projection failure."""

    def project(self, stored_events: Iterable[StoredEvent]) -> tuple[RunRecord, ...]:
        raise ExecutionHistoryStorageError("injected projection failure")


class AtomicFailingSQLiteRunStorage(SQLiteExecutionHistory):
    """SQLite run storage with one post-computation publication failure."""

    def __init__(self, *, path: str) -> None:
        self._publication_attempts = 0
        super().__init__(path=path)
        self._publication_attempts = 0

    def _publish_projection(self, projected: tuple[RunRecord, ...]) -> None:
        self._publication_attempts += 1
        if self._publication_attempts == 2:
            raise ExecutionHistoryStorageError("injected atomic projection publication failure")
        super()._publish_projection(projected)


def postgres_dsn() -> str:
    """Create an isolated PostgreSQL schema and return its DSN."""

    import psycopg
    from psycopg.conninfo import make_conninfo
    from psycopg.sql import SQL, Identifier

    dsn: str = os.environ["SQLBUILD_TEST_POSTGRES_DSN"]
    schema: str = f"sqlbuild_history_{uuid.uuid4().hex}"
    with psycopg.connect(dsn, autocommit=True) as connection:
        connection.execute(SQL("CREATE SCHEMA {}").format(Identifier(schema)))
    return make_conninfo(dsn, options=f"-c search_path={schema}")


def postgres_factory() -> LifecycleEventLogStorage:
    """Build an isolated PostgreSQL backend when conformance is explicitly DSN-gated."""

    from sqlbuild.postgres_history import PostgresExecutionHistory

    return PostgresExecutionHistory(postgres_dsn())


class FailingPostgresEventLog:
    """Test-only PostgreSQL append failure decorator."""

    def __new__(cls) -> LifecycleEventLogStorage:
        storage: LifecycleEventLogStorage = postgres_factory()

        def fail(events: Iterable[CanonicalLifecycleEvent]) -> tuple[StoredEvent, ...]:
            raise ExecutionHistoryStorageError("injected append failure")

        storage.append_events = fail  # ty: ignore[invalid-assignment]
        return storage


class FailingPostgresRunStorage:
    """Test-only PostgreSQL projection failure decorator."""

    def __new__(cls) -> RunStorage:
        storage: RunStorage = cast(RunStorage, postgres_factory())

        def fail(events: Iterable[StoredEvent]) -> tuple[RunRecord, ...]:
            raise ExecutionHistoryStorageError("injected projection failure")

        storage.project = fail  # ty: ignore[invalid-assignment]
        return storage


class AtomicFailingPostgresRunStorage:
    """Test-only PostgreSQL publication failure decorator."""

    def __new__(cls) -> RunStorage:
        from sqlbuild.postgres_history import PostgresExecutionHistory

        class InjectedPublicationFailure(PostgresExecutionHistory):
            def __init__(self, dsn: str) -> None:
                self._publication_attempts = 0
                super().__init__(dsn)
                self._publication_attempts = 0

            def _replace_projection(self, *, cursor: Any, projected: tuple[RunRecord, ...]) -> None:
                self._publication_attempts += 1
                if self._publication_attempts == 2:
                    raise ExecutionHistoryStorageError(
                        "injected atomic projection publication failure"
                    )
                super()._replace_projection(cursor=cursor, projected=projected)

        return InjectedPublicationFailure(postgres_dsn())


def lifecycle_event(
    event_id: str,
    event_type: str = "run_started",
    *,
    invocation_id: str = "invocation-1",
    run_id: str | None = "run-1",
    producer: str = "sqlbuild",
    occurred_at: datetime = BASE_TIME,
) -> LifecycleEvent:
    """Build a valid canonical lifecycle event for contract tests."""

    payload: dict[str, JSONValue] = {}
    if event_type == "invocation_started":
        payload = {"command": "build"}
        run_id = None
    return LifecycleEvent(
        event_id=event_id,
        event_type=event_type,
        schema_version=1,
        producer=producer,
        producer_version="0.72.2",
        occurred_at=occurred_at,
        invocation_id=invocation_id,
        run_id=run_id,
        payload=payload,
    )


def event_matches(*, record: StoredEvent, event_filter: EventFilter) -> bool:
    """Apply backend-neutral event filter semantics for the test backend."""

    event: CanonicalLifecycleEvent = record.event
    invocation_id: str | None = event_text(event=event, field_name="invocation_id")
    run_id: str | None = event_text(event=event, field_name="run_id")
    event_type: str | None = event_text(event=event, field_name="event_type")
    producer: str | None = event_text(event=event, field_name="producer")
    occurred_at: datetime | None = event_occurred_at(event=event)
    checks: tuple[bool, ...] = (
        event_filter.invocation_id is None or invocation_id == event_filter.invocation_id,
        event_filter.run_id is None or run_id == event_filter.run_id,
        not event_filter.event_types or event_type in event_filter.event_types,
        event_filter.family is None
        or (event_type is not None and event_type.startswith(f"{event_filter.family.value}_")),
        event_filter.producer is None or producer == event_filter.producer,
        event_filter.occurred_at_start is None
        or (occurred_at is not None and occurred_at >= event_filter.occurred_at_start),
        event_filter.occurred_at_end is None
        or (occurred_at is not None and occurred_at <= event_filter.occurred_at_end),
    )
    return all(checks)


def event_text(*, event: CanonicalLifecycleEvent, field_name: str) -> str | None:
    """Read a correctly typed stable text envelope field."""

    value: object = (
        getattr(event, field_name)
        if isinstance(event, LifecycleEvent)
        else event.raw.get(field_name)
    )
    return value if isinstance(value, str) and value else None


def event_occurred_at(*, event: CanonicalLifecycleEvent) -> datetime | None:
    """Read a correctly typed UTC occurred-at envelope field."""

    if isinstance(event, LifecycleEvent):
        return event.occurred_at
    raw_value: object = event.raw.get("occurred_at")
    if not isinstance(raw_value, str):
        return None
    try:
        value: datetime = datetime.fromisoformat(raw_value.replace("Z", "+00:00"))
    except ValueError:
        return None
    return value if value.tzinfo is UTC else None


def run_matches(*, run: RunRecord, run_filter: RunFilter) -> bool:
    """Apply backend-neutral run filter semantics for the test backend."""

    checks: tuple[bool, ...] = (
        run_filter.invocation_id is None or run.invocation_id == run_filter.invocation_id,
        not run_filter.statuses or run.status in run_filter.statuses,
        run_filter.created_at_start is None or run.created_at >= run_filter.created_at_start,
        run_filter.created_at_end is None or run.created_at <= run_filter.created_at_end,
    )
    return all(checks)


def run_cursor(*, run: RunRecord) -> str:
    """Encode the test backend's implementation-owned run cursor."""

    payload: str = json.dumps(
        [run.created_at.isoformat(), run.run_id], ensure_ascii=True, separators=(",", ":")
    )
    encoded: str = base64.urlsafe_b64encode(payload.encode()).decode()
    return f"{RUN_CURSOR_PREFIX}{encoded}"


def run_sort_key(*, run: RunRecord) -> tuple[datetime, str]:
    """Return the backend-neutral deterministic run ordering key."""

    return run.created_at, run.run_id


def run_cursor_key(
    *, runs: tuple[RunRecord, ...], after_cursor: str | None
) -> tuple[datetime, str] | None:
    """Decode and validate an implementation-owned global run cursor."""

    if after_cursor is None:
        return None
    try:
        if not after_cursor.startswith(RUN_CURSOR_PREFIX):
            raise ValueError
        encoded: str = after_cursor.removeprefix(RUN_CURSOR_PREFIX)
        decoded: object = json.loads(base64.urlsafe_b64decode(encoded).decode())
        if not isinstance(decoded, list) or len(decoded) != 2:
            raise ValueError
        created_raw, run_id = decoded
        if not isinstance(created_raw, str) or not isinstance(run_id, str) or not run_id:
            raise ValueError
        created_at: datetime = datetime.fromisoformat(created_raw)
        if created_at.tzinfo is not UTC:
            raise ValueError
        key: tuple[datetime, str] = created_at, run_id
    except (ValueError, TypeError, UnicodeError, json.JSONDecodeError, binascii.Error) as error:
        raise InvalidCursorError("run cursor is malformed or belongs to another backend") from error
    known_keys: frozenset[tuple[datetime, str]] = frozenset(run_sort_key(run=run) for run in runs)
    if key not in known_keys:
        raise InvalidCursorError("run cursor does not identify a run in this storage")
    return key


def opaque_event(event_id: str) -> OpaqueLifecycleEvent:
    """Build an opaque canonical lifecycle envelope for storage tests."""

    return OpaqueLifecycleEvent(raw={"event_id": event_id, "schema_version": 3})


def append_failing_event_log_factory() -> LifecycleEventLogStorage:
    """Build an event log with append failure injection."""

    storage: InMemoryLifecycleEventLogStorage = InMemoryLifecycleEventLogStorage()
    storage.fail_append = True
    return storage


def project_failing_run_storage_factory() -> RunStorage:
    """Build run storage with pre-computation projection failure injection."""

    storage: InMemoryRunStorage = InMemoryRunStorage()
    storage.fail_project = True
    return storage


def atomic_failing_run_storage_factory() -> RunStorage:
    """Build run storage with one post-computation publication failure."""

    storage: InMemoryRunStorage = InMemoryRunStorage()
    storage.fail_after_compute_at_attempt = 2
    return storage


def sqlite_path() -> str:
    """Return an isolated persistent SQLite test path."""

    return str(Path(tempfile.mkdtemp()) / "history.sqlite3")


def sqlite_factory() -> SQLiteExecutionHistory:
    """Build an isolated SQLite execution history backend."""

    return SQLiteExecutionHistory(path=sqlite_path())


def sqlite_append_failing_factory() -> LifecycleEventLogStorage:
    """Build SQLite event storage with append failure injection."""

    return FailingSQLiteEventLog(path=sqlite_path())


def sqlite_project_failing_factory() -> RunStorage:
    """Build SQLite run storage with projection failure injection."""

    return FailingSQLiteRunStorage(path=sqlite_path())


def sqlite_atomic_failing_factory() -> RunStorage:
    """Build SQLite run storage with atomic publication failure injection."""

    return AtomicFailingSQLiteRunStorage(path=sqlite_path())


BACKEND_CASES: tuple[BackendCase, ...] = (
    BackendCase(
        description="test-only in-memory backend",
        event_log_factory=InMemoryLifecycleEventLogStorage,
        run_storage_factory=InMemoryRunStorage,
        append_failing_event_log_factory=append_failing_event_log_factory,
        project_failing_run_storage_factory=project_failing_run_storage_factory,
        atomic_failing_run_storage_factory=atomic_failing_run_storage_factory,
        project_call_count=lambda storage: cast(InMemoryRunStorage, storage).project_calls,
        expected_backend="memory",
    ),
    BackendCase(
        description="native SQLite backend",
        event_log_factory=sqlite_factory,
        run_storage_factory=sqlite_factory,
        append_failing_event_log_factory=sqlite_append_failing_factory,
        project_failing_run_storage_factory=sqlite_project_failing_factory,
        atomic_failing_run_storage_factory=sqlite_atomic_failing_factory,
        project_call_count=lambda storage: cast(SQLiteExecutionHistory, storage).project_calls,
        expected_backend="sqlite",
    ),
)

if os.environ.get("SQLBUILD_TEST_POSTGRES_DSN"):
    BACKEND_CASES += (
        BackendCase(
            description="deployed PostgreSQL backend",
            event_log_factory=postgres_factory,
            run_storage_factory=lambda: cast(RunStorage, postgres_factory()),
            append_failing_event_log_factory=FailingPostgresEventLog,
            project_failing_run_storage_factory=FailingPostgresRunStorage,
            atomic_failing_run_storage_factory=AtomicFailingPostgresRunStorage,
            project_call_count=lambda storage: cast(Any, storage).project_calls,
            expected_backend="postgres",
        ),
    )


@pytest.fixture(params=BACKEND_CASES, ids=lambda case: case.description)
def backend_case(request: pytest.FixtureRequest) -> BackendCase:
    """Provide one registered backend to every shared conformance test."""

    return cast(BackendCase, request.param)


@pytest.fixture
def event_log_factory(backend_case: BackendCase) -> Callable[[], LifecycleEventLogStorage]:
    """Provide the registered event log factory."""

    return backend_case.event_log_factory


@pytest.fixture
def run_storage_factory(backend_case: BackendCase) -> Callable[[], RunStorage]:
    """Provide the registered run storage factory."""

    return backend_case.run_storage_factory


@pytest.fixture
def event_log(
    event_log_factory: Callable[[], LifecycleEventLogStorage],
) -> Iterator[LifecycleEventLogStorage]:
    """Yield an isolated event log contract implementation."""

    storage: LifecycleEventLogStorage = event_log_factory()
    with storage:
        yield storage


@pytest.fixture
def run_storage(run_storage_factory: Callable[[], RunStorage]) -> Iterator[RunStorage]:
    """Yield an isolated run projection contract implementation."""

    storage: RunStorage = run_storage_factory()
    yield storage
    storage.close()


@pytest.fixture
def append_failing_event_log(backend_case: BackendCase) -> LifecycleEventLogStorage:
    """Provide an event log with deterministic append failure injection."""

    return backend_case.append_failing_event_log_factory()


@pytest.fixture
def project_failing_run_storage(backend_case: BackendCase) -> RunStorage:
    """Provide run storage with deterministic projection failure injection."""

    return backend_case.project_failing_run_storage_factory()


@pytest.fixture
def atomic_failing_run_storage(backend_case: BackendCase) -> RunStorage:
    """Provide storage that fails one atomic projection publication."""

    return backend_case.atomic_failing_run_storage_factory()


@pytest.fixture
def tracking_run_storage(backend_case: BackendCase) -> tuple[RunStorage, Callable[[], int]]:
    """Provide run storage and an accessor for its projection call count."""

    storage: RunStorage = backend_case.run_storage_factory()
    return storage, lambda: backend_case.project_call_count(storage)
