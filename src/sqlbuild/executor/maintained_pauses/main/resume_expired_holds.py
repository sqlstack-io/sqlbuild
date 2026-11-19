"""Resume the maintained models whose holds expired: a location sync's only warehouse write."""

from __future__ import annotations

from typing import Any

from sqlbuild._native import MAINTAINED_PAUSES_TABLE_NAME
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.relations.types import RelationChangeTokenReader
from sqlbuild.executor.maintained_pauses._helpers.resume import resume_holds
from sqlbuild.executor.maintained_pauses._helpers.sql import read_open_holds
from sqlbuild.executor.maintained_pauses.models import OpenHold, ResumeOutcome
from sqlbuild.executor.maintained_pauses.types import StateLocation


def resume_expired_holds(
    *,
    adapter: BaseAdapter,
    connection: Any,
    location: StateLocation,
    grace_seconds: float,
    recorded_by: str,
    quiet_token: str | None,
) -> ResumeOutcome:
    """Close holds silent for `grace_seconds`, skipping a pause table still at `quiet_token`."""

    if not adapter.relation_exists(
        connection=connection,
        database=location[0],
        schema=location[1],
        name=MAINTAINED_PAUSES_TABLE_NAME,
    ):
        return ResumeOutcome(resumed=(), quiet_token=None)
    token: str | None = (
        adapter.read_relation_change_token(
            connection=connection,
            database=location[0],
            schema=location[1],
            name=MAINTAINED_PAUSES_TABLE_NAME,
        )
        if isinstance(adapter, RelationChangeTokenReader)
        else None
    )
    if token is not None and token == quiet_token:
        return ResumeOutcome(resumed=(), quiet_token=token)
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
    _ = resume_holds(
        adapter=adapter,
        connection=connection,
        location=location,
        holds=expired,
        recorded_by=recorded_by,
    )
    return ResumeOutcome(resumed=expired, quiet_token=None if holds else token)
