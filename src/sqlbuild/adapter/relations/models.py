"""Invocation-scoped warehouse inspection records."""

from __future__ import annotations

from dataclasses import dataclass

from sqlbuild.adapter.contract.models import ColumnInfo, RelationInfo


@dataclass(frozen=True)
class ListedRelation:
    """One relation from a schema listing with the exact name stored by the warehouse."""

    stored_name: str
    relation: RelationInfo


@dataclass(frozen=True)
class SchemaRelationListing:
    """Every relation one database schema exposes to the inspecting role."""

    database: str | None
    schema: str
    entries: tuple[ListedRelation, ...]


@dataclass(frozen=True)
class SchemaColumnListing:
    """Every column one database schema exposes, keyed by stored relation name."""

    database: str | None
    schema: str
    columns_by_stored_name: dict[str, tuple[ColumnInfo, ...]]
    complete: bool = True


@dataclass(frozen=True)
class InspectionQueryRecord:
    """One warehouse metadata or cursor-bound read issued while planning."""

    sql: str
    elapsed_seconds: float
    row_count: int | None
    error: str | None = None


@dataclass(frozen=True)
class StatementMetadataEffect:
    """What one executed statement may have changed in cached relation metadata."""

    relation_names: frozenset[str] = frozenset()
    invalidates_all: bool = False
    ends_transaction: bool = False


@dataclass(frozen=True)
class RunSchemaListing:
    """One schema's relations as a build listed them, and the cache versions at that moment."""

    relations: dict[str, RelationInfo]
    global_version: int
    name_versions: dict[str, int]
    expires_at: float | None = None
