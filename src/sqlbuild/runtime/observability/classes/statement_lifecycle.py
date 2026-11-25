"""Canonical lifecycle boundary for one logical SQL statement."""

from __future__ import annotations

import asyncio
import hashlib
import re
import threading
import time
from collections.abc import Callable, Iterator, Mapping
from contextlib import ExitStack, contextmanager
from contextvars import ContextVar, Token
from types import TracebackType

from sqlbuild.runtime.execution_limits.classes.item_deadline import ItemDeadline
from sqlbuild.runtime.execution_limits.main.current_item_deadline import current_item_deadline
from sqlbuild.runtime.observability._helpers.dispatcher import (
    current_event_dispatcher,
    dispatcher_scope,
)
from sqlbuild.runtime.observability._helpers.factory import create_lifecycle_event
from sqlbuild.runtime.observability._helpers.identity import (
    current_execution_identity,
    invocation_scope,
    statement_scope,
)
from sqlbuild.runtime.observability._helpers.statement_listeners import (
    notify_statement_finished,
    statement_listener_scope,
)
from sqlbuild.runtime.observability.classes.event_dispatcher import EventDispatcher
from sqlbuild.runtime.observability.classes.statement_monitor import (
    StatementMonitor,
)
from sqlbuild.runtime.observability.constants import (
    LOCAL_RUN_ON,
    STATEMENT_HEARTBEAT_THRESHOLD_SECONDS,
)
from sqlbuild.runtime.observability.exceptions import ObservabilityValidationError
from sqlbuild.runtime.observability.models import ExecutionIdentity
from sqlbuild.runtime.observability.types import JSONValue, StatementListener

_MAX_ERROR_VALUE_LENGTH: int = 128
_STATEMENT_KIND_PATTERN: re.Pattern[str] = re.compile(r"^([A-Za-z]{1,64})(?![A-Za-z0-9_])")
_ERROR_CODE_PATTERN: re.Pattern[str] = re.compile(r"^[A-Za-z0-9._:-]{1,128}$")
_STATEMENT_LIFECYCLE_OWNER: ContextVar[StatementLifecycle | None] = ContextVar(
    "sqlbuild_statement_lifecycle_owner", default=None
)
_STATEMENT_PURPOSE: ContextVar[str | None] = ContextVar("sqlbuild_statement_purpose", default=None)
_STATEMENT_RUN_ON: ContextVar[str | None] = ContextVar("sqlbuild_statement_run_on", default=None)
_HANDLED_FAILURE: ContextVar[Callable[[BaseException], bool] | None] = ContextVar(
    "sqlbuild_statement_handled_failure", default=None
)
_HANDLED_OUTCOME: str = "handled"
type _ExecutionOwnerKey = tuple[int, int | None]


