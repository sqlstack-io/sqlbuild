"""Invocation-scoped relation existence and column metadata shared by build workers."""

from __future__ import annotations

import threading
import time
from collections.abc import Callable
from typing import cast

from sqlbuild._native import (
    qualified_relation_name_key,
    relation_name_key,
    statement_metadata_effect,
)
from sqlbuild.adapter.contract.models import RelationInfo
from sqlbuild.adapter.relations.constants import (
    SCHEMA_LISTING_LOOKUP_THRESHOLD,
    SCHEMA_LISTING_MAX_AGE_SECONDS,
)
from sqlbuild.adapter.relations.models import RunSchemaListing, StatementMetadataEffect
from sqlbuild.adapter.relations.types import (
    RelationCacheKey,
    RelationCacheVersion,
    SchemaListingScope,
)


class RelationMetadataCache:
    """Serve repeated identical metadata lookups until SQL may have changed the relation."""

    def __init__(
        self, *, transactional_ddl: bool, clock: Callable[[], float] = time.monotonic
    ) -> None:
        self._transactional_ddl: bool = transactional_ddl
        self._clock: Callable[[], float] = clock
        self._lock: threading.Lock = threading.Lock()
        self._entries: dict[RelationCacheKey, object] = {}
        self._name_versions: dict[str, int] = {}
        self._global_version: int = 0
        self._pending_names: dict[int, set[str]] = {}
        self._pending_all: set[int] = set()
        self._listings: dict[SchemaListingScope, RunSchemaListing] = {}
        self._schema_lookups: dict[SchemaListingScope, int] = {}
        self._listing_locks: dict[SchemaListingScope, threading.Lock] = {}

    def lookup[ValueT](
        self,
        *,
        kind: str,
        database: str | None,
        schema: str | None,
        name: str,
        read: Callable[[], ValueT],
    ) -> ValueT:
        """Return the cached answer for this exact lookup, reading it on a miss."""

        return cast(ValueT, self._lookup(key=(kind, database, schema, name), read=read))

    def remember(
        self, *, kind: str, database: str | None, schema: str | None, name: str, value: object
    ) -> None:
        """Record an answer this run has just established, such as a relation it dropped."""

        with self._lock:
            self._entries[(kind, database, schema, name)] = value

    def schema_relation(
        self,
        *,
        scope: SchemaListingScope,
        name: str,
        stored_name: str,
        read_listing: Callable[[], dict[str, RelationInfo]],
    ) -> tuple[bool, RelationInfo | None]:
        """Answer ``(answered, relation)`` from the schema listing made at its second lookup."""

        listing: RunSchemaListing | None
        with self._lock:
            listing = self._listings.get(scope)
            expired: bool = listing is not None and self._expired(listing=listing)
            if expired:
                del self._listings[scope]
                listing = None
            if listing is None and not expired:
                lookups: int = self._schema_lookups.get(scope, 0) + 1
                self._schema_lookups[scope] = lookups
                if lookups < SCHEMA_LISTING_LOOKUP_THRESHOLD:
                    return False, None
            listing_lock: threading.Lock = self._listing_locks.setdefault(scope, threading.Lock())
        if listing is None:
            with listing_lock:
                with self._lock:
                    listing = self._listings.get(scope)
                if listing is None:
                    listing = self.refresh_schema_listing(scope=scope, read_listing=read_listing)
        with self._lock:
            if not self._listed_fresh(listing=listing, name=name):
                return False, None
        return True, listing.relations.get(stored_name)

    def refresh_schema_listing(
        self, *, scope: SchemaListingScope, read_listing: Callable[[], dict[str, RelationInfo]]
    ) -> RunSchemaListing:
        """List the schema again; every relation in it is current as of this listing, for a time."""

        versions: tuple[int, dict[str, int]] = self.listing_versions()
        return self.keep_schema_listing(
            scope=scope,
            relations=read_listing(),
            versions=versions,
            max_age_seconds=SCHEMA_LISTING_MAX_AGE_SECONDS,
        )

    def listing_versions(self) -> tuple[int, dict[str, int]]:
        """The cache versions a listing read from now on is current as of."""

        with self._lock:
            return self._global_version, dict(self._name_versions)

    def keep_schema_listing(
        self,
        *,
        scope: SchemaListingScope,
        relations: dict[str, RelationInfo],
        versions: tuple[int, dict[str, int]],
        max_age_seconds: float | None = None,
    ) -> RunSchemaListing:
        """Keep a listing read after ``listing_versions`` returned ``versions``, for a time."""

        global_version: int = versions[0]
        expires_at: float | None = (
            None if max_age_seconds is None else self._clock() + max_age_seconds
        )
        listing: RunSchemaListing = RunSchemaListing(
            relations=relations,
            global_version=global_version,
            name_versions=versions[1],
            expires_at=expires_at,
        )
        with self._lock:
            if self._global_version == global_version:
                self._listings[scope] = listing
        return listing

    def listed_relation(
        self, *, scope: SchemaListingScope, name: str, stored_name: str
    ) -> tuple[bool, RelationInfo | None]:
        """Answer from an existing listing of the schema only, without listing it."""

        with self._lock:
            listing: RunSchemaListing | None = self._listings.get(scope)
            if listing is None or not self._listed_fresh(listing=listing, name=name):
                return False, None
        return True, listing.relations.get(stored_name)

    def _listed_fresh(self, *, listing: RunSchemaListing, name: str) -> bool:
        name_key: str = relation_name_key(name)
        return (
            not self._expired(listing=listing)
            and listing.global_version == self._global_version
            and listing.name_versions.get(name_key, 0) == self._name_versions.get(name_key, 0)
        )

    def _expired(self, *, listing: RunSchemaListing) -> bool:
        return listing.expires_at is not None and self._clock() >= listing.expires_at

    def invalidate_relations(self, *, qualified_names: tuple[str, ...]) -> None:
        """Evict relations changed outside SQL statements, such as by a warehouse copy job."""

        keys: tuple[str | None, ...] = tuple(map(qualified_relation_name_key, qualified_names))
        names: frozenset[str] = frozenset(key for key in keys if key is not None)
        with self._lock:
            if None in keys:
                self._invalidate_all()
                return
            self._invalidate_names(names=names)

    def observe_statement(self, *, sql: str) -> None:
        """Invalidate entries the finished statement ``sql`` may have changed."""

        effect: StatementMetadataEffect = statement_metadata_effect(sql)
        thread_id: int = threading.get_ident()
        with self._lock:
            if effect.ends_transaction and self._transactional_ddl:
                names: set[str] = self._pending_names.pop(thread_id, set())
                if thread_id in self._pending_all:
                    self._pending_all.discard(thread_id)
                    self._invalidate_all()
                else:
                    self._invalidate_names(names=frozenset(names))
                return
            if effect.invalidates_all:
                if self._transactional_ddl:
                    self._pending_all.add(thread_id)
                self._invalidate_all()
                return
            if effect.relation_names:
                if self._transactional_ddl:
                    self._pending_names.setdefault(thread_id, set()).update(effect.relation_names)
                self._invalidate_names(names=effect.relation_names)

    def _lookup(self, *, key: RelationCacheKey, read: Callable[[], object]) -> object:
        name_key: str = relation_name_key(key[3])
        with self._lock:
            if key in self._entries:
                return self._entries[key]
            version: RelationCacheVersion = self._version(name_key=name_key)
        value: object = read()
        with self._lock:
            if self._version(name_key=name_key) == version:
                self._entries[key] = value
        return value

    def _version(self, *, name_key: str) -> RelationCacheVersion:
        return self._global_version, self._name_versions.get(name_key, 0)

    def _invalidate_names(self, *, names: frozenset[str]) -> None:
        if not names:
            return
        name: str
        for name in names:
            self._name_versions[name] = self._name_versions.get(name, 0) + 1
        stale: list[RelationCacheKey] = [
            key for key in self._entries if relation_name_key(key[3]) in names
        ]
        key: RelationCacheKey
        for key in stale:
            del self._entries[key]

    def _invalidate_all(self) -> None:
        self._global_version += 1
        self._entries.clear()
        self._listings.clear()
