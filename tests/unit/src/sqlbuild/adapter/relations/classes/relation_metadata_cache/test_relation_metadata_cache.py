"""Identical metadata lookups are served once until SQL may have changed the relation."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

from sqlbuild.adapter.contract.models import ColumnInfo, RelationInfo
from sqlbuild.adapter.relations.classes.relation_metadata_cache import RelationMetadataCache
from sqlbuild.adapter.relations.main.cached_relation_columns import cached_relation_columns
from sqlbuild.adapter.relations.main.cached_relation_exists import cached_relation_exists
from sqlbuild.adapter.relations.main.relation_metadata_cache_scope import (
    relation_metadata_cache_scope,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from tests.unit.src.sqlbuild.adapter.relations.classes.relation_metadata_cache._test_types import (
    CacheInvalidationTestCase,
    ListingExpiryTestCase,
    LiveInvalidationTestCase,
    OverlappingReadTestCase,
)
from tests.unit.src.sqlbuild.adapter.relations.classes.relation_metadata_cache.helpers import (
    CountingColumnsRead,
    read_orders,
)


@pytest.mark.parametrize(
    "test_case",
    [
        CacheInvalidationTestCase(
            description="no statement serves the cached read",
            transactional_ddl=False,
            statements_between=(),
            expected_reads=1,
        ),
        CacheInvalidationTestCase(
            description="data statements keep the entry",
            transactional_ddl=False,
            statements_between=(
                "INSERT INTO analytics.marts.orders SELECT * FROM analytics.marts.orders__delta",
                "SELECT COUNT(*) FROM analytics.marts.orders",
            ),
            expected_reads=1,
        ),
        CacheInvalidationTestCase(
            description="DDL on another relation keeps the entry",
            transactional_ddl=False,
            statements_between=(
                "CREATE OR REPLACE TABLE analytics.marts.customers AS SELECT 1 AS id",
            ),
            expected_reads=1,
        ),
        CacheInvalidationTestCase(
            description="replace of the relation refreshes the entry",
            transactional_ddl=False,
            statements_between=(
                "CREATE OR REPLACE TABLE ANALYTICS.MARTS.ORDERS AS SELECT 1 AS id",
            ),
            expected_reads=2,
        ),
        CacheInvalidationTestCase(
            description="alter of the relation refreshes the entry",
            transactional_ddl=False,
            statements_between=('ALTER TABLE "ANALYTICS"."MARTS"."ORDERS" ADD COLUMN "NOTE" TEXT',),
            expected_reads=2,
        ),
        CacheInvalidationTestCase(
            description="swap into the relation refreshes the entry",
            transactional_ddl=False,
            statements_between=("ALTER TABLE marts.orders__stage SWAP WITH marts.orders",),
            expected_reads=2,
        ),
        CacheInvalidationTestCase(
            description="unknown statement refreshes everything",
            transactional_ddl=False,
            statements_between=("CALL marts.rebuild()",),
            expected_reads=2,
        ),
        CacheInvalidationTestCase(
            description="commit without transactional DDL keeps the entry",
            transactional_ddl=False,
            statements_between=("COMMIT",),
            expected_reads=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_statements_between_lookups_when_reading_columns_then_rereads_only_after_changes(
    test_case: CacheInvalidationTestCase,
) -> None:
    cache: RelationMetadataCache = RelationMetadataCache(
        transactional_ddl=test_case.transactional_ddl
    )
    read: CountingColumnsRead = CountingColumnsRead(cache=cache)

    read_orders(cache=cache, read=read)
    statement: str
    for statement in test_case.statements_between:
        cache.observe_statement(sql=statement)
    read_orders(cache=cache, read=read)

    assert read.reads == test_case.expected_reads


@pytest.mark.parametrize(
    "test_case",
    [
        CacheInvalidationTestCase(
            description=description,
            transactional_ddl=transactional_ddl,
            statements_between=(statement,),
            expected_reads=expected_reads,
        )
        for description, transactional_ddl, statement, expected_reads in (
            ("commit replays transactional DDL", True, "COMMIT", 2),
            ("rollback replays transactional DDL", True, "ROLLBACK", 2),
            ("commit is inert for autocommitted DDL", False, "COMMIT", 1),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_transaction_end_after_ddl_when_reading_columns_then_follows_ddl_visibility(
    test_case: CacheInvalidationTestCase,
) -> None:
    cache: RelationMetadataCache = RelationMetadataCache(
        transactional_ddl=test_case.transactional_ddl
    )
    read: CountingColumnsRead = CountingColumnsRead(cache=cache)

    cache.observe_statement(sql="ALTER TABLE analytics.marts.orders ADD COLUMN note TEXT")
    read_orders(cache=cache, read=read)
    statement: str
    for statement in test_case.statements_between:
        cache.observe_statement(sql=statement)
    read_orders(cache=cache, read=read)

    assert read.reads == test_case.expected_reads


@pytest.mark.parametrize(
    "test_case",
    [
        OverlappingReadTestCase(
            description="DDL during the read is not cached over",
            statement_during_read="DROP TABLE IF EXISTS analytics.marts.orders",
            expected_reads=2,
        ),
        OverlappingReadTestCase(
            description="unrelated DDL during the read still caches",
            statement_during_read="DROP TABLE IF EXISTS analytics.marts.customers",
            expected_reads=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_statement_during_read_when_reading_again_then_never_serves_a_pre_ddl_read(
    test_case: OverlappingReadTestCase,
) -> None:
    cache: RelationMetadataCache = RelationMetadataCache(transactional_ddl=False)
    overlapping: CountingColumnsRead = CountingColumnsRead(
        cache=cache, statement_during_read=test_case.statement_during_read
    )
    read_orders(cache=cache, read=overlapping)
    following: CountingColumnsRead = CountingColumnsRead(cache=cache)

    read_orders(cache=cache, read=following)

    assert overlapping.reads + following.reads == test_case.expected_reads


@pytest.mark.parametrize(
    "test_case",
    [
        LiveInvalidationTestCase(
            description="added column is visible on the next lookup",
            ddl="ALTER TABLE main.orders ADD COLUMN note VARCHAR",
            expected_first_columns=("order_id",),
            expected_second_columns=("order_id", "note"),
            expected_second_exists=True,
        ),
        LiveInvalidationTestCase(
            description="replaced table is read again",
            ddl="CREATE OR REPLACE TABLE main.orders AS SELECT 1 AS order_id, 2 AS customer_id",
            expected_first_columns=("order_id",),
            expected_second_columns=("order_id", "customer_id"),
            expected_second_exists=True,
        ),
        LiveInvalidationTestCase(
            description="dropped table is reported missing",
            ddl="DROP TABLE main.orders",
            expected_first_columns=("order_id",),
            expected_second_columns=(),
            expected_second_exists=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_adapter_ddl_when_looking_up_again_then_returns_fresh_metadata(
    test_case: LiveInvalidationTestCase, tmp_path: Path
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": str(tmp_path / "warehouse.duckdb")})
    _ = adapter.execute(
        connection=connection, sql="CREATE TABLE main.orders AS SELECT 1 AS order_id"
    )

    with relation_metadata_cache_scope(adapter=adapter):
        first: tuple[ColumnInfo, ...] = cached_relation_columns(
            adapter=adapter, connection=connection, database=None, schema="main", name="orders"
        )
        _ = cached_relation_exists(
            adapter=adapter, connection=connection, database=None, schema="main", name="orders"
        )
        _ = adapter.execute(connection=connection, sql=test_case.ddl)
        second: tuple[ColumnInfo, ...] = cached_relation_columns(
            adapter=adapter, connection=connection, database=None, schema="main", name="orders"
        )
        second_exists: bool = cached_relation_exists(
            adapter=adapter, connection=connection, database=None, schema="main", name="orders"
        )
    adapter.close(connection)

    assert tuple(column.name for column in first) == test_case.expected_first_columns
    assert tuple(column.name for column in second) == test_case.expected_second_columns
    assert second_exists is test_case.expected_second_exists


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])


@pytest.mark.parametrize(
    "test_case",
    [
        ListingExpiryTestCase(
            description="a recent listing answers the lookup",
            seconds_later=30.0,
            expected_listings=1,
        ),
        ListingExpiryTestCase(
            description="a listing older than a minute is read again",
            seconds_later=61.0,
            expected_listings=2,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_refreshed_listing_when_looking_up_later_then_expired_listing_is_read_again(
    test_case: ListingExpiryTestCase,
) -> None:
    now: list[float] = [0.0]
    cache: RelationMetadataCache = RelationMetadataCache(
        transactional_ddl=False, clock=lambda: now[0]
    )
    listings: list[int] = []
    orders: RelationInfo = RelationInfo(
        database="ANALYTICS", schema="MARTS", name="ORDERS", relation_type="table"
    )

    def read_listing() -> dict[str, RelationInfo]:
        listings.append(1)
        return {"ORDERS": orders}

    _ = cache.refresh_schema_listing(scope=("ANALYTICS", "MARTS"), read_listing=read_listing)
    now[0] = test_case.seconds_later
    _ = cache.schema_relation(
        scope=("ANALYTICS", "MARTS"),
        name="analytics.marts.orders",
        stored_name="ORDERS",
        read_listing=read_listing,
    )

    assert len(listings) == test_case.expected_listings
