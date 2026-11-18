"""Event builders for model migration state unit tests."""

from __future__ import annotations

from datetime import UTC, datetime, timedelta

from sqlbuild._native import (
    ColumnMigrationDecision,
    ColumnMigrationEvent,
    Fingerprint,
    MigrationDecision,
    MigrationDiscovery,
    MigrationEvent,
    MigrationRelation,
    OldNameViewEvent,
    OldNameViewEventType,
    deterministic_column_migration_event_id,
    deterministic_migration_event_id,
    deterministic_old_name_view_event_id,
)

_STARTED_AT: datetime = datetime(2026, 1, 1, tzinfo=UTC)


def main_relation(name: str) -> MigrationRelation:
    """Return a relation in the main schema with no database qualifier."""

    return MigrationRelation(database=None, schema="main", name=name)


def migration_event(*, origin: str, destination: str, day: int) -> MigrationEvent:
    """Return a manual migrate event recorded on the given day."""

    return MigrationEvent(
        event_id=deterministic_migration_event_id(
            run_id=f"run-{day}",
            target_name="dev",
            origin=main_relation(origin),
            destination=main_relation(destination),
        ),
        target_name="dev",
        origin_model=origin,
        origin=main_relation(origin),
        destination_model=destination,
        destination=main_relation(destination),
        origin_version_hash=f"hash-{origin}",
        discovery=MigrationDiscovery.MANUAL,
        decision=MigrationDecision.MIGRATE,
        run_id=f"run-{day}",
        created_at=_STARTED_AT + timedelta(days=day),
    )


def column_migration_event(*, origin: str, destination: str, day: int) -> ColumnMigrationEvent:
    """Return a manual column rename on main.fct_orders recorded on the given day."""

    relation: MigrationRelation = main_relation("fct_orders")
    return ColumnMigrationEvent(
        event_id=deterministic_column_migration_event_id(
            run_id=f"run-{day}",
            target_name="dev",
            relation=relation,
            origin_column=origin,
            destination_column=destination,
        ),
        target_name="dev",
        model_name="fct_orders",
        relation=relation,
        origin_column=origin,
        destination_column=destination,
        discovery=MigrationDiscovery.MANUAL,
        decision=ColumnMigrationDecision.RENAME,
        run_id=f"run-{day}",
        created_at=_STARTED_AT + timedelta(days=day),
    )


def old_name_fact(
    *,
    move: MigrationEvent,
    event_type: OldNameViewEventType,
    expires_at: datetime | None = None,
    column_aliases: tuple[tuple[str, str], ...] = (),
    archive_name: str | None = None,
    grants_copied: tuple[str, ...] | None = None,
) -> OldNameViewEvent:
    """Return one old-name fact of a move from main.revenue to main.daily_revenue."""

    return OldNameViewEvent(
        event_id=deterministic_old_name_view_event_id(
            event_type=event_type, migration_event_id=move.event_id
        ),
        target_name="dev",
        event_type=event_type,
        migration_event_id=move.event_id,
        destination_model=move.destination_model,
        old=main_relation("revenue"),
        new=main_relation("daily_revenue"),
        run_id=move.run_id,
        created_at=move.created_at,
        view_retention="30d",
        column_aliases=column_aliases,
        expires_at=expires_at,
        archive_name=archive_name,
        grants_copied=grants_copied,
    )


def built_fingerprint(node_type: str, node_name: str, schema: str, day: int) -> Fingerprint:
    """Return a fingerprint of `node_name` built into `schema` at noon on January `day`, 2026."""

    return Fingerprint(
        node_type=node_type,
        node_name=node_name,
        target_database=None,
        target_schema=schema,
        target_name=node_name,
        run_id=f"run_{day:03d}",
        definition_hash="definition_hash",
        version_hash="version_hash",
        schema_fingerprint="schema_hash",
        definition="SELECT 1",
        metadata_json="{}",
        ts=datetime(2026, 1, day, 12, 0, 0),
    )
