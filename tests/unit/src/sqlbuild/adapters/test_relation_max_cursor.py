from __future__ import annotations

import pytest

from sqlbuild.adapters.bigquery.classes.bigquery_adapter import BigQueryAdapter
from sqlbuild.adapters.databricks.classes.databricks_adapter import DatabricksAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.motherduck.classes.motherduck_adapter import MotherDuckAdapter
from sqlbuild.adapters.postgres.classes.postgres_adapter import PostgresAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.adapters.sqlserver.classes.sqlserver_adapter import SqlServerAdapter
from tests.unit.src.sqlbuild.adapters._test_types import (
    AdapterEligibleMaxCursorSqlTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    (
        AdapterEligibleMaxCursorSqlTestCase(
            "duckdb date predicate",
            DuckDbAdapter(),
            'event"time',
            "2026-09-01",
            True,
            'SELECT MAX("event""time") FROM analytics.events '
            'WHERE "event""time" <= CAST(\'2026-09-01\' AS DATE)',
        ),
        AdapterEligibleMaxCursorSqlTestCase(
            "motherduck date predicate",
            MotherDuckAdapter(),
            'event"time',
            "2026-09-01",
            True,
            'SELECT MAX("event""time") FROM analytics.events '
            'WHERE "event""time" <= CAST(\'2026-09-01\' AS DATE)',
        ),
        AdapterEligibleMaxCursorSqlTestCase(
            "postgres date predicate",
            PostgresAdapter(),
            'event"time',
            "2026-09-01",
            True,
            'SELECT MAX("event""time") FROM analytics.events '
            'WHERE "event""time" <= CAST(\'2026-09-01\' AS DATE)',
        ),
        AdapterEligibleMaxCursorSqlTestCase(
            "bigquery date predicate",
            BigQueryAdapter(),
            "event`time",
            "2026-09-01",
            True,
            r"SELECT MAX(`event\`time`) FROM analytics.events "
            r"WHERE `event\`time` <= CAST('2026-09-01' AS DATE)",
        ),
        AdapterEligibleMaxCursorSqlTestCase(
            "sqlserver date predicate",
            SqlServerAdapter(),
            "event]time",
            "2026-09-01",
            True,
            "SELECT MAX([event]]time]) FROM analytics.events "
            "WHERE [event]]time] <= CAST('2026-09-01' AS DATE)",
        ),
        AdapterEligibleMaxCursorSqlTestCase(
            "snowflake date predicate",
            SnowflakeAdapter(),
            'event"time',
            "2026-09-01",
            True,
            'SELECT MAX("EVENT""TIME") FROM analytics.events '
            'WHERE "EVENT""TIME" <= CAST(\'2026-09-01\' AS DATE)',
        ),
        AdapterEligibleMaxCursorSqlTestCase(
            "databricks date predicate",
            DatabricksAdapter(),
            "event`time",
            "2026-09-01",
            True,
            "SELECT MAX(`event``time`) FROM analytics.events "
            "WHERE `event``time` <= CAST('2026-09-01' AS DATE)",
        ),
        AdapterEligibleMaxCursorSqlTestCase(
            "sqlserver timestamp predicate",
            SqlServerAdapter(),
            "event]time",
            "2026-09-01T12:00:00",
            False,
            "SELECT MAX([event]]time]) FROM analytics.events WHERE [event]]time] <= "
            "CAST('2026-09-01T12:00:00' AS DATETIME2(6))",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_cursor_horizon_when_rendering_eligible_max_then_adapter_owns_portable_sql(
    test_case: AdapterEligibleMaxCursorSqlTestCase,
) -> None:
    sql: str = test_case.adapter.render_max_cursor_at_or_before(
        relation="analytics.events",
        cursor_column=test_case.cursor_column,
        maximum_allowed=test_case.maximum_allowed,
        cursor_type="timestamp",
        is_date=test_case.is_date,
    )

    assert sql == test_case.expected_sql
