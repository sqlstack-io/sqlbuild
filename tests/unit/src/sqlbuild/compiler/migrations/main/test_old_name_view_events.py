"""Unit coverage for append-only old-name view facts and their projection."""

from __future__ import annotations

from datetime import UTC, datetime, timedelta

import duckdb
import pytest

from sqlbuild._native import (
    MigrationEvent,
    MigrationStateInspection,
    OldNameViewEvent,
    OldNameViewEventType,
    OldNameViewHistory,
    live_reads,
    project_old_name_views,
    read_migration_state_events,
    stored_old_name_view_columns,
    write_migration_state_event,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from tests.unit.src.sqlbuild.compiler.migrations.main._test_types import (
    OldNameViewAbsentColumnsTestCase,
    OldNameViewProjectionTestCase,
    OldNameViewStorageTestCase,
)
from tests.unit.src.sqlbuild.compiler.migrations.main.helpers import (
    migration_event,
    old_name_fact,
)

_NOW: datetime = datetime(2026, 2, 1, tzinfo=UTC)


@pytest.mark.parametrize(
    "test_case",
    [
        OldNameViewStorageTestCase(
            description="writing the same fact twice stores it once",
            write_count=2,
            foreign_event_types=(),
            expected_event_types=("view_created",),
        ),
        OldNameViewStorageTestCase(
            description="facts of a type from a newer version are skipped",
            write_count=1,
            foreign_event_types=("view_renamed_by_a_future_version",),
            expected_event_types=("view_created",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_old_name_facts_when_writing_and_reading_then_rows_are_idempotent(
    test_case: OldNameViewStorageTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    move: MigrationEvent = migration_event(origin="revenue", destination="daily_revenue", day=1)
    fact: OldNameViewEvent = old_name_fact(
        move=move,
        event_type=OldNameViewEventType.VIEW_CREATED,
        expires_at=_NOW,
        column_aliases=(("amount", "revenue"),),
    )

    for _ in range(test_case.write_count):
        write_migration_state_event(
            kind="old_name",
            connection=connection,
            execute=adapter.execute,
            event=fact,
            render_qualified_name=adapter.render_qualified_name,
            create_table_sql=adapter.render_create_old_name_view_state_table_sql(
                database=None, schema="main"
            ),
        )
    for foreign in test_case.foreign_event_types:
        _ = connection.execute(
            "INSERT INTO main._sqlbuild_old_name_views (event_id, event_type, "
            "migration_event_id, destination_model, old_name, new_name, run_id, created_at) "
            f"VALUES ('future', '{foreign}', 'm', 'daily_revenue', 'revenue', 'daily_revenue', "
            "'run-9', TIMESTAMP '2026-01-09')"
        )
    stored: tuple[OldNameViewEvent, ...] = read_migration_state_events(
        kind="old_name",
        connection=connection,
        execute=adapter.execute,
        database=None,
        schema="main",
        stored_columns=stored_old_name_view_columns(
            adapter=adapter, connection=connection, database=None, schema="main"
        ),
        render_qualified_name=adapter.render_qualified_name,
    )

    assert tuple(event.event_type.value for event in stored) == test_case.expected_event_types
    assert stored == (fact,)


@pytest.mark.parametrize(
    "test_case",
    [
        OldNameViewProjectionTestCase(
            description="move recorded before old-name views existed has no history",
            recorded_types=(),
            recorded_moves=1,
            expires_in_days=1,
            expected_statuses=(),
        ),
        OldNameViewProjectionTestCase(
            description="requirement without its move is ignored",
            recorded_types=("required",),
            recorded_moves=0,
            expires_in_days=1,
            expected_statuses=(),
        ),
        OldNameViewProjectionTestCase(
            description="required move waits for its archive",
            recorded_types=("required",),
            recorded_moves=1,
            expires_in_days=1,
            expected_statuses=("pending_archive",),
        ),
        OldNameViewProjectionTestCase(
            description="archived move waits for its view",
            recorded_types=("required", "origin_archived"),
            recorded_moves=1,
            expires_in_days=1,
            expected_statuses=("pending_view",),
        ),
        OldNameViewProjectionTestCase(
            description="created view is live until it expires",
            recorded_types=("required", "origin_archived", "view_created"),
            recorded_moves=1,
            expires_in_days=1,
            expected_statuses=("live",),
        ),
        OldNameViewProjectionTestCase(
            description="created view past its expiry is expired",
            recorded_types=("required", "origin_archived", "view_created"),
            recorded_moves=1,
            expires_in_days=-1,
            expected_statuses=("expired",),
        ),
        OldNameViewProjectionTestCase(
            description="dropped view is dropped",
            recorded_types=("required", "origin_archived", "view_created", "view_dropped"),
            recorded_moves=1,
            expires_in_days=-1,
            expected_statuses=("dropped",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_recorded_facts_when_projecting_then_status_follows_the_newest_step(
    test_case: OldNameViewProjectionTestCase,
) -> None:
    move: MigrationEvent = migration_event(origin="revenue", destination="daily_revenue", day=1)
    facts: tuple[OldNameViewEvent, ...] = tuple(
        old_name_fact(
            move=move,
            event_type=OldNameViewEventType(event_type),
            expires_at=_NOW + timedelta(days=test_case.expires_in_days),
        )
        for event_type in test_case.recorded_types
    )

    histories: tuple[OldNameViewHistory, ...] = project_old_name_views(
        moves=(move,)[: test_case.recorded_moves], facts=facts
    )

    assert tuple(history.status(now=_NOW).value for history in histories) == (
        test_case.expected_statuses
    )


@pytest.mark.parametrize(
    "test_case",
    [
        OldNameViewAbsentColumnsTestCase(
            description="table without grants_copied reads grants as unknown",
            absent_columns=("grants_copied",),
            expected_statuses=("live",),
            expected_archive_name="_sqb_archive__revenue",
        ),
        OldNameViewAbsentColumnsTestCase(
            description="table without grants_copied and archive_name reads both as null",
            absent_columns=("grants_copied", "archive_name"),
            expected_statuses=("live",),
            expected_archive_name=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_state_table_without_optional_columns_when_planning_then_absent_columns_read_null(
    test_case: OldNameViewAbsentColumnsTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    move: MigrationEvent = migration_event(origin="revenue", destination="daily_revenue", day=1)
    write_migration_state_event(
        kind="model",
        connection=connection,
        execute=adapter.execute,
        event=move,
        render_qualified_name=adapter.render_qualified_name,
        create_table_sql=adapter.render_create_migration_state_table_sql(
            database=None, schema="main"
        ),
    )
    event_type: OldNameViewEventType
    for event_type in (
        OldNameViewEventType.REQUIRED,
        OldNameViewEventType.ORIGIN_ARCHIVED,
        OldNameViewEventType.VIEW_CREATED,
    ):
        write_migration_state_event(
            kind="old_name",
            connection=connection,
            execute=adapter.execute,
            event=old_name_fact(
                move=move,
                event_type=event_type,
                expires_at=_NOW + timedelta(days=30),
                archive_name="_sqb_archive__revenue",
                grants_copied=("GRANT SELECT ON main.revenue TO reporting",),
            ),
            render_qualified_name=adapter.render_qualified_name,
            create_table_sql=adapter.render_create_old_name_view_state_table_sql(
                database=None, schema="main"
            ),
        )
    column: str
    for column in test_case.absent_columns:
        _ = connection.execute(f"ALTER TABLE main._sqlbuild_old_name_views DROP COLUMN {column}")
    inspection: MigrationStateInspection = MigrationStateInspection(
        reads=live_reads(adapter=adapter, connection=connection), database=None
    )

    inspection.inspect_old_name_views(schemas={"main"})

    histories: tuple[OldNameViewHistory, ...] = inspection.old_name_histories
    assert tuple(history.status(now=_NOW).value for history in histories) == (
        test_case.expected_statuses
    )
    created: OldNameViewEvent | None = histories[0].created
    archived: OldNameViewEvent | None = histories[0].archived
    assert created is not None
    assert created.grants_copied is None
    assert archived is not None
    assert archived.archive_name == test_case.expected_archive_name
