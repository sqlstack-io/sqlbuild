"""One build's hold on the maintained models it touches: pause, record, beat, then resume."""

from __future__ import annotations

import threading
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from typing import Any

from sqlbuild._native import CompiledRelationLocation
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.contract.types import MaintainedOperation
from sqlbuild.executor.maintained_pauses._helpers.resume import (
    resume_holds,
    run_maintained_operation,
    scheduling_state,
)
from sqlbuild.executor.maintained_pauses._helpers.sql import (
    insert_sql,
    read_open_holds,
    setup_statements,
)
from sqlbuild.executor.maintained_pauses.classes.maintained_heartbeat import (
    MaintainedHeartbeat,
)
from sqlbuild.executor.maintained_pauses.constants import (
    ACTIVE_STATE,
    HELD_ELSEWHERE_CODE,
    SUSPENDED_STATE,
)
from sqlbuild.executor.maintained_pauses.models import MaintainedPauseEvent, OpenHold
from sqlbuild.executor.maintained_pauses.types import MaintainedPauseEventType, StateLocation

type _RelationKey = tuple[str | None, str | None, str]


class MaintainedHold:
    """Pause the held models a build found active, record it, and resume only those at the end."""

    def __init__(
        self,
        *,
        adapter: BaseAdapter,
        held: dict[str, CompiledRelationLocation],
        refreshed: frozenset[str],
        state_location: StateLocation | None,
        settings: tuple[str, bool, float, bool],
        heartbeat: Callable[[Callable[..., None]], MaintainedHeartbeat],
    ) -> None:
        self._adapter: BaseAdapter = adapter
        self._held: dict[str, CompiledRelationLocation] = held
        self._refreshed: frozenset[str] = refreshed
        self._state_location: StateLocation | None = state_location
        self._run_id: str
        self._pause_maintained: bool
        self._grace_seconds: float
        self._ignore_grace: bool
        self._run_id, self._pause_maintained, self._grace_seconds, self._ignore_grace = settings
        self._new_heartbeat: Callable[[Callable[..., None]], MaintainedHeartbeat] = heartbeat
        self._heartbeat: MaintainedHeartbeat | None = None
        self._paused: list[str] = []
        self._left_paused: frozenset[str] = frozenset()
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
                self._warn(f"could not resume the maintained models this build paused: {error}")
            raise
        self.release(connection=connection)

    def refreshes(self, *, name: str) -> bool:
        """Whether a kept or altered model is refreshed by hand once its inputs are built."""

        return self._pause_maintained or name in self._refreshed

    def acquire(self, *, connection: Any, on_progress: Callable[[str], None] | None) -> None:
        """Take over expired holds, refuse live ones, then pause the held models found active."""

        self._on_progress = on_progress
        if self._pause_maintained:
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
            self._warn(f"maintained model {name} was paused outside SQLBuild; it stays paused")
        self._record(
            connection=connection,
            events=(
                *(
                    self._event(event_type=MaintainedPauseEventType.RESUMED, hold=hold)
                    for hold in taken
                ),
                *(
                    self._own(event_type=MaintainedPauseEventType.PAUSED, name=name)
                    for name in self._paused
                ),
            ),
        )
        self._yield_to_earlier(connection=connection, location=location)
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
        """Suspend a held model that CREATE OR REPLACE made active; resume it only if ours."""

        if name not in self._held:
            return
        with self._lock:
            newly_paused: bool = not (
                self._pause_maintained or name in self._paused or name in self._left_paused
            )
            if newly_paused:
                self._paused.append(name)
        if newly_paused:
            self._record(
                connection=connection,
                events=(self._own(event_type=MaintainedPauseEventType.PAUSED, name=name),),
            )
        self._operate(connection=connection, name=name, operation=MaintainedOperation.SUSPEND)

    def release(self, *, connection: Any) -> None:
        """Stop the heartbeat and resume what this build paused, upstream first."""

        if self._heartbeat is not None:
            self._heartbeat.stop()
            self._report_heartbeat_failure(failure=self._heartbeat.failure)
        order: list[str] = list(self._held)
        still_held: frozenset[_RelationKey] | None = self._still_held(connection=connection)
        resumed: list[str] = []
        failures: list[Exception] = []
        for name in sorted(self._paused, key=order.index):
            if still_held is not None and _key(self._held[name]) not in still_held:
                continue
            try:
                self._operate(
                    connection=connection, name=name, operation=MaintainedOperation.RESUME
                )
                resumed.append(name)
            except Exception as error:
                failures.append(error)
        self._record(
            connection=connection,
            events=tuple(
                self._own(event_type=MaintainedPauseEventType.RESUMED, name=name)
                for name in resumed
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

    def _yield_to_earlier(self, *, connection: Any, location: StateLocation) -> None:
        """Read the pauses back; if another live hold on a model came first, close ours and stop."""

        mine: set[_RelationKey] = {_key(self._held[name]) for name in self._paused}
        if not mine:
            return
        holds: tuple[OpenHold, ...] = read_open_holds(
            adapter=self._adapter, connection=connection, location=location
        )
        first: dict[_RelationKey, OpenHold] = {}
        for hold in sorted(holds, key=lambda hold: (hold.paused_at, hold.run_id)):
            if hold.idle_seconds < self._grace_seconds and _key(hold.relation) in mine:
                first.setdefault(_key(hold.relation), hold)
        winners: tuple[OpenHold, ...] = tuple(
            hold for hold in first.values() if hold.run_id != self._run_id
        )
        if not winners:
            return
        self._record(
            connection=connection,
            events=tuple(
                self._own(event_type=MaintainedPauseEventType.RESUMED, name=name)
                for name in self._paused
            ),
        )
        self._paused = []
        raise _held_elsewhere(holds=winners)

    def _still_held(self, *, connection: Any) -> frozenset[_RelationKey] | None:
        """This run's holds nobody took over, or None when the pause table can't say."""

        location: StateLocation | None = self._state_location
        if location is None or self._pause_maintained:
            return None
        try:
            holds: tuple[OpenHold, ...] = read_open_holds(
                adapter=self._adapter, connection=connection, location=location
            )
        except Exception as error:
            self._warn(f"could not read which maintained models this build still holds: {error}")
            return None
        return frozenset(_key(hold.relation) for hold in holds if hold.run_id == self._run_id)

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
                self._operate(
                    connection=connection, name=name, operation=MaintainedOperation.SUSPEND
                )

    def _state(self, *, connection: Any, name: str) -> str | None:
        return scheduling_state(
            adapter=self._adapter, connection=connection, relation=self._held[name]
        )

    def _operate(self, *, connection: Any, name: str, operation: MaintainedOperation) -> None:
        run_maintained_operation(
            adapter=self._adapter,
            connection=connection,
            relation=self._held[name],
            operation=operation,
        )

    def _record(self, *, connection: Any, events: tuple[MaintainedPauseEvent, ...]) -> None:
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
                MaintainedPauseEvent(
                    event_type=MaintainedPauseEventType.HEARTBEAT,
                    run_id=self._run_id,
                    recorded_by=self._run_id,
                    relation=None,
                    sequence=sequence,
                ),
            ),
        )

    def _own(self, *, event_type: MaintainedPauseEventType, name: str) -> MaintainedPauseEvent:
        return MaintainedPauseEvent(
            event_type=event_type,
            run_id=self._run_id,
            recorded_by=self._run_id,
            relation=self._held[name],
        )

    def _event(
        self, *, event_type: MaintainedPauseEventType, hold: OpenHold
    ) -> MaintainedPauseEvent:
        return MaintainedPauseEvent(
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
                f"{verb} maintained model {hold.relation.name}, paused by run {hold.run_id} "
                f"whose heartbeat has been silent for {int(hold.idle_seconds)}s"
            )

    def _report_heartbeat_failure(self, *, failure: Exception | None) -> None:
        if failure is not None:
            self._warn(f"a maintained pause heartbeat failed: {failure}")

    def _warn(self, message: str) -> None:
        if self._on_progress is not None:
            self._on_progress(f"Warning: {message}")


def _key(location: CompiledRelationLocation) -> _RelationKey:
    return (location.database, location.schema, location.name)


def _held_elsewhere(*, holds: tuple[OpenHold, ...]) -> AdapterUserError:
    first: OpenHold = holds[0]
    names: str = ", ".join(sorted({hold.relation.name for hold in holds}))
    return AdapterUserError(
        f"run {first.run_id} holds maintained models this build needs ({names}); "
        f"its last heartbeat was {int(first.idle_seconds)}s ago",
        code=HELD_ELSEWHERE_CODE,
        help=(
            "wait for that build to finish, or for its hold to pass [maintained] "
            "heartbeat_grace; with allow_ignore_grace = true, "
            "sqb build --ignore-maintained-grace takes over at once"
        ),
    )