class StatementLifecycle:
    """Publish exactly one start and terminal fact around a logical statement."""

    def __init__(
        self,
        *,
        adapter: str,
        sql: str,
        intent: str,
        batch_size: int | None = None,
        statement_id: str | None = None,
        query_id_provider: Callable[[], str | None] | None = None,
        heartbeat_threshold_seconds: float = STATEMENT_HEARTBEAT_THRESHOLD_SECONDS,
        heartbeat_interval_seconds: float | None = None,
        clock: Callable[[], float] = time.monotonic,
    ) -> None:
        self._adapter: str = adapter
        self._sql: str = sql
        self._intent: str = intent
        self._batch_size: int | None = batch_size
        self._statement_id: str | None = statement_id
        self._stack: ExitStack = ExitStack()
        self._dispatcher: EventDispatcher | None = None
        self._started_monotonic: float | None = None
        self._terminal = False
        self._owner: StatementLifecycle | None = None
        self._owner_token: Token[StatementLifecycle | None] | None = None
        self._execution_owner_key: _ExecutionOwnerKey | None = None
        self._query_id_provider: Callable[[], str | None] | None = query_id_provider
        self._monitor: StatementMonitor | None = None
        self._heartbeat_threshold_seconds: float = heartbeat_threshold_seconds
        self._heartbeat_interval_seconds: float | None = heartbeat_interval_seconds
        self._clock: Callable[[], float] = clock
        self._pending_completion: tuple[str | None, str | None, int | None, int | None] | None = (
            None
        )
        self._pending_failure: tuple[BaseException, str | None, str | None] | None = None
        self._statement_facts: tuple[str, str] | None = None
        self._purpose: str | None = _STATEMENT_PURPOSE.get()
        self._run_on: str | None = _STATEMENT_RUN_ON.get()
        self._is_handled: Callable[[BaseException], bool] | None = _HANDLED_FAILURE.get()
        self._item_deadline: ItemDeadline | None = None
        self._terminal_query_id: str | None = None

    @staticmethod
    @contextmanager
    def listener_scope(listener: StatementListener) -> Iterator[None]:
        """Deliver each finished outermost statement's SQL text to an in-process listener."""

        with statement_listener_scope(listener):
            yield

    @staticmethod
    @contextmanager
    def purpose_scope(purpose: str) -> Iterator[None]:
        """Label statements run inside the block with why SQLBuild runs them, such as audit."""

        token: Token[str | None] = _STATEMENT_PURPOSE.set(purpose)
        try:
            yield
        finally:
            _STATEMENT_PURPOSE.reset(token)

    @staticmethod
    @contextmanager
    def run_on_scope(run_on: str) -> Iterator[None]:
        """Label statements in the block with where they run; local ones keep no SQL text."""

        token: Token[str | None] = _STATEMENT_RUN_ON.set(run_on)
        try:
            yield
        finally:
            _STATEMENT_RUN_ON.reset(token)

    @staticmethod
    @contextmanager
    def handled_failure_scope(is_handled: Callable[[BaseException], bool]) -> Iterator[None]:
        """Mark failures the caller recovers from, such as a timeout it retries in parts."""

        token: Token[Callable[[BaseException], bool] | None] = _HANDLED_FAILURE.set(is_handled)
        try:
            yield
        finally:
            _HANDLED_FAILURE.reset(token)

    @staticmethod
    def discard_failed_attempt() -> None:
        """Forget a failed attempt deferred to the active statement before an adapter retry."""

        owner: StatementLifecycle | None = _STATEMENT_LIFECYCLE_OWNER.get()
        if owner is not None and owner._execution_owner_key == _current_execution_owner_key():
            owner._pending_failure = None

    def __enter__(self) -> StatementLifecycle:
        execution_owner_key: _ExecutionOwnerKey = _current_execution_owner_key()
        owner: StatementLifecycle | None = _STATEMENT_LIFECYCLE_OWNER.get()
        if owner is not None and owner._execution_owner_key == execution_owner_key:
            self._owner = owner
            self._statement_id = owner.statement_id
            if self._query_id_provider is not None:
                owner.set_query_id_provider(self._query_id_provider)
            return self
        if current_execution_identity() is None:
            self._stack.enter_context(invocation_scope())
        dispatcher: EventDispatcher | None = current_event_dispatcher()
        if dispatcher is None:
            dispatcher = EventDispatcher()
            self._stack.enter_context(dispatcher_scope(dispatcher))
        identity: ExecutionIdentity = self._stack.enter_context(statement_scope(self._statement_id))
        self._statement_id = identity.statement_id
        self._dispatcher = dispatcher
        self._started_monotonic = self._clock()
        self._execution_owner_key = execution_owner_key
        self._publish(event_type="statement_started", payload=self._base_payload())
        if self._run_on != LOCAL_RUN_ON:
            dispatcher.publish_sql_artifact(
                sql=self._sql, statement_id=self._statement_id, resource_id=identity.resource_id
            )
        self._item_deadline = current_item_deadline()
        if self._item_deadline is not None:
            self._item_deadline.statement_started()
        self._owner_token = _STATEMENT_LIFECYCLE_OWNER.set(self)
        self._monitor = StatementMonitor(
            on_submitted=lambda query_id: self.submitted(query_id=query_id),
            on_heartbeat=lambda elapsed_seconds, query_id: self.heartbeat(
                elapsed_seconds=elapsed_seconds, query_id=query_id
            ),
            threshold_seconds=self._heartbeat_threshold_seconds,
            interval_seconds=self._heartbeat_interval_seconds,
            clock=self._clock,
        )
        if self._query_id_provider is not None:
            self._monitor.set_query_id_provider(self._query_id_provider)
        self._monitor.start()
        return self

    @property
    def statement_id(self) -> str:
        if self._statement_id is None:
            raise ObservabilityValidationError("statement lifecycle has not started")
        return self._statement_id

    def submitted(self, *, query_id: str | None = None, job_id: str | None = None) -> None:
        if self._owner is not None:
            self._owner.submitted(query_id=query_id, job_id=job_id)
            return
        payload: dict[str, JSONValue] = self._base_payload()
        if query_id is not None:
            payload["query_id"] = query_id
        if job_id is not None:
            payload["job_id"] = job_id
        if self._item_deadline is not None:
            self._item_deadline.statement_submitted(query_id=query_id or job_id)
        self._publish(event_type="statement_submitted", payload=payload)

    def set_query_id_provider(self, provider: Callable[[], str | None]) -> None:
        """Expose an adapter's non-blocking query-ID reader to the active monitor."""

        if self._owner is not None:
            self._owner.set_query_id_provider(provider)
            return
        monitor: StatementMonitor | None = self._monitor
        if monitor is not None:
            monitor.set_query_id_provider(provider)

    def heartbeat(self, *, elapsed_seconds: float, query_id: str | None = None) -> None:
        """Publish one bounded progress fact for a statement still executing."""

        if self._owner is not None:
            self._owner.heartbeat(elapsed_seconds=elapsed_seconds, query_id=query_id)
            return
        payload: dict[str, JSONValue] = self._base_payload()
        payload["duration_ms"] = max(0.0, elapsed_seconds * 1000.0)
        if query_id is not None:
            payload["query_id"] = query_id
        self._publish(event_type="statement_heartbeat", payload=payload)

    def completed(
        self,
        *,
        query_id: str | None = None,
        job_id: str | None = None,
        affected_rows: int | None = None,
        row_count: int | None = None,
    ) -> None:
        if self._owner is not None:
            self._terminal = True
            self._owner._defer_completed(
                query_id=query_id,
                job_id=job_id,
                affected_rows=affected_rows,
                row_count=row_count,
            )
            return
        discovered_query_id: str | None = self._stop_monitor()
        if query_id is None:
            query_id = discovered_query_id
        if self._pending_failure is not None:
            error, pending_query_id, pending_job_id = self._pending_failure
            self.failed(
                error=error,
                query_id=pending_query_id if pending_query_id is not None else query_id,
                job_id=pending_job_id,
            )
            return
        if self._pending_completion is not None:
            pending_query_id, pending_job_id, pending_affected_rows, pending_row_count = (
                self._pending_completion
            )
            query_id = pending_query_id if pending_query_id is not None else query_id
            job_id = pending_job_id if pending_job_id is not None else job_id
            affected_rows = (
                pending_affected_rows if pending_affected_rows is not None else affected_rows
            )
            row_count = pending_row_count if pending_row_count is not None else row_count
        payload: dict[str, JSONValue] = self._terminal_payload()
        if query_id is not None:
            payload["query_id"] = query_id
        if job_id is not None:
            payload["job_id"] = job_id
        if affected_rows is not None and affected_rows >= 0:
            payload["affected_rows"] = affected_rows
        if row_count is not None and row_count >= 0:
            payload["row_count"] = row_count
        self._publish_terminal(event_type="statement_completed", payload=payload)

    def failed(
        self,
        *,
        error: BaseException,
        query_id: str | None = None,
        job_id: str | None = None,
    ) -> None:
        if self._owner is not None:
            self._terminal = True
            self._owner._defer_failed(error=error, query_id=query_id, job_id=job_id)
            return
        discovered_query_id: str | None = self._stop_monitor()
        if query_id is None:
            query_id = discovered_query_id
        payload: dict[str, JSONValue] = self._terminal_payload()
        payload["error_type"] = type(error).__name__[:_MAX_ERROR_VALUE_LENGTH]
        error_code: str | None = _error_code(error=error)
        if error_code is not None:
            payload["error_code"] = error_code
        if query_id is not None:
            payload["query_id"] = query_id
        if job_id is not None:
            payload["job_id"] = job_id
        if self._failure_handled(error=error):
            payload["outcome"] = _HANDLED_OUTCOME
        self._publish_terminal(event_type="statement_failed", payload=payload)

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_value: BaseException | None,
        traceback: TracebackType | None,
    ) -> None:
        del exc_type, traceback
        if self._owner is not None:
            if exc_value is not None and not self._terminal:
                self.failed(error=exc_value)
            return
        try:
            if not self._terminal:
                if self._pending_failure is not None:
                    error, query_id, job_id = self._pending_failure
                    self.failed(error=error, query_id=query_id, job_id=job_id)
                elif exc_value is None:
                    self.completed()
                else:
                    self.failed(error=exc_value)
        finally:
            self._stop_monitor()
            if self._item_deadline is not None:
                self._item_deadline.statement_finished(query_id=self._terminal_query_id)
                self._item_deadline = None
            if self._owner_token is not None:
                _STATEMENT_LIFECYCLE_OWNER.reset(self._owner_token)
                self._owner_token = None
            self._stack.close()
            notify_statement_finished(sql=self._sql)

    def _base_payload(self) -> dict[str, JSONValue]:
        if self._statement_facts is None:
            self._statement_facts = (
                hashlib.sha256(self._sql.encode("utf-8")).hexdigest(),
                _statement_kind(sql=self._sql),
            )
        sql_digest, statement_kind = self._statement_facts
        payload: dict[str, JSONValue] = {
            "adapter": self._adapter,
            "intent": self._intent,
            "sql_digest": sql_digest,
            "statement_kind": statement_kind,
        }
        if self._batch_size is not None:
            payload["batch_size"] = self._batch_size
        if self._purpose is not None:
            payload["purpose"] = self._purpose
        if self._run_on is not None:
            payload["run_on"] = self._run_on
        return payload

    def _terminal_payload(self) -> dict[str, JSONValue]:
        payload: dict[str, JSONValue] = self._base_payload()
        started: float = self._started_monotonic if self._started_monotonic is not None else 0.0
        payload["duration_ms"] = max(0.0, (self._clock() - started) * 1000.0)
        return payload

    def _failure_handled(self, *, error: BaseException) -> bool:
        is_handled: Callable[[BaseException], bool] | None = self._is_handled
        if is_handled is None:
            return False
        return bool(is_handled(error))

    def _stop_monitor(self) -> str | None:
        monitor: StatementMonitor | None = self._monitor
        if monitor is None:
            return None
        self._monitor = None
        return monitor.stop()

    def _publish(self, *, event_type: str, payload: Mapping[str, JSONValue]) -> None:
        if self._dispatcher is None:
            raise ObservabilityValidationError("statement lifecycle has not started")
        self._dispatcher.publish_lifecycle(
            create_lifecycle_event(event_type=event_type, payload=payload)
        )

    def _publish_terminal(self, *, event_type: str, payload: Mapping[str, JSONValue]) -> None:
        if self._terminal:
            return
        self._terminal = True
        terminal_id: JSONValue | None = payload.get("query_id") or payload.get("job_id")
        self._terminal_query_id = terminal_id if isinstance(terminal_id, str) else None
        self._publish(event_type=event_type, payload=payload)

    def _defer_completed(
        self,
        *,
        query_id: str | None,
        job_id: str | None,
        affected_rows: int | None,
        row_count: int | None,
    ) -> None:
        self._pending_completion = (query_id, job_id, affected_rows, row_count)

    def _defer_failed(
        self, *, error: BaseException, query_id: str | None, job_id: str | None
    ) -> None:
        self._pending_failure = (error, query_id, job_id)


def _current_execution_owner_key() -> _ExecutionOwnerKey:
    try:
        task: asyncio.Task[object] | None = asyncio.current_task()
    except RuntimeError:
        task = None
    return threading.get_ident(), None if task is None else id(task)


def _statement_kind(*, sql: str) -> str:
    stripped: str = sql.lstrip()
    match: re.Match[str] | None = _STATEMENT_KIND_PATTERN.match(stripped)
    return match.group(1).upper() if match is not None else "UNKNOWN"


def _error_code(*, error: BaseException) -> str | None:
    for attribute in ("sqlstate", "code", "errno"):
        value: object | None = getattr(error, attribute, None)
        if isinstance(value, bool) or not isinstance(value, str | int):
            continue
        code: str = str(value)
        if _ERROR_CODE_PATTERN.fullmatch(code) is not None:
            return code
    return None
