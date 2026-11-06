"""Rename one model's columns in place and append their column migration events."""

from __future__ import annotations

from datetime import UTC, datetime
from typing import Any

from sqlbuild._native import (
    MIGRATION_WRITE_ATTEMPTS,
    ColumnMigrationDecision,
    ColumnMigrationEvent,
    ColumnMigrationPlanEntry,
    MigrationRelation,
    deterministic_column_migration_event_id,
    write_migration_state_event,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.main.column_name_key import column_name_key
from sqlbuild.adapter.contract.models import ColumnInfo
from sqlbuild.executor.migrations._helpers.relation_for_location import (
    migration_relation_for_location,
)


def apply_model_column_renames(
    *,
    adapter: BaseAdapter,
    connection: Any,
    entries: tuple[ColumnMigrationPlanEntry, ...],
    run_id: str,
) -> None:
    """Rename and record every pending column of one model, re-checking live columns per rename."""

    first: ColumnMigrationPlanEntry = entries[0]
    adapter.ensure_schema(
        connection=connection,
        database=first.destination.database,
        schema=first.destination.schema,
        statement_recorder=StatementRecorder(),
    )
    create_table_sql: str = adapter.render_create_column_migration_state_table_sql(
        database=first.destination.database, schema=first.destination.schema or ""
    )
    if not adapter.supports_transactional_ddl():
        entry: ColumnMigrationPlanEntry
        for entry in entries:
            _rename_and_record(
                adapter=adapter,
                connection=connection,
                entry=entry,
                run_id=run_id,
                create_table_sql=create_table_sql,
            )
        return
    _ = adapter.execute(connection=connection, sql=create_table_sql)
    with adapter.transaction(connection):
        for entry in entries:
            _rename_and_record(
                adapter=adapter,
                connection=connection,
                entry=entry,
                run_id=run_id,
                create_table_sql=None,
                attempts=1,
            )


def _rename_and_record(
    *,
    adapter: BaseAdapter,
    connection: Any,
    entry: ColumnMigrationPlanEntry,
    run_id: str,
    create_table_sql: str | None,
    attempts: int = MIGRATION_WRITE_ATTEMPTS,
) -> None:
    live: frozenset[str] = frozenset(
        column_name_key(column.name)
        for column in _live_columns(adapter=adapter, connection=connection, entry=entry)
    )
    already_renamed: bool = (
        column_name_key(entry.origin_column) not in live
        and column_name_key(entry.destination_column) in live
    )
    decision: ColumnMigrationDecision = entry.decision
    if entry.decision.renames and not already_renamed:
        adapter.rename_column(
            connection=connection,
            destination=entry.destination.qualified_name or entry.destination.name,
            old_name=entry.origin_column,
            new_name=entry.destination_column,
            statement_recorder=StatementRecorder(),
        )
    elif entry.decision.renames:
        decision = ColumnMigrationDecision.RECORD
    event: ColumnMigrationEvent = column_migration_event(
        entry=entry, decision=decision, run_id=run_id
    )
    write_migration_state_event(
        kind="column",
        connection=connection,
        execute=adapter.execute,
        event=event,
        render_qualified_name=adapter.render_qualified_name,
        create_table_sql=create_table_sql,
        attempts=attempts,
    )


def _live_columns(
    *, adapter: BaseAdapter, connection: Any, entry: ColumnMigrationPlanEntry
) -> tuple[ColumnInfo, ...]:
    return adapter.get_columns(
        connection=connection,
        database=entry.destination.database,
        schema=entry.destination.schema,
        name=entry.destination.name,
    )


def column_migration_event(
    *, entry: ColumnMigrationPlanEntry, decision: ColumnMigrationDecision, run_id: str
) -> ColumnMigrationEvent:
    """Build the append-only event recording one completed column rename."""

    relation: MigrationRelation = migration_relation_for_location(entry.destination)
    return ColumnMigrationEvent(
        event_id=deterministic_column_migration_event_id(
            run_id=run_id,
            target_name=entry.target_name,
            relation=relation,
            origin_column=entry.origin_column,
            destination_column=entry.destination_column,
        ),
        target_name=entry.target_name,
        model_name=entry.model_name,
        relation=relation,
        origin_column=entry.origin_column,
        destination_column=entry.destination_column,
        discovery=entry.discovery,
        decision=decision,
        run_id=run_id,
        created_at=datetime.now(tz=UTC),
    )
