"""Metadata-only relation row counts used by the default full diff size guard."""

from __future__ import annotations

from typing import Any, cast

import duckdb
import pytest

from sqlbuild.adapter.contract.models import RelationRowCountEstimate
from sqlbuild.adapters.bigquery.classes.bigquery_adapter import BigQueryAdapter
from sqlbuild.adapters.clickhouse.classes.clickhouse_adapter import ClickHouseAdapter
from sqlbuild.adapters.databricks.classes.databricks_adapter import DatabricksAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.postgres.classes.postgres_adapter import PostgresAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.adapters.sqlserver.classes.sqlserver_adapter import SqlServerAdapter
from tests.unit.src.sqlbuild.adapters._test_types import (
    BigQueryRowCountEstimateTestCase,
    DuckDbRowCountEstimateTestCase,
    RelationRowCountEstimateTestCase,
)
from tests.unit.src.sqlbuild.adapters.helpers import (
    FakeBigQueryClient,
    FakeBigQueryConnection,
    FakeBigQueryTable,
    MetadataConnection,
    execute_through_cursor,
)

_DATABRICKS_DETAIL_HEADER: tuple[tuple[object, ...], ...] = (
    ("order_id", "bigint", None),
    ("", "", ""),
    ("# Detailed Table Information", "", ""),
)


