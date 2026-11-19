"""Resume the maintained models whose holds expired: a location sync's only warehouse write."""

from __future__ import annotations

from typing import Any

from sqlbuild._native import MAINTAINED_PAUSES_TABLE_NAME
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.executor.maintained_pauses._helpers.resume import resume_holds
from sqlbuild.executor.maintained_pauses._helpers.sql import read_open_holds
from sqlbuild.executor.maintained_pauses.models import OpenHold
from sqlbuild.executor.maintained_pauses.types import StateLocation


def resume_expired_holds(
    *,
    adapter: BaseAdapter,
    connection: Any,
    location: StateLocation,
    grace_seconds: float,
    recorded_by: str,
) -> tuple[OpenHold, ...]:
    """Close holds silent for `grace_seconds` on models no run holds live; return them."""

    if not adapter.relation_exists(
        connection=connection,
        database=location[0],
        schema=location[1],
        name=MAINTAINED_PAUSES_TABLE_NAME,
    ):
        return ()
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
    resume_holds(
        adapter=adapter,
        connection=connection,
        location=location,
        holds=expired,
        recorded_by=recorded_by,
    )
    return expired
