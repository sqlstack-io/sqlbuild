"""Catalog-served Snowflake metadata must equal what the adapter returns directly."""

from __future__ import annotations

from collections import Counter
from dataclasses import replace

import pytest

from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.contract.models import (
    ColumnInfo,
    RelationInfo,
    TableFreshnessMetadata,
    TableFreshnessRequest,
)
from sqlbuild.adapter.contract.types import TableFreshnessStatus
from sqlbuild.adapter.relations.classes.inspection_catalog import InspectionCatalog
from sqlbuild.adapter.relations.constants import INSPECTION_IN_LIST_LIMIT
from sqlbuild.adapter.relations.main.open_inspection_catalog import open_inspection_catalog
from sqlbuild.adapter.relations.models import SchemaColumnListing
from sqlbuild.adapters.snowflake.classes.snowflake_connection import _SnowflakeConnection
from tests.unit.src.sqlbuild.adapters.snowflake.inspection._test_types import (
    CappedColumnFallbackTestCase,
    ColumnRequestEquivalenceTestCase,
    DroppedRelationTestCase,
    FreshnessErrorTestCase,
    FreshnessReuseTestCase,
    MissingSchemaTestCase,
    NestedConcurrencyTestCase,
    PartedListingTestCase,
    RelationRequestEquivalenceTestCase,
    RepeatedLookupTestCase,
    ShowResultCapTestCase,
    ShowScopeTestCase,
    SpeculativePrefetchTestCase,
)
from tests.unit.src.sqlbuild.adapters.snowflake.inspection.helpers import (
    FakeRelation,
    OfflineSnowflakeAdapter,
    RecordingSnowflakeWarehouse,
    build_inspection_catalog_relations,
    build_offline_snowflake,
    build_wide_schema_relations,
    build_wide_schemas_relations,
    sorted_relation_reprs,
)

_MANY_NAMES: tuple[str, ...] = (
    "orders",
    "customers_v",
    *(f"inventory_{index}" for index in range(INSPECTION_IN_LIST_LIMIT + 10)),
)


