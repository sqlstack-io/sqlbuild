"""Resume dynamic models through the adapter, and close the holds that paused them."""

from __future__ import annotations

from typing import Any

from sqlbuild._native import DYNAMIC_PAUSES_TABLE_NAME, CompiledRelationLocation
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.types import DynamicListing, DynamicOperation
from sqlbuild.adapter.relations.types import RelationChangeTokenReader
from sqlbuild.executor.dynamic_pauses._helpers.sql import insert_sql, read_open_holds
from sqlbuild.executor.dynamic_pauses.constants import SUSPENDED_STATE
from sqlbuild.executor.dynamic_pauses.models import (
    DynamicPauseEvent,
    HoldsResumed,
    OpenHold,
    ResumeOutcome,
)
from sqlbuild.executor.dynamic_pauses.types import DynamicPauseEventType, StateLocation


def scheduling_state(
    *, adapter: BaseAdapter, connection: Any, relation: CompiledRelationLocation
) -> str | None:
    """A dynamic model's scheduling state, or None when the warehouse does not list it."""

    listing: DynamicListing | None = adapter.dynamic_listing(
        connection=connection,
        database=relation.database,
        schema=relation.schema,
        name=relation.name,
    )
    return None if listing is None else listing.scheduling_state


def run_dynamic_operation(
    *,
    adapter: BaseAdapter,
    connection: Any,
    relation: CompiledRelationLocation,
    operation: DynamicOperation,
) -> None:
    """Suspend or resume one dynamic model."""

    destination: str = relation.qualified_name or str(
        adapter.render_qualified_name(
            database=relation.database, schema=relation.schema, name=relation.name
        )
    )
    for statement in adapter.render_dynamic_operation(destination=destination, operation=operation):
        adapter.execute(connection=connection, sql=statement)


def resume_holds(
    *,
    adapter: BaseAdapter,
    connection: Any,
    location: StateLocation,
    holds: tuple[OpenHold, ...],
    recorded_by: str,
) -> HoldsResumed:
    """Resume each held model still suspended and close its hold; a failure leaves only it open."""

    resumed: list[OpenHold] = []
    failures: list[tuple[OpenHold, Exception]] = []
    for hold in holds:
        try:
            _resume_suspended(adapter=adapter, connection=connection, relation=hold.relation)
        except Exception as error:
            failures.append((hold, error))
            continue
        resumed.append(hold)
    if resumed:
        _close_holds(
            adapter=adapter,
            connection=connection,
            location=location,
            holds=(tuple(resumed), recorded_by),
        )
    return HoldsResumed(resumed=tuple(resumed), failures=tuple(failures))


def _resume_suspended(
    *, adapter: BaseAdapter, connection: Any, relation: CompiledRelationLocation
) -> None:
    if scheduling_state(adapter=adapter, connection=connection, relation=relation) == (
        SUSPENDED_STATE
    ):
        run_dynamic_operation(
            adapter=adapter,
            connection=connection,
            relation=relation,
            operation=DynamicOperation.RESUME,
        )


def _close_holds(
    *,
    adapter: BaseAdapter,
    connection: Any,
    location: StateLocation,
    holds: tuple[tuple[OpenHold, ...], str],
) -> None:
    closed: tuple[OpenHold, ...]
    recorded_by: str
    closed, recorded_by = holds
    adapter.execute(
        connection=connection,
        sql=insert_sql(
            adapter=adapter,
            location=location,
            events=tuple(
                DynamicPauseEvent(
                    event_type=DynamicPauseEventType.RESUMED,
                    run_id=hold.run_id,
                    recorded_by=recorded_by,
                    relation=hold.relation,
                )
                for hold in closed
            ),
        ),
    )


def resume_expired(
    *,
    adapter: BaseAdapter,
    connection: Any,
    location: StateLocation,
    holds_after: tuple[float, str],
    quiet_token: str | None,
) -> ResumeOutcome:
    """Resume holds silent past the grace seconds unless the pause table still has `quiet_token`."""

    grace_seconds: float
    recorded_by: str
    grace_seconds, recorded_by = holds_after
    if not adapter.relation_exists(
        connection=connection,
        database=location[0],
        schema=location[1],
        name=DYNAMIC_PAUSES_TABLE_NAME,
    ):
        return ResumeOutcome(resumed=(), failures=(), quiet_token=None)
    token: str | None = (
        adapter.read_relation_change_token(
            connection=connection,
            database=location[0],
            schema=location[1],
            name=DYNAMIC_PAUSES_TABLE_NAME,
        )
        if isinstance(adapter, RelationChangeTokenReader)
        else None
    )
    if token is not None and token == quiet_token:
        return ResumeOutcome(resumed=(), failures=(), quiet_token=token)
    holds: tuple[OpenHold, ...] = read_open_holds(
        adapter=adapter, connection=connection, location=location
    )
    live: set[tuple[str | None, str | None, str]] = {
        (hold.relation.database, hold.relation.schema, hold.relation.name)
        for hold in holds
        if hold.idle_seconds < grace_seconds
    }
    expired: tuple[OpenHold, ...] = tuple(
        hold
        for hold in holds
        if hold.idle_seconds >= grace_seconds
        and (hold.relation.database, hold.relation.schema, hold.relation.name) not in live
    )
    outcome: HoldsResumed = resume_holds(
        adapter=adapter,
        connection=connection,
        location=location,
        holds=expired,
        recorded_by=recorded_by,
    )
    return ResumeOutcome(
        resumed=outcome.resumed,
        failures=tuple(f"{_dotted(hold.relation)}: {error}" for hold, error in outcome.failures),
        quiet_token=None if holds else token,
    )


def _dotted(relation: CompiledRelationLocation) -> str:
    return ".".join(part for part in (relation.database, relation.schema, relation.name) if part)
