"""Resume dynamic models through the adapter under a claim, and close the holds that paused them."""

from __future__ import annotations

import contextlib
import uuid
from typing import Any

from sqlbuild._native import DYNAMIC_PAUSES_TABLE_NAME, CompiledRelationLocation
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.types import DynamicListing, DynamicOperation
from sqlbuild.adapter.relations.types import RelationChangeTokenReader
from sqlbuild.executor.dynamic_pauses._helpers.sql import insert_sql, read_open_holds
from sqlbuild.executor.dynamic_pauses.constants import CLAIM_RUN_MARKER, SUSPENDED_STATE
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


def _resume_suspended(
    *, adapter: BaseAdapter, connection: Any, relation: CompiledRelationLocation
) -> None:
    """Resume if suspended; known gap: an ALTER sent just before a kill may land after its claim."""

    if scheduling_state(adapter=adapter, connection=connection, relation=relation) == (
        SUSPENDED_STATE
    ):
        run_dynamic_operation(
            adapter=adapter,
            connection=connection,
            relation=relation,
            operation=DynamicOperation.RESUME,
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
    outcome: HoldsResumed = claim_and_resume(
        adapter=adapter,
        connection=connection,
        location=location,
        holds=expired,
        holds_after=holds_after,
    )
    return ResumeOutcome(
        resumed=outcome.resumed,
        failures=tuple(f"{_dotted(hold.relation)}: {error}" for hold, error in outcome.failures),
        quiet_token=None if holds else token,
    )


def claim_and_resume(
    *,
    adapter: BaseAdapter,
    connection: Any,
    location: StateLocation,
    holds: tuple[OpenHold, ...],
    holds_after: tuple[float, str],
) -> HoldsResumed:
    """Claim the expired holds' models, then resume only those no run holds live after the claim."""

    grace_seconds: float
    recorded_by: str
    grace_seconds, recorded_by = holds_after
    if not holds:
        return HoldsResumed(resumed=(), failures=())
    claim_run: str = f"{recorded_by}{CLAIM_RUN_MARKER}{uuid.uuid4().hex}"
    claimed: tuple[CompiledRelationLocation, ...] = tuple(
        {_relation_key(hold.relation): hold.relation for hold in holds}.values()
    )
    claims: tuple[DynamicPauseEvent, ...] = tuple(
        DynamicPauseEvent(
            event_type=DynamicPauseEventType.RESUMING,
            run_id=claim_run,
            recorded_by=recorded_by,
            relation=relation,
        )
        for relation in claimed
    )
    _append(adapter=adapter, connection=connection, location=location, events=claims)
    closes: tuple[DynamicPauseEvent, ...] = tuple(
        DynamicPauseEvent(
            event_type=DynamicPauseEventType.RESUMED,
            run_id=claim_run,
            recorded_by=recorded_by,
            relation=relation,
        )
        for relation in claimed
    )
    try:
        live: frozenset[tuple[str | None, str | None, str]] = frozenset(
            _relation_key(hold.relation)
            for hold in read_open_holds(adapter=adapter, connection=connection, location=location)
            if hold.idle_seconds < grace_seconds
        )
    except Exception:
        with contextlib.suppress(Exception):
            _append(adapter=adapter, connection=connection, location=location, events=closes)
        raise
    resumed: list[OpenHold] = []
    failures: list[tuple[OpenHold, Exception]] = []
    for hold in holds:
        if _relation_key(hold.relation) in live:
            continue
        try:
            _resume_suspended(adapter=adapter, connection=connection, relation=hold.relation)
        except Exception as error:
            failures.append((hold, error))
            continue
        resumed.append(hold)
    _append(
        adapter=adapter,
        connection=connection,
        location=location,
        events=(
            *(
                DynamicPauseEvent(
                    event_type=DynamicPauseEventType.RESUMED,
                    run_id=hold.run_id,
                    recorded_by=recorded_by,
                    relation=hold.relation,
                )
                for hold in resumed
            ),
            *closes,
        ),
    )
    return HoldsResumed(resumed=tuple(resumed), failures=tuple(failures))


def _append(
    *,
    adapter: BaseAdapter,
    connection: Any,
    location: StateLocation,
    events: tuple[DynamicPauseEvent, ...],
) -> None:
    adapter.execute(
        connection=connection,
        sql=insert_sql(adapter=adapter, location=location, events=events),
    )


def _relation_key(relation: CompiledRelationLocation) -> tuple[str | None, str | None, str]:
    return (relation.database, relation.schema, relation.name)


def _dotted(relation: CompiledRelationLocation) -> str:
    return ".".join(part for part in (relation.database, relation.schema, relation.name) if part)
