"""Invocation-scoped warehouse metadata shared by every planning inspection phase."""

from __future__ import annotations

import threading
import time
from collections.abc import Callable
from typing import Any, cast

from sqlbuild.adapter.contract.models import ColumnInfo, RelationInfo
from sqlbuild.adapter.relations._helpers.inspection_context import emit_inspection_record
from sqlbuild.adapter.relations._helpers.relation_metadata_context import (
    current_relation_metadata_cache,
)
from sqlbuild.adapter.relations.classes.relation_metadata_cache import RelationMetadataCache
from sqlbuild.adapter.relations.constants import (
    INSPECTION_DESCRIBED_NAME_LIMIT,
    INSPECTION_IN_LIST_LIMIT,
    SCHEMA_LISTING_MAX_AGE_SECONDS,
)
from sqlbuild.adapter.relations.main.run_bounded_inspections import run_bounded_inspections
from sqlbuild.adapter.relations.models import (
    InspectionQueryRecord,
    SchemaColumnListing,
    SchemaRelationListing,
)
from sqlbuild.adapter.relations.types import (
    RelationListingScoper,
    SchemaScopedMetadataReader,
    ViewDefinitionBatchReader,
)

type _RelationIdentity = tuple[str | None, str | None, str]
type _SchemaKey = tuple[str | None, str]
type _RelationRequestKey = tuple[str | None, tuple[str, ...] | None, tuple[str, ...] | None]