@pytest.mark.parametrize(
    "test_case",
    [
        RelationRowCountEstimateTestCase(
            description="postgres analyzed table reports reltuples",
            adapter=PostgresAdapter(),
            metadata_rows=(("r", 1234.0, 8192),),
            expected_row_count=1234,
            expected_detail=None,
            expected_sql_fragment="relation.reltuples",
        ),
        RelationRowCountEstimateTestCase(
            description="postgres unanalyzed table with data is unknown",
            adapter=PostgresAdapter(),
            metadata_rows=(("r", -1.0, 8192),),
            expected_row_count=None,
            expected_detail="table statistics not collected; run ANALYZE",
            expected_sql_fragment="pg_relation_size",
        ),
        RelationRowCountEstimateTestCase(
            description="postgres table without pages is empty",
            adapter=PostgresAdapter(),
            metadata_rows=(("r", -1.0, 0),),
            expected_row_count=0,
            expected_detail=None,
            expected_sql_fragment="namespace.nspname = 'dev'",
        ),
        RelationRowCountEstimateTestCase(
            description="postgres view is unknown",
            adapter=PostgresAdapter(),
            metadata_rows=(("v", -1.0, 0),),
            expected_row_count=None,
            expected_detail="no table statistics (view or partitioned table)",
            expected_sql_fragment="relation.relname = 'orders'",
        ),
        RelationRowCountEstimateTestCase(
            description="postgres missing relation is unknown",
            adapter=PostgresAdapter(),
            metadata_rows=(),
            expected_row_count=None,
            expected_detail="relation not found",
            expected_sql_fragment="pg_class",
        ),
        RelationRowCountEstimateTestCase(
            description="snowflake table reports information schema row count",
            adapter=SnowflakeAdapter(),
            metadata_rows=((5000,),),
            expected_row_count=5000,
            expected_detail=None,
            expected_sql_fragment="SELECT row_count FROM",
        ),
        RelationRowCountEstimateTestCase(
            description="snowflake view has no row count",
            adapter=SnowflakeAdapter(),
            metadata_rows=((None,),),
            expected_row_count=None,
            expected_detail="no row count (view)",
            expected_sql_fragment="('ORDERS', 'DEV', 'ANALYTICS')",
        ),
        RelationRowCountEstimateTestCase(
            description="snowflake missing relation is unknown",
            adapter=SnowflakeAdapter(),
            metadata_rows=(),
            expected_row_count=None,
            expected_detail="relation not found",
            expected_sql_fragment="information_schema.tables",
        ),
        RelationRowCountEstimateTestCase(
            description="databricks analyzed table reports statistics rows",
            adapter=DatabricksAdapter(),
            metadata_rows=_DATABRICKS_DETAIL_HEADER
            + (("Type", "MANAGED", ""), ("Statistics", "2048 bytes, 42 rows", "")),
            expected_row_count=42,
            expected_detail=None,
            expected_sql_fragment="DESCRIBE TABLE EXTENDED `analytics`.`dev`.`orders`",
        ),
        RelationRowCountEstimateTestCase(
            description="databricks table without statistics is unknown",
            adapter=DatabricksAdapter(),
            metadata_rows=_DATABRICKS_DETAIL_HEADER + (("Type", "MANAGED", ""),),
            expected_row_count=None,
            expected_detail="table statistics not collected; run ANALYZE TABLE ... COMPUTE STATISTICS",
            expected_sql_fragment="DESCRIBE TABLE EXTENDED",
        ),
        RelationRowCountEstimateTestCase(
            description="databricks column named statistics is not read as table statistics",
            adapter=DatabricksAdapter(),
            metadata_rows=(("statistics", "12 rows", None), *_DATABRICKS_DETAIL_HEADER[1:])
            + (("Type", "EXTERNAL", ""),),
            expected_row_count=None,
            expected_detail="table statistics not collected; run ANALYZE TABLE ... COMPUTE STATISTICS",
            expected_sql_fragment="DESCRIBE TABLE EXTENDED",
        ),
        RelationRowCountEstimateTestCase(
            description="databricks view is unknown",
            adapter=DatabricksAdapter(),
            metadata_rows=_DATABRICKS_DETAIL_HEADER + (("Type", "VIEW", ""),),
            expected_row_count=None,
            expected_detail="no row count (view)",
            expected_sql_fragment="DESCRIBE TABLE EXTENDED",
        ),
        RelationRowCountEstimateTestCase(
            description="sql server table sums partition rows",
            adapter=SqlServerAdapter(),
            metadata_rows=(("U ", 777),),
            expected_row_count=777,
            expected_detail=None,
            expected_sql_fragment="FROM [analytics].sys.objects",
        ),
        RelationRowCountEstimateTestCase(
            description="sql server view is unknown",
            adapter=SqlServerAdapter(),
            metadata_rows=(("V ", None),),
            expected_row_count=None,
            expected_detail="no row count (view)",
            expected_sql_fragment="partitions.index_id IN (0, 1)",
        ),
        RelationRowCountEstimateTestCase(
            description="sql server missing relation is unknown",
            adapter=SqlServerAdapter(),
            metadata_rows=(),
            expected_row_count=None,
            expected_detail="relation not found",
            expected_sql_fragment="schemas.name = N'dev'",
        ),
        RelationRowCountEstimateTestCase(
            description="postgres bytes remain available without analyzed rows",
            adapter=PostgresAdapter(),
            metadata_rows=(("r", -1.0, 8192),),
            expected_row_count=None,
            expected_detail="table statistics not collected; run ANALYZE",
            expected_sql_fragment="pg_relation_size",
            metadata_only=True,
            expected_byte_count=8192,
            expected_byte_count_basis="pg_relation_size_main_fork",
        ),
        RelationRowCountEstimateTestCase(
            description="postgres empty storage is a known zero",
            adapter=PostgresAdapter(),
            metadata_rows=(("r", -1.0, 0),),
            expected_row_count=0,
            expected_detail=None,
            expected_sql_fragment="pg_relation_size",
            metadata_only=True,
            expected_byte_count=0,
            expected_byte_count_basis="pg_relation_size_main_fork",
        ),
        RelationRowCountEstimateTestCase(
            description="snowflake bytes and rows are distinct",
            adapter=SnowflakeAdapter(),
            metadata_rows=((17, 4096),),
            expected_row_count=17,
            expected_detail=None,
            expected_sql_fragment="SELECT row_count,bytes FROM",
            metadata_only=True,
            expected_byte_count=4096,
            expected_byte_count_basis="information_schema_bytes",
        ),
        RelationRowCountEstimateTestCase(
            description="snowflake missing view statistics remain unavailable",
            adapter=SnowflakeAdapter(),
            metadata_rows=((None, None),),
            expected_row_count=None,
            expected_detail="no row count (view)",
            expected_sql_fragment="SELECT row_count,bytes FROM",
            metadata_only=True,
        ),
        RelationRowCountEstimateTestCase(
            description="databricks bytes do not imply an analyzed row count",
            adapter=DatabricksAdapter(),
            metadata_rows=_DATABRICKS_DETAIL_HEADER
            + (("Type", "MANAGED", ""), ("Statistics", "2048 bytes", "")),
            expected_row_count=None,
            expected_detail="table statistics not collected; run ANALYZE TABLE ... COMPUTE STATISTICS",
            expected_sql_fragment="DESCRIBE TABLE EXTENDED",
            metadata_only=True,
            expected_byte_count=2048,
            expected_byte_count_basis="describe_table_statistics_bytes",
        ),
        RelationRowCountEstimateTestCase(
            description="clickhouse bytes do not imply a known row count",
            adapter=ClickHouseAdapter(),
            metadata_rows=(("MergeTree", None, 16384),),
            expected_row_count=None,
            expected_detail="engine MergeTree does not report row counts",
            expected_sql_fragment="SELECT engine, total_rows, total_bytes FROM system.tables",
            metadata_only=True,
            expected_byte_count=16384,
            expected_byte_count_basis="system_tables_total_bytes",
        ),
        RelationRowCountEstimateTestCase(
            description="clickhouse missing bytes do not imply empty storage",
            adapter=ClickHouseAdapter(),
            metadata_rows=(("Memory", 3, None),),
            expected_row_count=3,
            expected_detail=None,
            expected_sql_fragment="SELECT engine, total_rows, total_bytes FROM system.tables",
            metadata_only=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_catalog_metadata_when_estimating_row_count_then_reads_only_metadata(
    test_case: RelationRowCountEstimateTestCase, monkeypatch: pytest.MonkeyPatch
) -> None:
    connection: MetadataConnection = MetadataConnection(test_case.metadata_rows)

    monkeypatch.setattr(test_case.adapter, "execute", execute_through_cursor)

    estimate: RelationRowCountEstimate = test_case.adapter.estimate_relation_row_count(
        connection=connection,
        database=test_case.database,
        schema=test_case.schema,
        name="orders",
        metadata_only=test_case.metadata_only,
    )

    assert estimate.row_count == test_case.expected_row_count
    assert estimate.detail == test_case.expected_detail
    assert estimate.byte_count == test_case.expected_byte_count
    assert estimate.byte_count_basis == test_case.expected_byte_count_basis
    assert len(connection.executed) == 1
    assert test_case.expected_sql_fragment in connection.executed[0]


@pytest.mark.parametrize(
    "test_case",
    [
        BigQueryRowCountEstimateTestCase(
            description="native table reports num_rows",
            table_type="TABLE",
            num_rows=9_000,
            expected_row_count=9_000,
        ),
        BigQueryRowCountEstimateTestCase(
            description="view is unknown",
            table_type="VIEW",
            num_rows=None,
            expected_row_count=None,
        ),
        BigQueryRowCountEstimateTestCase(
            description="external table is unknown",
            table_type="EXTERNAL",
            num_rows=0,
            expected_row_count=None,
        ),
        BigQueryRowCountEstimateTestCase(
            description="native bytes remain available without row statistics",
            table_type="TABLE",
            num_rows=None,
            num_bytes=32768,
            metadata_only=True,
            expected_row_count=None,
            expected_byte_count=32768,
        ),
        BigQueryRowCountEstimateTestCase(
            description="external bytes are not native-table storage",
            table_type="EXTERNAL",
            num_rows=12,
            num_bytes=65536,
            metadata_only=True,
            expected_row_count=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_bigquery_table_metadata_when_estimating_row_count_then_uses_tables_api(
    test_case: BigQueryRowCountEstimateTestCase,
) -> None:
    client: FakeBigQueryClient = FakeBigQueryClient(
        FakeBigQueryTable(
            table_type=test_case.table_type,
            num_rows=test_case.num_rows,
            num_bytes=test_case.num_bytes,
        )
    )
    connection: Any = cast(Any, FakeBigQueryConnection(client=client))

    estimate: RelationRowCountEstimate = BigQueryAdapter().estimate_relation_row_count(
        connection=connection,
        database="proj",
        schema="dev",
        name="orders",
        metadata_only=test_case.metadata_only,
    )

    assert estimate.row_count == test_case.expected_row_count
    assert estimate.byte_count == test_case.expected_byte_count
    assert client.requested == [test_case.expected_table_id]


@pytest.mark.parametrize(
    "test_case",
    [
        DuckDbRowCountEstimateTestCase(
            description="table reports estimated size",
            setup_sql=(
                "CREATE SCHEMA dev",
                "CREATE TABLE dev.orders AS SELECT range AS order_id FROM range(7)",
            ),
            schema="dev",
            name="orders",
            expected_row_count=7,
        ),
        DuckDbRowCountEstimateTestCase(
            description="name matching ignores case",
            setup_sql=(
                "CREATE SCHEMA dev",
                "CREATE TABLE dev.orders AS SELECT range AS order_id FROM range(3)",
            ),
            schema="DEV",
            name="Orders",
            expected_row_count=3,
        ),
        DuckDbRowCountEstimateTestCase(
            description="unqualified schema uses the current schema",
            setup_sql=("CREATE TABLE orders AS SELECT range AS order_id FROM range(4)",),
            schema=None,
            name="orders",
            expected_row_count=4,
        ),
        DuckDbRowCountEstimateTestCase(
            description="view is unknown",
            setup_sql=(
                "CREATE SCHEMA dev",
                "CREATE VIEW dev.orders AS SELECT range AS order_id FROM range(5)",
            ),
            schema="dev",
            name="orders",
            expected_row_count=None,
            expected_detail="no table size metadata (view or missing table)",
        ),
        DuckDbRowCountEstimateTestCase(
            description="table in another schema is not matched",
            setup_sql=(
                "CREATE SCHEMA prod",
                "CREATE TABLE prod.orders AS SELECT range AS order_id FROM range(5)",
            ),
            schema="dev",
            name="orders",
            expected_row_count=None,
            expected_detail="no table size metadata (view or missing table)",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_duckdb_relation_when_estimating_row_count_then_reads_catalog_size(
    test_case: DuckDbRowCountEstimateTestCase,
) -> None:
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    try:
        statement: str
        for statement in test_case.setup_sql:
            connection.execute(statement)

        estimate: RelationRowCountEstimate = DuckDbAdapter().estimate_relation_row_count(
            connection=connection, database=None, schema=test_case.schema, name=test_case.name
        )
    finally:
        connection.close()

    assert estimate.row_count == test_case.expected_row_count
    assert estimate.detail == test_case.expected_detail


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
