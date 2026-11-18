"""A target's mirrored catalog, which a plan uses once the warehouse confirms it is current."""

from __future__ import annotations

import sys
import threading
from datetime import datetime

from sqlbuild.adapter.contract.models import ColumnInfo
from sqlbuild.adapter.relations.constants import SYNCED_DDL_CLOCK_SKEW
from sqlbuild.adapter.relations.models import (
    ListedRelation,
    RelationChange,
    SchemaRelationListing,
    SeededSchema,
    SyncedRelation,
)
from sqlbuild.adapter.relations.types import SyncedRow


class SyncedCatalog:
    """The mirror of one target's catalog, by lowercased database and schema."""

    def __init__(self, *, relations: tuple[SyncedRelation, ...]) -> None:
        self._schemas: dict[tuple[str | None, str], dict[str, list[SyncedRelation]]] = {}
        relation: SyncedRelation
        for relation in relations:
            self._schemas.setdefault(
                _scope_key(database=relation.database, schema=relation.schema), {}
            ).setdefault(relation.name.lower(), []).append(relation)
        self._lock: threading.Lock = threading.Lock()
        self._checked: list[tuple[float, int, int]] = []
        self._failure: str | None = None

    @classmethod
    def from_rows(cls, *, rows: list[SyncedRow]) -> SyncedCatalog:
        """Each mirrored `(database, schema, name, columns, observed_at)` row as one catalog."""

        return cls(relations=tuple(_synced_relation(row=row) for row in rows))

    def covers(self, *, database: str | None, schema: str) -> bool:
        """Whether the mirror holds any relation of this schema."""

        return _scope_key(database=database, schema=schema) in self._schemas

    def seed(
        self, *, database: str | None, schema: str, changes: tuple[RelationChange, ...]
    ) -> SeededSchema:
        """The schema as the check lists it, with mirrored columns where no DDL followed them."""

        mirrored: dict[str, list[SyncedRelation]] = self._schemas.get(
            _scope_key(database=database, schema=schema), {}
        )
        entries: list[ListedRelation] = []
        columns: dict[str, tuple[ColumnInfo, ...]] = {}
        stale: list[ListedRelation] = []
        names: dict[str, int] = {}
        change: RelationChange
        for change in changes:
            names[change.listed.stored_name.lower()] = (
                names.get(change.listed.stored_name.lower(), 0) + 1
            )
        for change in changes:
            entries.append(change.listed)
            candidates: list[SyncedRelation] = mirrored.get(change.listed.stored_name.lower(), [])
            current: bool = (
                len(candidates) == 1
                and names[change.listed.stored_name.lower()] == 1
                and change.last_ddl is not None
                and change.last_ddl <= candidates[0].observed_at - SYNCED_DDL_CLOCK_SKEW
            )
            if current:
                columns[change.listed.stored_name] = candidates[0].columns
            else:
                stale.append(change.listed)
        return SeededSchema(
            relations=SchemaRelationListing(
                database=database, schema=schema, entries=tuple(entries)
            ),
            columns=columns,
            stale=tuple(stale),
        )

    def record_check(self, *, seconds: float, relations: int, refreshed: int) -> None:
        """Note one check: how long it took, the relations it listed and those re-read."""

        with self._lock:
            self._checked.append((seconds, relations, refreshed))

    def record_failure(self, *, reason: str) -> None:
        """Note that the warehouse could not confirm the mirror, so the plan reads it live."""

        with self._lock:
            self._failure = self._failure or reason
        print(f"Planning live: {reason}", file=sys.stderr, flush=True)

    def report(self) -> str | None:
        """One line on what the checks found, or None when nothing was checked."""

        with self._lock:
            if not self._checked:
                return None
            seconds: float = sum(check[0] for check in self._checked)
            refreshed: int = sum(check[2] for check in self._checked)
        noun: str = "table" if refreshed == 1 else "tables"
        return f"Planned from synced state: checked in {seconds:.1f}s, {refreshed} {noun} refreshed"


def _scope_key(*, database: str | None, schema: str) -> tuple[str | None, str]:
    return (None if database is None else database.lower(), schema.lower())


def _synced_relation(*, row: SyncedRow) -> SyncedRelation:
    database, schema, name, columns, observed_at = row
    return SyncedRelation(
        database=database,
        schema=schema,
        name=name,
        columns=tuple(ColumnInfo(name=column, type=kind) for column, kind in columns),
        observed_at=datetime.fromisoformat(observed_at),
    )