class InspectionCatalog:
    """Read each schema's metadata at most once per planning invocation, filtering client-side."""

    def __init__(self, *, adapter: Any, connection: Any) -> None:
        self.adapter: Any = adapter
        self.connection: Any = connection
        self._reader: SchemaScopedMetadataReader | None = (
            adapter if isinstance(adapter, SchemaScopedMetadataReader) else None
        )
        self._concurrency: int = max(1, int(getattr(adapter, "metadata_inspection_concurrency", 1)))
        self._relation_listings: dict[_SchemaKey, SchemaRelationListing] = {}
        self._column_listings: dict[_SchemaKey, SchemaColumnListing] = {}
        self._capped_column_schemas: set[_SchemaKey] = set()
        self._remembered: dict[tuple[str, ...], object] = {}
        self._remember_lock: threading.Lock = threading.Lock()
        self._relation_requests: dict[_RelationRequestKey, tuple[RelationInfo, ...]] = {}
        self._relation_columns: dict[_RelationIdentity, tuple[ColumnInfo, ...] | None] = {}
        self._view_definitions: dict[_RelationIdentity, str | None] = {}

    def remember[ValueT](self, *, key: tuple[str, ...], compute: Callable[[], ValueT]) -> ValueT:
        """Compute one adapter-owned metadata fact at most once per planning invocation."""

        with self._remember_lock:
            if key in self._remembered:
                return cast(ValueT, self._remembered[key])
        value: ValueT = compute()
        with self._remember_lock:
            self._remembered.setdefault(key, value)
        return value

    @property
    def schema_scoped(self) -> bool:
        """Return whether schema listings come from one case-aware read per schema."""

        return self._reader is not None

    @property
    def concurrency(self) -> int:
        """Return the bounded number of concurrent reads this adapter supports."""

        return self._concurrency

    def list_relations(
        self,
        *,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[RelationInfo, ...]:
        """Return the relations ``adapter.list_relations`` would return for this request."""

        if self._reader is None or not schemas:
            return self._list_relations_directly(database=database, schemas=schemas, names=names)
        unique_schemas: tuple[str, ...] = tuple(dict.fromkeys(schemas))
        self.prefetch_relation_listings(database=database, schemas=unique_schemas)
        wanted: frozenset[str] | None = (
            None if names is None else frozenset(self._reader.metadata_name_key(n) for n in names)
        )
        relations: list[RelationInfo] = []
        schema: str
        for schema in unique_schemas:
            listing: SchemaRelationListing = self._relation_listings[
                self._schema_key(database=database, schema=schema)
            ]
            relations.extend(
                entry.relation
                for entry in listing.entries
                if wanted is None or entry.stored_name in wanted
            )
        return tuple(relations)

    def prefetch_relation_listings(self, *, database: str | None, schemas: tuple[str, ...]) -> None:
        """Read every not-yet-listed schema of one database concurrently."""

        self.prefetch_schema_listings(scopes=tuple((database, schema) for schema in schemas))

    def prefetch_schema_listings(
        self, *, scopes: tuple[tuple[str | None, str], ...], best_effort: bool = False
    ) -> None:
        """Read unlisted schemas concurrently; best-effort failures stay uncached for re-read."""

        reader: SchemaScopedMetadataReader | None = self._reader
        if reader is None:
            return
        missing: dict[_SchemaKey, tuple[str | None, str]] = {}
        database: str | None
        schema: str
        for database, schema in scopes:
            key: _SchemaKey = self._schema_key(database=database, schema=schema)
            if key not in self._relation_listings:
                missing.setdefault(key, (database, schema))
        listings: list[SchemaRelationListing | None] = run_bounded_inspections(
            tasks=tuple(
                self._bind_relation_listing(
                    reader=reader, database=database, schema=schema, best_effort=best_effort
                )
                for database, schema in missing.values()
            ),
            concurrency=self._concurrency,
        )
        missing_key: _SchemaKey
        listing: SchemaRelationListing | None
        for missing_key, listing in zip(missing, listings, strict=True):
            if listing is not None:
                self._relation_listings[missing_key] = listing

    def prefetch_view_definitions(
        self, *, requests: tuple[tuple[str | None, str, tuple[str, ...]], ...]
    ) -> None:
        """Read each (database, schema, names) request's view definitions in one batched read."""

        read_batch: ViewDefinitionBatchReader | None = getattr(
            self.adapter, "read_view_definitions", None
        )
        if read_batch is None:
            return
        pending: list[tuple[str | None, str, tuple[str, ...]]] = []
        database: str | None
        schema: str
        names: tuple[str, ...]
        for database, schema, names in requests:
            missing: tuple[str, ...] = tuple(
                name
                for name in dict.fromkeys(names)
                if (database, schema, name) not in self._view_definitions
            )
            if missing:
                pending.append((database, schema, missing))
        results: list[dict[str, str]] = run_bounded_inspections(
            tasks=tuple(
                self._bind_view_definitions(
                    read_batch=read_batch, database=database, schema=schema, names=names
                )
                for database, schema, names in pending
            ),
            concurrency=self._concurrency,
        )
        definitions: dict[str, str]
        for (database, schema, names), definitions in zip(pending, results, strict=True):
            name: str
            for name in names:
                self._view_definitions[(database, schema, name)] = definitions.get(name)

    def cached_view_definition(
        self, *, database: str | None, schema: str, name: str
    ) -> tuple[bool, str | None]:
        """Return whether a prefetched definition is known for this view, and the definition."""

        key: _RelationIdentity = (database, schema, name)
        if key not in self._view_definitions:
            return False, None
        return True, self._view_definitions[key]

    def _bind_view_definitions(
        self,
        *,
        read_batch: ViewDefinitionBatchReader,
        database: str | None,
        schema: str,
        names: tuple[str, ...],
    ) -> Callable[[], dict[str, str]]:
        def read_definitions() -> dict[str, str]:
            definitions: dict[str, str] = {}
            start: int
            for start in range(0, len(names), INSPECTION_IN_LIST_LIMIT):
                definitions.update(
                    read_batch(
                        connection=self.connection,
                        database=database,
                        schema=schema,
                        names=names[start : start + INSPECTION_IN_LIST_LIMIT],
                    )
                )
            return definitions

        return read_definitions

    def get_columns_for_relations(
        self, *, relations: tuple[RelationInfo, ...]
    ) -> dict[_RelationIdentity, tuple[ColumnInfo, ...]]:
        """Return the columns ``adapter.get_columns_for_relations`` would return."""

        unique: dict[_RelationIdentity, RelationInfo] = {}
        relation: RelationInfo
        for relation in relations:
            unique.setdefault(relation.identity, relation)
        missing: tuple[RelationInfo, ...] = tuple(
            relation
            for identity, relation in unique.items()
            if identity not in self._relation_columns
        )
        if missing:
            if self._reader is None:
                self._read_columns_directly(relations=missing)
            else:
                self._read_columns_by_schema(reader=self._reader, relations=missing)
        result: dict[_RelationIdentity, tuple[ColumnInfo, ...]] = {}
        identity: _RelationIdentity
        for identity in unique:
            columns: tuple[ColumnInfo, ...] | None = self._relation_columns.get(identity)
            if columns is not None:
                result[identity] = columns
        return result

    def _list_relations_directly(
        self,
        *,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None,
    ) -> tuple[RelationInfo, ...]:
        key: _RelationRequestKey = (
            database,
            None if schemas is None else tuple(schemas),
            None if names is None else tuple(names),
        )
        cached: tuple[RelationInfo, ...] | None = self._relation_requests.get(key)
        if cached is not None:
            return cached
        relations: tuple[RelationInfo, ...]
        if self._reader is not None and names is not None and len(names) > INSPECTION_IN_LIST_LIMIT:
            unique_names: tuple[str, ...] = tuple(dict.fromkeys(names))
            chunks: tuple[tuple[str, ...], ...] = tuple(
                unique_names[start : start + INSPECTION_IN_LIST_LIMIT]
                for start in range(0, len(unique_names), INSPECTION_IN_LIST_LIMIT)
            )
            chunk_results: list[tuple[RelationInfo, ...]] = run_bounded_inspections(
                tasks=tuple(
                    self._bind_direct_listing(database=database, schemas=schemas, names=chunk)
                    for chunk in chunks
                ),
                concurrency=self._concurrency,
            )
            relations = _flatten_relations(chunk_results)
        else:
            relations = self._bind_direct_listing(database=database, schemas=schemas, names=names)()
        self._relation_requests[key] = relations
        return relations

    def _bind_direct_listing(
        self,
        *,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None,
    ) -> Callable[[], tuple[RelationInfo, ...]]:
        def list_directly() -> tuple[RelationInfo, ...]:
            started: float = time.monotonic()
            relations: tuple[RelationInfo, ...] = self.adapter.list_relations(
                connection=self.connection, database=database, schemas=schemas, names=names
            )
            emit_inspection_record(
                record=InspectionQueryRecord(
                    sql=(
                        f"list_relations(database={database}, "
                        f"schemas={_describe(schemas)}, names={_describe(names)})"
                    ),
                    elapsed_seconds=time.monotonic() - started,
                    row_count=len(relations),
                )
            )
            return relations

        return list_directly

    def _bind_relation_listing(
        self,
        *,
        reader: SchemaScopedMetadataReader,
        database: str | None,
        schema: str,
        best_effort: bool = False,
    ) -> Callable[[], SchemaRelationListing | None]:
        def read_listing() -> SchemaRelationListing | None:
            try:
                return self._read_relation_listing(reader=reader, database=database, schema=schema)
            except Exception:
                if best_effort:
                    return None
                raise

        return read_listing

    def _read_relation_listing(
        self, *, reader: SchemaScopedMetadataReader, database: str | None, schema: str
    ) -> SchemaRelationListing:
        """Read one schema's relations, and keep them for the build's run when one is open."""

        cache: RelationMetadataCache | None = current_relation_metadata_cache(adapter=self.adapter)
        if cache is None or not isinstance(self.adapter, RelationListingScoper):
            return reader.read_schema_relation_listing(
                connection=self.connection, database=database, schema=schema
            )
        versions: tuple[int, dict[str, int]] = cache.listing_versions()
        listing: SchemaRelationListing = reader.read_schema_relation_listing(
            connection=self.connection, database=database, schema=schema
        )
        _ = cache.keep_schema_listing(
            scope=self.adapter.relation_listing_scope(
                connection=self.connection, database=database, schema=schema
            ),
            relations={entry.stored_name: entry.relation for entry in listing.entries},
            versions=versions,
            max_age_seconds=SCHEMA_LISTING_MAX_AGE_SECONDS,
        )
        return listing

    def _read_columns_directly(self, *, relations: tuple[RelationInfo, ...]) -> None:
        started: float = time.monotonic()
        columns: dict[_RelationIdentity, tuple[ColumnInfo, ...]] = (
            self.adapter.get_columns_for_relations(connection=self.connection, relations=relations)
        )
        emit_inspection_record(
            record=InspectionQueryRecord(
                sql=f"get_columns_for_relations(relations={len(relations)})",
                elapsed_seconds=time.monotonic() - started,
                row_count=sum(len(value) for value in columns.values()),
            )
        )
        relation: RelationInfo
        for relation in relations:
            self._relation_columns[relation.identity] = columns.get(relation.identity)

    def _read_columns_by_schema(
        self, *, reader: SchemaScopedMetadataReader, relations: tuple[RelationInfo, ...]
    ) -> None:
        """Use exact per-relation reads for small requests and one read per schema otherwise."""

        unscoped: list[RelationInfo] = []
        remaining: list[RelationInfo] = []
        relation: RelationInfo
        for relation in relations:
            if relation.schema is None:
                unscoped.append(relation)
                continue
            listing: SchemaColumnListing | None = self._column_listings.get(
                self._schema_key(database=relation.database, schema=relation.schema)
            )
            if listing is None:
                remaining.append(relation)
                continue
            self._relation_columns[relation.identity] = listing.columns_by_stored_name.get(
                reader.metadata_name_key(relation.name)
            )
        if unscoped:
            self._read_columns_directly(relations=tuple(unscoped))
        if not remaining:
            return
        if len(remaining) <= reader.exact_column_inspection_limit:
            exact_results: list[tuple[ColumnInfo, ...] | None] = run_bounded_inspections(
                tasks=tuple(
                    self._bind_relation_columns(reader=reader, relation=exact)
                    for exact in remaining
                ),
                concurrency=self._concurrency,
            )
            exact_columns: tuple[ColumnInfo, ...] | None
            for relation, exact_columns in zip(remaining, exact_results, strict=True):
                self._relation_columns[relation.identity] = exact_columns
            return
        by_schema: dict[_SchemaKey, list[RelationInfo]] = {}
        for relation in remaining:
            by_schema.setdefault(
                self._schema_key(database=relation.database, schema=relation.schema or ""), []
            ).append(relation)
        listings: list[SchemaColumnListing] = run_bounded_inspections(
            tasks=tuple(
                self._bind_column_listing(
                    reader=reader,
                    database=scoped[0].database,
                    schema=scoped[0].schema or "",
                    stored_names=self._stored_names(reader=reader, relations=scoped),
                    known_capped=schema_key in self._capped_column_schemas,
                )
                for schema_key, scoped in by_schema.items()
            ),
            concurrency=self._concurrency,
        )
        schema_key: _SchemaKey
        scoped: list[RelationInfo]
        column_listing: SchemaColumnListing
        for (schema_key, scoped), column_listing in zip(by_schema.items(), listings, strict=True):
            if column_listing.complete:
                self._column_listings[schema_key] = column_listing
            else:
                self._capped_column_schemas.add(schema_key)
            for relation in scoped:
                self._relation_columns[relation.identity] = (
                    column_listing.columns_by_stored_name.get(
                        reader.metadata_name_key(relation.name)
                    )
                )

    @staticmethod
    def _stored_names(
        *, reader: SchemaScopedMetadataReader, relations: list[RelationInfo]
    ) -> frozenset[str]:
        return frozenset(reader.metadata_name_key(relation.name) for relation in relations)

    def _bind_column_listing(
        self,
        *,
        reader: SchemaScopedMetadataReader,
        database: str | None,
        schema: str,
        stored_names: frozenset[str],
        known_capped: bool,
    ) -> Callable[[], SchemaColumnListing]:
        def read_listing() -> SchemaColumnListing:
            return reader.read_schema_column_listing(
                connection=self.connection,
                database=database,
                schema=schema,
                stored_names=stored_names,
                known_capped=known_capped,
            )

        return read_listing

    def _bind_relation_columns(
        self, *, reader: SchemaScopedMetadataReader, relation: RelationInfo
    ) -> Callable[[], tuple[ColumnInfo, ...] | None]:
        def read_columns() -> tuple[ColumnInfo, ...] | None:
            return reader.read_relation_columns(connection=self.connection, relation=relation)

        return read_columns

    def _schema_key(self, *, database: str | None, schema: str) -> _SchemaKey:
        if self._reader is None:
            return (database, schema)
        return (
            None if database is None else self._reader.metadata_name_key(database),
            self._reader.metadata_name_key(schema),
        )


def _flatten_relations(chunks: list[tuple[RelationInfo, ...]]) -> tuple[RelationInfo, ...]:
    relations: list[RelationInfo] = []
    chunk: tuple[RelationInfo, ...]
    for chunk in chunks:
        relations.extend(chunk)
    return tuple(relations)


def _describe(values: tuple[str, ...] | None) -> str:
    if values is None:
        return "*"
    if len(values) <= INSPECTION_DESCRIBED_NAME_LIMIT:
        return "[" + ", ".join(values) + "]"
    return f"[{len(values)} names]"
