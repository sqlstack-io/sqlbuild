"""Portable SQL for the append-only dynamic pause table, timed by the warehouse clock."""

from __future__ import annotations

import hashlib
import json
from datetime import datetime
from typing import Any

from sqlbuild._native import (
    DYNAMIC_PAUSES_TABLE_NAME,
    CompiledRelationLocation,
    render_state_sql_literal,
    render_state_table_create_sql,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.dynamic_pauses.constants import (
    COLUMN_EVENT_ID,
    COLUMN_EVENT_TYPE,
    COLUMN_RECORDED_AT,
    COLUMN_RECORDED_BY,
    COLUMN_RELATION_DATABASE,
    COLUMN_RELATION_NAME,
    COLUMN_RELATION_SCHEMA,
    COLUMN_RUN_ID,
    COLUMN_SCHEMA_VERSION,
    DYNAMIC_PAUSE_COLUMN_TYPES,
    DYNAMIC_PAUSE_COLUMNS,
    DYNAMIC_PAUSE_REQUIRED_COLUMNS,
    DYNAMIC_PAUSE_SCHEMA_VERSION,
)
from sqlbuild.executor.dynamic_pauses.models import DynamicPauseEvent, OpenHold
from sqlbuild.executor.dynamic_pauses.types import DynamicPauseEventType, StateLocation


def pause_table_name(*, adapter: BaseAdapter, location: StateLocation) -> str:
    qualified: str | None = adapter.render_qualified_name(
        database=location[0], schema=location[1], name=DYNAMIC_PAUSES_TABLE_NAME
    )
    if qualified is None:
        raise ExecutorInputError("the dynamic pause table needs a schema")
    return qualified


def setup_statements(*, adapter: BaseAdapter, location: StateLocation) -> tuple[str, ...]:
    """Create the schema and the pause table when they are missing."""

    return (
        *adapter.render_create_schema(database=location[0], schema=location[1]),
        render_state_table_create_sql(
            qualified_name=pause_table_name(adapter=adapter, location=location),
            columns=DYNAMIC_PAUSE_COLUMNS,
            column_types=DYNAMIC_PAUSE_COLUMN_TYPES,
            required_columns=DYNAMIC_PAUSE_REQUIRED_COLUMNS,
            render_framework_type=adapter.render_framework_type,
        ),
    )


def insert_sql(
    *, adapter: BaseAdapter, location: StateLocation, events: tuple[DynamicPauseEvent, ...]
) -> str:
    """Append every event in one statement, so a hand-over closes and opens holds together."""

    now: str = adapter.render_current_utc_timestamp()
    rows: str = " UNION ALL ".join(
        f"SELECT {_row_literals(event=event)}, {now}" for event in events
    )
    return (
        f"INSERT INTO {pause_table_name(adapter=adapter, location=location)} "
        f"({', '.join(DYNAMIC_PAUSE_COLUMNS)}) {rows}"
    )


def open_holds_sql(*, adapter: BaseAdapter, location: StateLocation) -> str:
    """Paused models nobody resumed, with when their run last wrote and the warehouse's now."""

    table: str = pause_table_name(adapter=adapter, location=location)
    now: str = adapter.render_current_utc_timestamp()
    same_relation: str = " AND ".join(
        f"COALESCE(r.{column}, '') = COALESCE(p.{column}, '')"
        for column in (COLUMN_RELATION_DATABASE, COLUMN_RELATION_SCHEMA, COLUMN_RELATION_NAME)
    )
    return (
        f"SELECT p.{COLUMN_RUN_ID}, p.{COLUMN_RELATION_DATABASE}, p.{COLUMN_RELATION_SCHEMA}, "
        f"p.{COLUMN_RELATION_NAME}, CAST(h.last_seen AS TIMESTAMP), CAST({now} AS TIMESTAMP), "
        f"CAST(p.{COLUMN_RECORDED_AT} AS TIMESTAMP) "
        f"FROM {table} p JOIN (SELECT {COLUMN_RUN_ID}, MAX({COLUMN_RECORDED_AT}) AS last_seen "
        f"FROM {table} WHERE {COLUMN_RECORDED_BY} = {COLUMN_RUN_ID} GROUP BY {COLUMN_RUN_ID}) h "
        f"ON h.{COLUMN_RUN_ID} = p.{COLUMN_RUN_ID} "
        f"WHERE p.{COLUMN_EVENT_TYPE} = '{DynamicPauseEventType.PAUSED.value}' "
        f"AND NOT EXISTS (SELECT 1 FROM {table} r "
        f"WHERE r.{COLUMN_EVENT_TYPE} = '{DynamicPauseEventType.RESUMED.value}' "
        f"AND r.{COLUMN_RUN_ID} = p.{COLUMN_RUN_ID} AND {same_relation})"
    )


def open_holds(*, rows: tuple[tuple[Any, ...], ...]) -> tuple[OpenHold, ...]:
    """The open holds of `open_holds_sql` rows."""

    return tuple(
        OpenHold(
            run_id=str(row[0]),
            relation=CompiledRelationLocation(
                database=_optional(row[1]),
                schema=_optional(row[2]),
                name=str(row[3]),
                qualified_name=None,
            ),
            idle_seconds=_seconds_between(start=row[4], end=row[5]),
            paused_at=row[6].replace(tzinfo=None),
        )
        for row in rows
    )


def _seconds_between(*, start: datetime, end: datetime) -> float:
    return (end.replace(tzinfo=None) - start.replace(tzinfo=None)).total_seconds()


def _optional(value: object) -> str | None:
    return None if value is None else str(value)


def _row_literals(*, event: DynamicPauseEvent) -> str:
    relation: CompiledRelationLocation | None = event.relation
    values: dict[str, object | None] = {
        COLUMN_EVENT_ID: _event_id(event=event),
        COLUMN_SCHEMA_VERSION: DYNAMIC_PAUSE_SCHEMA_VERSION,
        COLUMN_EVENT_TYPE: event.event_type.value,
        COLUMN_RUN_ID: event.run_id,
        COLUMN_RELATION_DATABASE: None if relation is None else relation.database,
        COLUMN_RELATION_SCHEMA: None if relation is None else relation.schema,
        COLUMN_RELATION_NAME: None if relation is None else relation.name,
        COLUMN_RECORDED_BY: event.recorded_by,
    }
    return ", ".join(
        render_state_sql_literal(
            value=values[column], declared_type=DYNAMIC_PAUSE_COLUMN_TYPES[column]
        )
        for column in DYNAMIC_PAUSE_COLUMNS
        if column != COLUMN_RECORDED_AT
    )


def _event_id(*, event: DynamicPauseEvent) -> str:
    """Content-addressed: one pause and one resume per run and model, one ID per heartbeat."""

    relation: CompiledRelationLocation | None = event.relation
    identity: tuple[object, ...] = (
        event.event_type.value,
        event.run_id,
        None if relation is None else (relation.database, relation.schema, relation.name),
        event.sequence,
    )
    encoded: str = json.dumps(identity, separators=(",", ":"), ensure_ascii=True)
    return hashlib.sha256(encoded.encode("utf-8")).hexdigest()
