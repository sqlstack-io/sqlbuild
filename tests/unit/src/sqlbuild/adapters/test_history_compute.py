"""Which built-in adapters start or wake billed compute when a code location reads history."""

from __future__ import annotations

import pytest

from sqlbuild.adapters.bigquery.classes.bigquery_adapter import BigQueryAdapter
from sqlbuild.adapters.clickhouse.classes.clickhouse_adapter import ClickHouseAdapter
from sqlbuild.adapters.databricks.classes.databricks_adapter import DatabricksAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.motherduck.classes.motherduck_adapter import MotherDuckAdapter
from sqlbuild.adapters.postgres.classes.postgres_adapter import PostgresAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.adapters.sqlite.classes.sqlite_adapter import SqliteAdapter
from sqlbuild.adapters.sqlserver.classes.sqlserver_adapter import SqlServerAdapter
from tests.unit.src.sqlbuild.adapters._test_types import HistoryComputeTestCase


@pytest.mark.parametrize(
    "test_case",
    [
        HistoryComputeTestCase("Databricks starts a SQL warehouse", DatabricksAdapter, True),
        HistoryComputeTestCase("ClickHouse wakes an idle service", ClickHouseAdapter, True),
        HistoryComputeTestCase("MotherDuck runs every query on compute", MotherDuckAdapter, True),
        HistoryComputeTestCase(
            "Snowflake skips unchanged tables with metadata", SnowflakeAdapter, False
        ),
        HistoryComputeTestCase("BigQuery has no warehouse to resume", BigQueryAdapter, False),
        HistoryComputeTestCase("PostgreSQL is a running server", PostgresAdapter, False),
        HistoryComputeTestCase("SQL Server is a running server", SqlServerAdapter, False),
        HistoryComputeTestCase("DuckDB reads a local file", DuckDbAdapter, False),
        HistoryComputeTestCase("SQLite reads a local file", SqliteAdapter, False),
    ],
    ids=lambda case: case.description,
)
def test_given_an_adapter_when_a_location_reads_history_then_it_declares_whether_compute_wakes(
    test_case: HistoryComputeTestCase,
) -> None:
    assert test_case.adapter.history_reads_wake_compute is test_case.expected_wakes_compute