@pytest.mark.parametrize(
    "test_case",
    [
        RelationRequestEquivalenceTestCase(
            description="whole schema with case variants, views, transient and external tables",
            database="analytics",
            schemas=("staging",),
            names=None,
            expected_relation_count=67,
        ),
        RelationRequestEquivalenceTestCase(
            description="name filter across schemas ignores quoted lowercase twins",
            database="analytics",
            schemas=("staging", "marts"),
            names=("orders", "orderlines", "customers_v", "revenue_v", "missing"),
            expected_relation_count=2,
        ),
        RelationRequestEquivalenceTestCase(
            description="uppercase request matches unquoted storage",
            database="ANALYTICS",
            schemas=("STAGING",),
            names=("ORDERS",),
            expected_relation_count=1,
        ),
        RelationRequestEquivalenceTestCase(
            description="quoted mixed-case schema is not matched by a logical name",
            database="analytics",
            schemas=("mixed_case",),
            names=None,
            expected_relation_count=0,
        ),
        RelationRequestEquivalenceTestCase(
            description="session database lists only the current catalog",
            database=None,
            schemas=("staging",),
            names=("orders",),
            expected_relation_count=1,
        ),
        RelationRequestEquivalenceTestCase(
            description="database-wide name lookup",
            database="analytics",
            schemas=None,
            names=("orders", "revenue"),
            expected_relation_count=2,
        ),
        RelationRequestEquivalenceTestCase(
            description="database-wide lookup beyond the IN-list cap is chunked",
            database="analytics",
            schemas=None,
            names=_MANY_NAMES,
            expected_relation_count=62,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_metadata_request_when_served_by_catalog_then_matches_direct_adapter(
    test_case: RelationRequestEquivalenceTestCase,
) -> None:
    adapter, connection, warehouse = build_offline_snowflake(
        relations=build_inspection_catalog_relations()
    )
    direct: tuple[RelationInfo, ...] = adapter.list_relations(
        connection=connection,
        database=test_case.database,
        schemas=test_case.schemas,
        names=test_case.names,
    )
    warehouse.reset()

    served: tuple[RelationInfo, ...] = InspectionCatalog(
        adapter=adapter, connection=connection
    ).list_relations(database=test_case.database, schemas=test_case.schemas, names=test_case.names)

    assert len(served) == test_case.expected_relation_count
    assert sorted_relation_reprs(served) == sorted_relation_reprs(direct)
    assert all(query.largest_in_list <= INSPECTION_IN_LIST_LIMIT for query in warehouse.queries)


@pytest.mark.parametrize(
    "test_case",
    [
        ColumnRequestEquivalenceTestCase(
            description="sparse request keeps exact SHOW COLUMNS reads",
            schemas=("staging", "marts"),
            names=("orders", "customers_v", "transient_events", "revenue"),
            expected_query_kinds=("show_columns",) * 4,
        ),
        ColumnRequestEquivalenceTestCase(
            description="broad request reads each schema's columns once without IN lists",
            schemas=("staging",),
            names=None,
            expected_query_kinds=("show_schema_columns",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_relations_when_reading_columns_through_catalog_then_matches_direct_adapter(
    test_case: ColumnRequestEquivalenceTestCase,
) -> None:
    adapter, connection, warehouse = build_offline_snowflake(
        relations=build_inspection_catalog_relations()
    )
    relations: tuple[RelationInfo, ...] = adapter.list_relations(
        connection=connection,
        database="analytics",
        schemas=test_case.schemas,
        names=test_case.names,
    )
    direct: dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]] = (
        adapter.get_columns_for_relations(connection=connection, relations=relations)
    )
    warehouse.reset()
    catalog: InspectionCatalog = InspectionCatalog(adapter=adapter, connection=connection)

    served: dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]] = (
        catalog.get_columns_for_relations(relations=relations)
    )
    repeated: dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]] = (
        catalog.get_columns_for_relations(relations=relations)
    )

    assert served == direct
    assert repeated == direct
    assert Counter(query.kind for query in warehouse.queries) == Counter(
        test_case.expected_query_kinds
    )
    assert all(query.largest_in_list == 0 for query in warehouse.queries)


@pytest.mark.parametrize(
    "test_case",
    [
        FreshnessReuseTestCase(
            description="freshness reads LAST_ALTERED only for requested tables in capped chunks",
            requests=(
                ("analytics", "staging", "orders"),
                ("analytics", "staging", "transient_events"),
                *(("analytics", "staging", f"inventory_{index}") for index in range(60)),
                ("analytics", "marts", "revenue"),
            ),
            listed_schemas=("staging", "marts"),
            expected_metadata_reads=3,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_open_catalog_when_reading_source_freshness_then_reads_requested_tables_only(
    test_case: FreshnessReuseTestCase,
) -> None:
    adapter, connection, warehouse = build_offline_snowflake(
        relations=build_inspection_catalog_relations()
    )
    requests: tuple[TableFreshnessRequest, ...] = tuple(
        TableFreshnessRequest(database=database, schema=schema, name=name)
        for database, schema, name in test_case.requests
    )
    direct: dict[TableFreshnessRequest, TableFreshnessMetadata] = (
        adapter.get_tables_freshness_metadata(connection=connection, requests=requests)
    )
    warehouse.reset()

    with open_inspection_catalog(adapter=adapter, connection=connection) as catalog:
        _ = catalog.list_relations(database="analytics", schemas=test_case.listed_schemas)
        warehouse.reset()
        served: dict[TableFreshnessRequest, TableFreshnessMetadata] = (
            adapter.get_tables_freshness_metadata(connection=connection, requests=requests)
        )

    assert served == direct
    assert len(warehouse.queries_of_kind("tables")) == test_case.expected_metadata_reads
    assert len(warehouse.queries) == test_case.expected_metadata_reads
    assert max(query.largest_in_list for query in warehouse.queries) <= INSPECTION_IN_LIST_LIMIT


@pytest.mark.parametrize(
    "test_case",
    [
        FreshnessErrorTestCase(
            description="view freshness is unavailable like the direct read",
            request=("analytics", "staging", "customers_v"),
            expected_error_fragment="only supports physical tables",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_open_catalog_when_freshness_targets_a_view_then_reports_unavailable_like_direct_read(
    test_case: FreshnessErrorTestCase,
) -> None:
    adapter, connection, _ = build_offline_snowflake(relations=build_inspection_catalog_relations())
    database, schema, name = test_case.request
    request: TableFreshnessRequest = TableFreshnessRequest(
        database=database, schema=schema, name=name
    )
    direct: TableFreshnessMetadata = adapter.get_tables_freshness_metadata(
        connection=connection, requests=(request,)
    )[request]

    with open_inspection_catalog(adapter=adapter, connection=connection):
        served: TableFreshnessMetadata = adapter.get_tables_freshness_metadata(
            connection=connection, requests=(request,)
        )[request]

    assert served == direct
    assert direct.status == TableFreshnessStatus.UNAVAILABLE
    assert test_case.expected_error_fragment in str(direct.message)


@pytest.mark.parametrize(
    "test_case",
    [
        RepeatedLookupTestCase(
            description="state table, model, and whole-schema lookups share one listing",
            lookups=(("_sqlbuild_fingerprints",), ("orders",), None),
            expected_relation_counts=(0, 1, 67),
            expected_query_kinds=("show_tables", "show_views"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_repeated_lookups_when_catalog_is_open_then_lists_schema_once(
    test_case: RepeatedLookupTestCase,
) -> None:
    adapter, connection, warehouse = build_offline_snowflake(
        relations=build_inspection_catalog_relations()
    )
    catalog: InspectionCatalog = InspectionCatalog(adapter=adapter, connection=connection)

    counts: tuple[int, ...] = tuple(
        len(catalog.list_relations(database="analytics", schemas=("staging",), names=names))
        for names in test_case.lookups
    )

    assert counts == test_case.expected_relation_counts
    assert sorted(query.kind for query in warehouse.queries) == sorted(
        test_case.expected_query_kinds
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SpeculativePrefetchTestCase(
            description="unreadable speculative schema fails only the request that needs it",
            failing_schema="RESTRICTED",
            expected_error_fragment="unavailable",
            expected_failed_reads=(3, 4),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unreadable_schema_when_prefetching_best_effort_then_only_real_reads_fail(
    test_case: SpeculativePrefetchTestCase,
) -> None:
    warehouse: RecordingSnowflakeWarehouse = RecordingSnowflakeWarehouse(
        relations=build_inspection_catalog_relations(),
        failing_metadata_schemas=frozenset({test_case.failing_schema}),
    )
    adapter: OfflineSnowflakeAdapter = OfflineSnowflakeAdapter(warehouse=warehouse)
    catalog: InspectionCatalog = InspectionCatalog(adapter=adapter, connection=adapter.connect({}))

    catalog.prefetch_schema_listings(
        scopes=(("analytics", "staging"), ("analytics", test_case.failing_schema.lower())),
        best_effort=True,
    )
    staging: tuple[RelationInfo, ...] = catalog.list_relations(
        database="analytics", schemas=("staging",), names=("orders",)
    )
    with pytest.raises(RuntimeError, match=test_case.expected_error_fragment):
        _ = catalog.list_relations(
            database="analytics", schemas=(test_case.failing_schema.lower(),)
        )

    assert len(staging) == 1
    assert len(warehouse.queries_of_kind("show_tables")) == 1
    failed_reads: int = len(warehouse.attempted_sql) - len(warehouse.queries)
    assert test_case.expected_failed_reads[0] <= failed_reads <= test_case.expected_failed_reads[1]


@pytest.mark.parametrize(
    "test_case",
    [
        ShowResultCapTestCase(
            description="more than 10,000 tables are paged",
            relation_count=10_050,
            columns_per_relation=0,
            expected_query_kinds=(
                "show_tables",
                "show_tables",
                "show_views",
                "show_schema_columns",
            ),
        ),
        ShowResultCapTestCase(
            description="exactly 10,000 columns is treated as truncated and reread",
            relation_count=40,
            columns_per_relation=250,
            expected_query_kinds=("show_tables", "show_views", "show_schema_columns", "columns"),
        ),
        ShowResultCapTestCase(
            description="10,001 columns is a complete listing",
            relation_count=73,
            columns_per_relation=137,
            expected_query_kinds=("show_tables", "show_views", "show_schema_columns"),
        ),
        ShowResultCapTestCase(
            description="fewer than 10,000 columns is a complete listing",
            relation_count=39,
            columns_per_relation=256,
            expected_query_kinds=("show_tables", "show_views", "show_schema_columns"),
        ),
        ShowResultCapTestCase(
            description="more than 10,000 columns is a complete listing",
            relation_count=120,
            columns_per_relation=260,
            expected_query_kinds=("show_tables", "show_views", "show_schema_columns"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_schema_beyond_show_cap_when_listing_then_matches_direct_adapter(
    test_case: ShowResultCapTestCase,
) -> None:
    adapter, connection, warehouse = build_offline_snowflake(
        relations=build_wide_schema_relations(
            relation_count=test_case.relation_count,
            columns_per_relation=test_case.columns_per_relation,
        )
    )
    direct: tuple[RelationInfo, ...] = adapter.list_relations(
        connection=connection, database="analytics", schemas=("inventory",)
    )
    direct_columns: dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]] = (
        adapter.get_columns_for_relations(connection=connection, relations=direct)
    )
    warehouse.reset()
    catalog: InspectionCatalog = InspectionCatalog(adapter=adapter, connection=connection)

    served: tuple[RelationInfo, ...] = catalog.list_relations(
        database="analytics", schemas=("inventory",)
    )
    served_columns: dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]] = (
        catalog.get_columns_for_relations(relations=served)
    )

    assert sorted_relation_reprs(served) == sorted_relation_reprs(direct)
    assert served_columns == direct_columns
    assert sorted(query.kind for query in warehouse.queries) == sorted(
        test_case.expected_query_kinds
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ShowScopeTestCase(
            description="session database is resolved once and qualifies every SHOW",
            database=None,
            expected_relation_count=69,
            expected_error_fragment=None,
            expected_attempted_sql=(
                "SELECT CURRENT_DATABASE()",
                'SHOW TABLES IN SCHEMA "ANALYTICS"."STAGING" LIMIT 10000',
                'SHOW VIEWS IN SCHEMA "ANALYTICS"."STAGING" LIMIT 10000',
                'SHOW TABLES IN SCHEMA "ANALYTICS"."MARTS" LIMIT 10000',
                'SHOW VIEWS IN SCHEMA "ANALYTICS"."MARTS" LIMIT 10000',
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_no_database_when_listing_schemas_then_show_uses_the_session_database(
    test_case: ShowScopeTestCase,
) -> None:
    adapter, connection, warehouse = build_offline_snowflake(
        relations=build_inspection_catalog_relations()
    )
    catalog: InspectionCatalog = InspectionCatalog(adapter=adapter, connection=connection)

    staging: tuple[RelationInfo, ...] = catalog.list_relations(
        database=test_case.database, schemas=("staging",)
    )
    marts: tuple[RelationInfo, ...] = catalog.list_relations(
        database=test_case.database, schemas=("marts",)
    )

    assert len(staging) + len(marts) == test_case.expected_relation_count
    assert {relation.database for relation in (*staging, *marts)} == {None}
    assert warehouse.attempted_sql[0] == test_case.expected_attempted_sql[0]
    assert sorted(warehouse.attempted_sql[1:]) == sorted(test_case.expected_attempted_sql[1:])


@pytest.mark.parametrize(
    "test_case",
    [
        ShowScopeTestCase(
            description="a missing database fails instead of listing nothing",
            database="restricted_catalog",
            expected_relation_count=0,
            expected_error_fragment="database restricted_catalog does not exist",
            expected_attempted_sql=(
                'SHOW TABLES IN SCHEMA "RESTRICTED_CATALOG"."STAGING" LIMIT 10000',
                'SHOW VIEWS IN SCHEMA "RESTRICTED_CATALOG"."STAGING" LIMIT 10000',
                "SHOW SCHEMAS LIKE 'STAGING' IN DATABASE \"RESTRICTED_CATALOG\"",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_database_when_listing_schema_then_inspection_fails(
    test_case: ShowScopeTestCase,
) -> None:
    adapter, connection, warehouse = build_offline_snowflake(
        relations=build_inspection_catalog_relations()
    )
    catalog: InspectionCatalog = InspectionCatalog(adapter=adapter, connection=connection)

    with pytest.raises(AdapterUserError, match=test_case.expected_error_fragment or ""):
        _ = catalog.list_relations(database=test_case.database, schemas=("staging",))

    assert set(warehouse.attempted_sql) == set(test_case.expected_attempted_sql)


@pytest.mark.parametrize(
    "test_case",
    [
        DroppedRelationTestCase(
            description="a relation dropped after listing has no columns instead of failing",
            relation_names=("orders", "customers_v", "transient_events"),
            dropped_name="orders_archive",
            expected_column_relations=("customers_v", "orders", "transient_events"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_dropped_relation_when_reading_exact_columns_then_it_is_missing(
    test_case: DroppedRelationTestCase,
) -> None:
    adapter, connection, _ = build_offline_snowflake(relations=build_inspection_catalog_relations())
    listed: tuple[RelationInfo, ...] = adapter.list_relations(
        connection=connection,
        database="analytics",
        schemas=("staging",),
        names=test_case.relation_names,
    )
    relations: tuple[RelationInfo, ...] = (
        *listed,
        replace(listed[0], name=test_case.dropped_name),
    )

    direct: dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]] = (
        adapter.get_columns_for_relations(connection=connection, relations=relations)
    )
    served: dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]] = InspectionCatalog(
        adapter=adapter, connection=connection
    ).get_columns_for_relations(relations=relations)

    assert served == direct
    assert tuple(sorted(identity[2] for identity in served)) == test_case.expected_column_relations


@pytest.mark.parametrize(
    "test_case",
    [
        CappedColumnFallbackTestCase(
            description="a cap-sized schema rereads only needed relations in capped chunks",
            relation_count=100,
            columns_per_relation=100,
            request_batches=(60, 40),
            expected_query_kinds=("columns", "columns", "columns", "show_schema_columns"),
            expected_in_list_sizes=(10, 40, 50),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_cap_sized_column_listing_when_reading_then_only_needed_relations_are_reread(
    test_case: CappedColumnFallbackTestCase,
) -> None:
    adapter, connection, warehouse = build_offline_snowflake(
        relations=build_wide_schema_relations(
            relation_count=test_case.relation_count,
            columns_per_relation=test_case.columns_per_relation,
        )
    )
    listed: tuple[RelationInfo, ...] = tuple(
        sorted(
            adapter.list_relations(
                connection=connection, database="analytics", schemas=("inventory",)
            ),
            key=lambda relation: relation.name,
        )
    )
    direct: dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]] = (
        adapter.get_columns_for_relations(connection=connection, relations=listed)
    )
    warehouse.reset()
    catalog: InspectionCatalog = InspectionCatalog(adapter=adapter, connection=connection)
    offsets: tuple[int, ...] = tuple(
        sum(test_case.request_batches[:index]) for index in range(len(test_case.request_batches))
    )

    served: dict[tuple[str | None, str | None, str], tuple[ColumnInfo, ...]] = {}
    for offset, size in zip(offsets, test_case.request_batches, strict=True):
        served.update(catalog.get_columns_for_relations(relations=listed[offset : offset + size]))

    assert served == direct
    assert tuple(sorted(query.kind for query in warehouse.queries)) == (
        test_case.expected_query_kinds
    )
    assert (
        tuple(sorted(query.largest_in_list for query in warehouse.queries_of_kind("columns")))
        == test_case.expected_in_list_sizes
    )


@pytest.mark.parametrize(
    "test_case",
    [
        MissingSchemaTestCase(
            description="a schema not created yet lists empty after one existence check",
            schema="fresh_target",
            forbidden_schemas=frozenset(),
            expected_error_fragment="",
            expected_schema_checks=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_schema_when_listing_then_relations_and_columns_are_empty(
    test_case: MissingSchemaTestCase,
) -> None:
    adapter, connection, warehouse = build_offline_snowflake(
        relations=build_inspection_catalog_relations()
    )

    with open_inspection_catalog(adapter=adapter, connection=connection) as catalog:
        relations: tuple[RelationInfo, ...] = catalog.list_relations(
            database="analytics", schemas=(test_case.schema,)
        )
        listing: SchemaColumnListing = adapter.read_schema_column_listing(
            connection=connection,
            database="analytics",
            schema=test_case.schema,
            stored_names=frozenset(),
        )

    assert relations == ()
    assert listing.columns_by_stored_name == {}
    assert len(warehouse.queries_of_kind("show_schemas")) == test_case.expected_schema_checks


@pytest.mark.parametrize(
    "test_case",
    [
        MissingSchemaTestCase(
            description="an existing schema the role cannot list re-raises the SHOW error",
            schema="staging",
            forbidden_schemas=frozenset({"STAGING"}),
            expected_error_fragment="Object does not exist, or operation cannot be performed",
            expected_schema_checks=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_existing_schema_when_show_reports_missing_then_original_error_is_raised(
    test_case: MissingSchemaTestCase,
) -> None:
    warehouse: RecordingSnowflakeWarehouse = RecordingSnowflakeWarehouse(
        relations=build_inspection_catalog_relations(),
        forbidden_schemas=test_case.forbidden_schemas,
    )
    adapter: OfflineSnowflakeAdapter = OfflineSnowflakeAdapter(warehouse=warehouse)
    connection: _SnowflakeConnection = adapter.connect({})

    with (
        open_inspection_catalog(adapter=adapter, connection=connection) as catalog,
        pytest.raises(RuntimeError, match=test_case.expected_error_fragment),
    ):
        _ = catalog.list_relations(database="analytics", schemas=(test_case.schema,))

    assert len(warehouse.queries_of_kind("show_schemas")) == test_case.expected_schema_checks


@pytest.mark.parametrize(
    "test_case",
    [
        NestedConcurrencyTestCase(
            description="fallback chunks inside schema workers share the outer bound",
            schema_count=4,
            relations_per_schema=125,
            columns_per_relation=80,
            statement_latency_seconds=0.02,
            expected_max_concurrent=4,
            expected_column_reads=12,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_capped_schemas_when_reading_columns_then_nested_reads_do_not_multiply_bound(
    test_case: NestedConcurrencyTestCase,
) -> None:
    relations: tuple[FakeRelation, ...] = build_wide_schemas_relations(
        schema_count=test_case.schema_count,
        relation_count=test_case.relations_per_schema,
        columns_per_relation=test_case.columns_per_relation,
    )
    adapter, connection, warehouse = build_offline_snowflake(
        relations=relations, statement_latency_seconds=test_case.statement_latency_seconds
    )
    listed: tuple[RelationInfo, ...] = adapter.list_relations(
        connection=connection,
        database="analytics",
        schemas=tuple(f"warehouse_{index}" for index in range(test_case.schema_count)),
    )
    warehouse.reset()

    with open_inspection_catalog(adapter=adapter, connection=connection) as catalog:
        _ = catalog.get_columns_for_relations(relations=listed)

    assert warehouse.max_concurrent_metadata <= test_case.expected_max_concurrent
    assert len(warehouse.queries_of_kind("columns")) == test_case.expected_column_reads


@pytest.mark.parametrize(
    "test_case",
    [
        PartedListingTestCase(
            description="each schema's tables and views are listed at once",
            schema_count=2,
            relations_per_schema=3,
            statement_latency_seconds=0.05,
            expected_max_concurrent=4,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_schemas_when_prefetching_listings_then_tables_and_views_run_together(
    test_case: PartedListingTestCase,
) -> None:
    relations: tuple[FakeRelation, ...] = build_wide_schemas_relations(
        schema_count=test_case.schema_count,
        relation_count=test_case.relations_per_schema,
        columns_per_relation=1,
    )
    adapter, connection, warehouse = build_offline_snowflake(
        relations=relations, statement_latency_seconds=test_case.statement_latency_seconds
    )
    schemas: tuple[str, ...] = tuple(
        f"warehouse_{index}" for index in range(test_case.schema_count)
    )
    direct: tuple[RelationInfo, ...] = adapter.list_relations(
        connection=connection, database="analytics", schemas=schemas
    )
    warehouse.reset()

    with open_inspection_catalog(adapter=adapter, connection=connection) as catalog:
        catalog.prefetch_schema_listings(scopes=tuple(("analytics", schema) for schema in schemas))
        listed: tuple[RelationInfo, ...] = catalog.list_relations(
            database="analytics", schemas=schemas
        )

    assert sorted(relation.identity for relation in listed) == sorted(
        relation.identity for relation in direct
    )
    assert warehouse.max_concurrent_metadata == test_case.expected_max_concurrent
    assert len(warehouse.queries_of_kind("show_tables")) == test_case.schema_count
    assert len(warehouse.queries_of_kind("show_views")) == test_case.schema_count


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
