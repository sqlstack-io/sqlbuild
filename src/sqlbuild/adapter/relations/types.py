"""Adapter relation naming and inspection contracts."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Protocol, runtime_checkable

if TYPE_CHECKING:
    from sqlbuild.adapter.contract.models import ColumnInfo, RelationInfo
    from sqlbuild.adapter.relations.models import SchemaColumnListing, SchemaRelationListing


type RelationCacheKey = tuple[str, str | None, str | None, str]
type RelationCacheVersion = tuple[int, int]
type SchemaListingScope = tuple[str | None, str]


class RelationLocation(Protocol):
    """Structural relation location accepted by adapter naming."""

    database: str | None
    schema: str | None
    name: str
    qualified_name: str | None


@runtime_checkable
class RelationListingScoper(Protocol):
    """Adapter whose builds keep schema listings under a stored database and schema name."""

    def relation_listing_scope(
        self, *, connection: Any, database: str | None, schema: str
    ) -> SchemaListingScope:
        """The stored names that key a build's listing of one schema."""
        ...


@runtime_checkable
class SchemaScopedMetadataReader(Protocol):
    """Adapter able to list a whole schema's relations and columns in one read each."""

    exact_column_inspection_limit: int

    def read_schema_relation_listing(
        self, *, connection: Any, database: str | None, schema: str
    ) -> SchemaRelationListing:
        """Read every relation in one schema."""
        ...

    def read_schema_column_listing(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str,
        stored_names: frozenset[str],
        known_capped: bool = False,
    ) -> SchemaColumnListing:
        """Read one schema's columns; an incomplete listing covers only ``stored_names``."""
        ...

    def read_relation_columns(
        self, *, connection: Any, relation: RelationInfo
    ) -> tuple[ColumnInfo, ...] | None:
        """Read one relation's columns, or None when it no longer exists."""
        ...

    def metadata_name_key(self, name: str) -> str:
        """Return the stored metadata name a logical identifier filter matches."""
        ...


class ViewDefinitionBatchReader(Protocol):
    """An adapter's ``read_view_definitions``: stored definitions of named views in one schema."""

    def __call__(
        self, *, connection: Any, database: str | None, schema: str, names: tuple[str, ...]
    ) -> dict[str, str]:
        """Return each found view's definition keyed by the requested name."""
        ...


class InspectionCompletion[ResultT](Protocol):
    """Callback receiving one finished inspection read on the calling thread."""

    def __call__(self, *, index: int, result: ResultT) -> None:
        """Handle the result of the task at ``index``."""
        ...
