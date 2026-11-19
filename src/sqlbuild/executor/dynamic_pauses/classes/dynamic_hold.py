"""One build's hold on the dynamic models it touches: pause, record, beat, then resume."""

from __future__ import annotations

import threading
import time
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from datetime import datetime, timedelta
from typing import Any

from sqlbuild._native import CompiledRelationLocation
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.contract.types import DynamicOperation
from sqlbuild.executor.dynamic_pauses._helpers.resume import (
    resume_holds,
    run_dynamic_operation,
    scheduling_state,
)
from sqlbuild.executor.dynamic_pauses._helpers.sql import (
    insert_sql,
    read_open_holds,
    setup_statements,
)
from sqlbuild.executor.dynamic_pauses.classes.dynamic_heartbeat import (
    DynamicHeartbeat,
)
from sqlbuild.executor.dynamic_pauses.constants import (
    ACTIVE_STATE,
    HELD_ELSEWHERE_CODE,
    SUSPENDED_STATE,
)
from sqlbuild.executor.dynamic_pauses.models import DynamicPauseEvent, OpenHold
from sqlbuild.executor.dynamic_pauses.types import DynamicPauseEventType, StateLocation

type _RelationKey = tuple[str | None, str | None, str]


class DynamicHold:
    """Pause the held models a build found active, record it, and resume only those at the end."""

    def __init__(
        self,
        *,
        adapter: BaseAdapter,
        held: dict[str, CompiledRelationLocation],
        refreshed: frozenset[str],
        state_location: StateLocation | None,
        settings: tuple[str, bool, float, bool],
        heartbeat: Callable[[Callable[..., None]], DynamicHeartbeat],
    ) -> None:
        self._adapter: BaseAdapter = adapter
        self._held: dict[str, CompiledRelationLocation] = held
        self._refreshed: frozenset[str] = refreshed
        self._state_location: StateLocation | None = state_location
        self._run_id: str
        self._pause_dynamic: bool
        self._grace_seconds: float
        self._ignore_grace: bool
        self._run_id, self._pause_dynamic, self._grace_seconds, self._ignore_grace = settings
        self._new_heartbeat: Callable[[Callable[..., None]], DynamicHeartbeat] = heartbeat
        self._heartbeat: DynamicHeartbeat | None = None
        self._paused: list[str] = []
        self._left_paused: frozenset[str] = frozenset()
        self._suspended_unheld: list[str] = []
        self._lock: threading.Lock = threading.Lock()
        self._on_progress: Callable[[str], None] | None = None

    @contextmanager
    def holding(
        self, *, connection: Any, on_progress: Callable[[str], None] | None
    ) -> Iterator[None]:
        """Hold for the build; after a failure, resume without hiding its error."""

        try:
            self.acquire(connection=connection, on_progress=on_progress)
            yield
        except BaseException:
            try:
                self.release(connection=connection)
            except Exception as error:
                self._warn(f"could not resume the dynamic models this build paused: {error}")
            raise
        self.release(connection=connection)

    def refreshes(self, *, name: str) -> bool:
        """Whether a kept or altered model is refreshed by hand once its inputs are built."""

        return self._pause_dynamic or name in self._refreshed

    def acquire(self, *, connection: Any, on_progress: Callable[[str], None] | None) -> None:
        """Take over expired holds, refuse live ones, then pause the held models found active."""

        self._on_progress = on_progress
        if self._pause_dynamic:
            self._suspend_active(connection=connection, names=tuple(self._held))
            return
        location: StateLocation | None = self._state_location
        if location is None:
            return
        for statement in setup_statements(adapter=self._adapter, location=location):
            self._adapter.execute(connection=connection, sql=statement)
        taken: tuple[OpenHold, ...] = self._take_over(connection=connection, location=location)
        taken_keys: set[_RelationKey] = {_key(hold.relation) for hold in taken}
        states: dict[str, str | None] = {
            name: self._state(connection=connection, name=name) for name in self._held
        }
        self._paused = [
            name
            for name in self._held
            if states[name] == ACTIVE_STATE or _key(self._held[name]) in taken_keys
        ]
        self._left_paused = frozenset(
            name
            for name in self._held
            if states[name] == SUSPENDED_STATE and name not in self._paused
        )
        for name in sorted(self._left_paused):
            self._warn(f"dynamic model {name} was paused outside SQLBuild; it stays paused")
        window: timedelta = self._timed_record(
            connection=connection,
            events=(
                *(
                    self._event(event_type=DynamicPauseEventType.RESUMED, hold=hold)
                    for hold in taken
                ),
                *(
                    self._own(event_type=DynamicPauseEventType.PAUSED, name=name)
                    for name in self._paused
                ),
            ),
        )
        self._yield_to_rivals(
            connection=connection, location=location, names=tuple(self._paused), window=window
        )
        states.update(
            {
                name: self._state(connection=connection, name=name)
                for name in self._paused
                if _key(self._held[name]) in taken_keys
            }
        )
        self._suspend_active(connection=connection, names=tuple(self._paused), states=states)
        self._heartbeat = self._new_heartbeat(self._write_heartbeat)
        self._heartbeat.start()

    def after_create(self, *, connection: Any, name: str) -> None:
        """Suspend a model CREATE OR REPLACE made active; check its new hold as acquire does."""

        if name not in self._held:
            return
        with self._lock:
            newly_paused: bool = not (
                self._pause_dynamic or name in self._paused or name in self._left_paused
            )
            if newly_paused:
                self._paused.append(name)
        window: timedelta = timedelta()
        if newly_paused:
            window = self._timed_record(
                connection=connection,
                events=(self._own(event_type=DynamicPauseEventType.PAUSED, name=name),),
            )
        self._operate(connection=connection, name=name, operation=DynamicOperation.SUSPEND)
        location: StateLocation | None = self._state_location
        if newly_paused and location is not None:
            try:
                self._yield_to_rivals(
                    connection=connection, location=location, names=(name,), window=window
                )
            except AdapterUserError:
                with self._lock:
                    self._suspended_unheld.append(name)
                raise

    def release(self, *, connection: Any) -> None:
        """Stop the heartbeat and resume what this build paused, upstream first."""

        if self._heartbeat is not None:
            self._heartbeat.stop()
            self._report_heartbeat_failure(failure=self._heartbeat.failure)
        order: list[str] = list(self._held)
        holds: tuple[OpenHold, ...] | None = self._open_holds(connection=connection)
        still_held: frozenset[_RelationKey] | None = (
            None
            if holds is None
            else frozenset(_key(hold.relation) for hold in holds if hold.run_id == self._run_id)
        )
        resumed: list[str] = []
        failures: list[Exception] = self._resume_unheld(connection=connection, holds=holds)
        for name in sorted(self._paused, key=order.index):
            if still_held is not None and _key(self._held[name]) not in still_held:
                continue
            try:
                self._operate(connection=connection, name=name, operation=DynamicOperation.RESUME)
                resumed.append(name)
            except Exception as error:
                failures.append(error)
        self._record(
            connection=connection,
            events=tuple(
                self._own(event_type=DynamicPauseEventType.RESUMED, name=name) for name in resumed
            ),
        )
        if failures:
            raise failures[0]

    def _take_over(self, *, connection: Any, location: StateLocation) -> tuple[OpenHold, ...]:
        """Resume expired holds on other models now; return the held models' holds to take over."""

        held_keys: set[_RelationKey] = {_key(location) for location in self._held.values()}
        holds: tuple[OpenHold, ...] = tuple(
            hold
            for hold in read_open_holds(
                adapter=self._adapter, connection=connection, location=location
            )
            if hold.run_id != self._run_id
        )
        live: tuple[OpenHold, ...] = tuple(
            hold
            for hold in holds
            if hold.idle_seconds < self._grace_seconds and _key(hold.relation) in held_keys
        )
        if live and not self._ignore_grace:
            raise _held_elsewhere(holds=live)
        expired: tuple[OpenHold, ...] = tuple(
            hold for hold in holds if hold.idle_seconds >= self._grace_seconds
        )
        elsewhere: tuple[OpenHold, ...] = tuple(
            hold for hold in expired if _key(hold.relation) not in held_keys
        )
        resume_holds(
            adapter=self._adapter,
            connection=connection,
            location=location,
            holds=elsewhere,
            recorded_by=self._run_id,
        )
        taken: tuple[OpenHold, ...] = (*live, *(h for h in expired if h not in elsewhere))
        self._report(holds=elsewhere, verb="Resumed")
        self._report(holds=taken, verb="Took over")
        return taken

    def _yield_to_rivals(
        self,
        *,
        connection: Any,
        location: StateLocation,
        names: tuple[str, ...],
        window: timedelta,
    ) -> None:
        """Read the pauses back; if another live hold may have come first, close ours and stop."""

        mine: set[_RelationKey] = {_key(self._held[name]) for name in names}
        if not mine:
            return
        holds: tuple[OpenHold, ...] = read_open_holds(
            adapter=self._adapter, connection=connection, location=location
        )
        paused_at: dict[_RelationKey, datetime] = {
            _key(hold.relation): hold.paused_at
            for hold in holds
            if hold.run_id == self._run_id and _key(hold.relation) in mine
        }
        rivals: list[OpenHold] = sorted(
            (
                hold
                for hold in holds
                if hold.run_id != self._run_id
                and hold.idle_seconds < self._grace_seconds
                and _key(hold.relation) in mine
                and (
                    _key(hold.relation) not in paused_at
                    or hold.paused_at <= paused_at[_key(hold.relation)] + window
                )
            ),
            key=lambda hold: (hold.paused_at, hold.run_id),
        )
        if not rivals:
            return
        with self._lock:
            self._paused = [name for name in self._paused if name not in names]
        try:
            self._record(
                connection=connection,
                events=tuple(
                    self._own(event_type=DynamicPauseEventType.RESUMED, name=name) for name in names
                ),
            )
        except Exception as error:
            raise _held_elsewhere(holds=tuple(rivals), close_failure=error) from error
        raise _held_elsewhere(holds=tuple(rivals))

    def _resume_unheld(
        self, *, connection: Any, holds: tuple[OpenHold, ...] | None
    ) -> list[Exception]:
        """Resume the models this build suspended after creating them, then gave up, if unheld."""

        held_elsewhere: frozenset[_RelationKey] = frozenset(
            _key(hold.relation) for hold in holds or () if hold.run_id != self._run_id
        )
        failures: list[Exception] = []
        for name in self._suspended_unheld:
            if holds is None or _key(self._held[name]) in held_elsewhere:
                continue
            try:
                if self._state(connection=connection, name=name) == SUSPENDED_STATE:
                    self._operate(
                        connection=connection, name=name, operation=DynamicOperation.RESUME
                    )
            except Exception as error:
                failures.append(error)
        return failures

    def _open_holds(self, *, connection: Any) -> tuple[OpenHold, ...] | None:
        """Every open hold, or None when the pause table can't say."""

        location: StateLocation | None = self._state_location
        if location is None or self._pause_dynamic:
            return None
        try:
            holds: tuple[OpenHold, ...] = read_open_holds(
                adapter=self._adapter, connection=connection, location=location
            )
        except Exception as error:
            self._warn(f"could not read which dynamic models this build still holds: {error}")
            return None
        return holds

    def _suspend_active(
        self,
        *,
        connection: Any,
        names: tuple[str, ...],
        states: dict[str, str | None] | None = None,
    ) -> None:
        for name in names:
            state: str | None = (
                states[name]
                if states is not None
                else self._state(connection=connection, name=name)
            )
            if state == ACTIVE_STATE:
                self._operate(connection=connection, name=name, operation=DynamicOperation.SUSPEND)

    def _state(self, *, connection: Any, name: str) -> str | None:
        return scheduling_state(
            adapter=self._adapter, connection=connection, relation=self._held[name]
        )

    def _operate(self, *, connection: Any, name: str, operation: DynamicOperation) -> None:
        run_dynamic_operation(
            adapter=self._adapter,
            connection=connection,
            relation=self._held[name],
            operation=operation,
        )

    def _timed_record(self, *, connection: Any, events: tuple[DynamicPauseEvent, ...]) -> timedelta:
        """Record, and return how long the insert took: an upper bound on its commit delay."""

        started: float = time.monotonic()
        self._record(connection=connection, events=events)
        return timedelta(seconds=time.monotonic() - started)

    def _record(self, *, connection: Any, events: tuple[DynamicPauseEvent, ...]) -> None:
        location: StateLocation | None = self._state_location
        if not events or location is None:
            return
        self._adapter.execute(
            connection=connection,
            sql=insert_sql(adapter=self._adapter, location=location, events=events),
        )

    def _write_heartbeat(self, *, connection: Any, sequence: int) -> None:
        self._record(
            connection=connection,
            events=(
                DynamicPauseEvent(
                    event_type=DynamicPauseEventType.HEARTBEAT,
                    run_id=self._run_id,
                    recorded_by=self._run_id,
                    relation=None,
                    sequence=sequence,
                ),
            ),
        )

    def _own(self, *, event_type: DynamicPauseEventType, name: str) -> DynamicPauseEvent:
        return DynamicPauseEvent(
            event_type=event_type,
            run_id=self._run_id,
            recorded_by=self._run_id,
            relation=self._held[name],
        )

    def _event(self, *, event_type: DynamicPauseEventType, hold: OpenHold) -> DynamicPauseEvent:
        return DynamicPauseEvent(
            event_type=event_type,
            run_id=hold.run_id,
            recorded_by=self._run_id,
            relation=hold.relation,
        )

    def _report(self, *, holds: tuple[OpenHold, ...], verb: str) -> None:
        if self._on_progress is None:
            return
        for hold in holds:
            self._on_progress(
                f"{verb} dynamic model {hold.relation.name}, paused by run {hold.run_id} "
                f"whose heartbeat has been silent for {int(hold.idle_seconds)}s"
            )

    def _report_heartbeat_failure(self, *, failure: Exception | None) -> None:
        if failure is not None:
            self._warn(f"a dynamic pause heartbeat failed: {failure}")

    def _warn(self, message: str) -> None:
        if self._on_progress is not None:
            self._on_progress(f"Warning: {message}")


def _key(location: CompiledRelationLocation) -> _RelationKey:
    return (location.database, location.schema, location.name)


def _held_elsewhere(
    *, holds: tuple[OpenHold, ...], close_failure: Exception | None = None
) -> AdapterUserError:
    first: OpenHold = holds[0]
    names: str = ", ".join(sorted({hold.relation.name for hold in holds}))
    closing: str = (
        ""
        if close_failure is None
        else f"; closing this build's own pause also failed: {close_failure}"
    )
    return AdapterUserError(
        f"run {first.run_id} holds dynamic models this build needs ({names}); "
        f"its last heartbeat was {int(first.idle_seconds)}s ago{closing}",
        code=HELD_ELSEWHERE_CODE,
        help=(
            "wait for that build to finish, or for its hold to pass [dynamic] "
            "heartbeat_grace; with allow_ignore_grace = true, "
            "sqb build --ignore-dynamic-grace takes over at once"
        ),
    )
