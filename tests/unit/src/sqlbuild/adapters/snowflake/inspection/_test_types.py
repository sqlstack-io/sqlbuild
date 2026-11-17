from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class RelationRequestEquivalenceTestCase:
    """One list_relations request answered both directly and through the catalog."""

    description: str
    database: str | None
    schemas: tuple[str, ...] | None
    names: tuple[str, ...] | None
    expected_relation_count: int


@dataclass(frozen=True)
class ColumnRequestEquivalenceTestCase:
    """One column request whose catalog answer must equal the direct adapter answer."""

    description: str
    schemas: tuple[str, ...]
    names: tuple[str, ...] | None
    expected_query_kinds: tuple[str, ...]


@dataclass(frozen=True)
class FreshnessReuseTestCase:
    """Source freshness requests served from schema listings already read by the catalog."""

    description: str
    requests: tuple[tuple[str, str, str], ...]
    listed_schemas: tuple[str, ...]
    expected_metadata_reads: int


@dataclass(frozen=True)
class FreshnessErrorTestCase:
    """A freshness request that must fail identically with and without the catalog."""

    description: str
    request: tuple[str, str, str]
    expected_error_fragment: str


@dataclass(frozen=True)
class RepeatedLookupTestCase:
    """Several filtered lookups of one schema answered from one listing."""

    description: str
    lookups: tuple[tuple[str, ...] | None, ...]
    expected_relation_counts: tuple[int, ...]
    expected_query_kinds: tuple[str, ...]


@dataclass(frozen=True)
class SpeculativePrefetchTestCase:
    """Best-effort listing of a schema whose read fails."""

    description: str
    failing_schema: str
    expected_error_fragment: str
    expected_failed_reads: tuple[int, int]


@dataclass(frozen=True)
class ShowColumnTypeTestCase:
    """One SHOW COLUMNS data_type and the INFORMATION_SCHEMA fields of the same column."""

    description: str
    show_data_type: dict[str, object]
    information_schema_fields: tuple[str, object, object, object]
    expected_type: str


@dataclass(frozen=True)
class InvalidShowColumnTypeTestCase:
    """A SHOW COLUMNS data_type value that is not valid type metadata."""

    description: str
    raw_data_type: object
    expected_error_fragment: str


@dataclass(frozen=True)
class ShowRelationRowTestCase:
    """One SHOW TABLES or SHOW VIEWS row and the relation INFORMATION_SCHEMA would list."""

    description: str
    is_view: bool
    row: dict[str, object]
    expected_relation_type: str
    expected_is_transient: bool | None
    expected_retention_days: int | None


@dataclass(frozen=True)
class MissingObjectErrorTestCase:
    """A SHOW failure classified as an absent schema or a real error."""

    description: str
    errno: int | None
    message: str
    expected_missing: bool


@dataclass(frozen=True)
class ShowResultCapTestCase:
    """A schema whose SHOW output reaches Snowflake's 10,000-row cap."""

    description: str
    relation_count: int
    columns_per_relation: int
    expected_query_kinds: tuple[str, ...]


@dataclass(frozen=True)
class ShowScopeTestCase:
    """A schema listing whose database is missing or comes from the session."""

    description: str
    database: str | None
    expected_relation_count: int
    expected_error_fragment: str | None
    expected_attempted_sql: tuple[str, ...]


@dataclass(frozen=True)
class DroppedRelationTestCase:
    """Columns requested for a relation dropped after it was listed."""

    description: str
    relation_names: tuple[str, ...]
    dropped_name: str
    expected_column_relations: tuple[str, ...]


@dataclass(frozen=True)
class CappedColumnFallbackTestCase:
    """Columns needed from a schema whose SHOW COLUMNS output is exactly the cap."""

    description: str
    relation_count: int
    columns_per_relation: int
    request_batches: tuple[int, ...]
    expected_query_kinds: tuple[str, ...]
    expected_in_list_sizes: tuple[int, ...]


@dataclass(frozen=True)
class MissingSchemaTestCase:
    """SHOW reads of a schema the warehouse does not have, or hides from the role."""

    description: str
    schema: str
    forbidden_schemas: frozenset[str]
    expected_error_fragment: str
    expected_schema_checks: int


@dataclass(frozen=True)
class NestedConcurrencyTestCase:
    """Several cap-sized schemas whose fallback reads would nest inside schema workers."""

    description: str
    schema_count: int
    relations_per_schema: int
    columns_per_relation: int
    statement_latency_seconds: float
    expected_max_concurrent: int
    expected_column_reads: int


@dataclass(frozen=True)
class SingleRelationLookupTestCase:
    """One build-path existence and column lookup answered by SHOW."""

    description: str
    database: str | None
    schema: str
    name: str
    expected_exists: bool
    expected_column_names: tuple[str, ...]
    expected_query_kinds: tuple[str, ...]


@dataclass(frozen=True)
class SingleRelationErrorTestCase:
    """A build-path lookup whose database does not exist."""

    description: str
    database: str
    schema: str
    name: str
    expected_error_fragment: str


@dataclass(frozen=True)
class ShowColumnsEquivalenceTestCase:
    """One relation whose SHOW COLUMNS read must equal its INFORMATION_SCHEMA read."""

    description: str
    database: str
    schema: str
    name: str
    expected_column_count: int


@dataclass(frozen=True)
class SessionDatabaseSwitchTestCase:
    """Unqualified lookups before and after a USE statement switches the session database."""

    description: str
    switch_sql: str
    expected_scopes: tuple[str, ...]
    expected_session_reads: int


@dataclass(frozen=True)
class PartedListingTestCase:
    """Schemas whose tables and views are listed by separate reads running together."""

    description: str
    schema_count: int
    relations_per_schema: int
    statement_latency_seconds: float
    expected_max_concurrent: int
