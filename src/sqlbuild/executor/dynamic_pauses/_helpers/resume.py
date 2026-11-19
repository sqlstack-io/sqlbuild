"""Resume dynamic models through the adapter, and close the holds that paused them."""

from __future__ import annotations

from typing import Any

from sqlbuild._native import CompiledRelationLocation
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.types import DynamicListing, DynamicOperation
from sqlbuild.executor.dynamic_pauses._helpers.sql import insert_sql
from sqlbuild.executor.dynamic_pauses.constants import SUSPENDED_STATE
from sqlbuild.executor.dynamic_pauses.models import DynamicPauseEvent, OpenHold
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
    for statement in adapter.render_dynamic_operation(
        destination=destination, operation=operation
    ):
        adapter.execute(connection=connection, sql=statement)


def resume_holds(
    *,
    adapter: BaseAdapter,
    connection: Any,
    location: StateLocation,
    holds: tuple[OpenHold, ...],
    recorded_by: str,
) -> None:
    """Resume each held model that is still suspended, then close its hold."""

    for hold in holds:
        if (
            scheduling_state(adapter=adapter, connection=connection, relation=hold.relation)
            == SUSPENDED_STATE
        ):
            run_dynamic_operation(
                adapter=adapter,
                connection=connection,
                relation=hold.relation,
                operation=DynamicOperation.RESUME,
            )
    if not holds:
        return
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
                for hold in holds
            ),
        ),
    )
