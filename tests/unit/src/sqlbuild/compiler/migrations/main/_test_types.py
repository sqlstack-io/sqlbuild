"""Test case types for model migration state entrypoints."""

from __future__ import annotations

from dataclasses import dataclass

from sqlbuild._native import MigrationRelation, OldNameViewDropReason, OldNameViewEventType


@dataclass(frozen=True)
class NewestEventTestCase:
    description: str
    events: tuple[tuple[str, str], ...]
    relation: str
    expected_newest_index: int | None


@dataclass(frozen=True)
class RepeatedEventWriteTestCase:
    description: str
    write_count: int
    expected_stored_count: int


@dataclass(frozen=True)
class ColumnEventStorageTestCase:
    description: str
    write_count: int
    foreign_decisions: tuple[str, ...]
    expected_stored_count: int


@dataclass(frozen=True)
class NewestColumnEventTestCase:
    description: str
    renames: tuple[tuple[str, str], ...]
    origin_column: str
    destination_column: str
    expected_newest_index: int | None


@dataclass(frozen=True)
class OldNameViewStorageTestCase:
    description: str
    write_count: int
    foreign_event_types: tuple[str, ...]
    expected_event_types: tuple[str, ...]


@dataclass(frozen=True)
class OldNameViewProjectionTestCase:
    description: str
    recorded_types: tuple[str, ...]
    recorded_moves: int
    expires_in_days: int
    expected_statuses: tuple[str, ...]


@dataclass(frozen=True)
class OldNameViewAbsentColumnsTestCase:
    description: str
    absent_columns: tuple[str, ...]
    expected_statuses: tuple[str, ...]
    expected_archive_name: str | None


@dataclass(frozen=True)
class PinnedMoveEventIdTestCase:
    description: str
    run_id: str
    target_name: str | None
    origin: MigrationRelation
    destination: MigrationRelation
    expected_event_id: str


@dataclass(frozen=True)
class PinnedFreshnessHashTestCase:
    description: str
    source_name: str
    strategy: str
    value_kind: str
    data_version: str
    expected_hash: str


@dataclass(frozen=True)
class PinnedColumnEventIdTestCase:
    description: str
    relation: MigrationRelation
    origin_column: str
    destination_column: str
    expected_event_id: str


@dataclass(frozen=True)
class PinnedOldNameEventIdTestCase:
    description: str
    event_type: OldNameViewEventType
    migration_event_id: str
    expected_event_id: str


@dataclass(frozen=True)
class StoredOldNameRowTestCase:
    description: str
    column_aliases_sql: str
    grants_copied_sql: str
    drop_reason_sql: str
    expected_aliases: tuple[tuple[str, str], ...]
    expected_grants: tuple[str, ...] | None
    expected_drop_reason: OldNameViewDropReason | None


@dataclass(frozen=True)
class StoredColumnRowTestCase:
    description: str
    decisions: tuple[str, ...]
    expected_decisions: tuple[str, ...]


@dataclass(frozen=True)
class NamedFingerprintTestCase:
    description: str
    model_name: str
    """The `schema.name` the newest fingerprint built, when there is one."""
    expected_targets: tuple[str, ...]
