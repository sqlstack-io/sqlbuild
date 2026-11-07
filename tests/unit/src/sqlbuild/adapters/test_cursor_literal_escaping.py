"""Cursor bounds reach SQL through each dialect's string escaping, whatever their text holds."""

from __future__ import annotations

import pytest

from sqlbuild.adapter.contract.classes.strict_adapter import StrictAdapter
from sqlbuild.adapters.bigquery.classes.bigquery_adapter import BigQueryAdapter
from sqlbuild.adapters.clickhouse.classes.clickhouse_adapter import ClickHouseAdapter
from sqlbuild.adapters.databricks.classes.databricks_adapter import DatabricksAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.motherduck.classes.motherduck_adapter import MotherDuckAdapter
from sqlbuild.adapters.postgres.classes.postgres_adapter import PostgresAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.adapters.sqlserver.classes.sqlserver_adapter import SqlServerAdapter
from tests.unit.src.sqlbuild.adapters._test_types import AdapterCursorLiteralEscapingTestCase

BOUND: str = "o'k\\x\ny"


@pytest.mark.parametrize(
    "test_case",
    (
        AdapterCursorLiteralEscapingTestCase(
            "duckdb doubles quotes", DuckDbAdapter(), "'o''k\\x\ny'", "'o''k\\x\ny'"
        ),
        AdapterCursorLiteralEscapingTestCase(
            "motherduck doubles quotes", MotherDuckAdapter(), "'o''k\\x\ny'", "'o''k\\x\ny'"
        ),
        AdapterCursorLiteralEscapingTestCase(
            "postgres doubles quotes", PostgresAdapter(), "'o''k\\x\ny'", "'o''k\\x\ny'"
        ),
        AdapterCursorLiteralEscapingTestCase(
            "sqlserver doubles quotes", SqlServerAdapter(), "'o''k\\x\ny'", "'o''k\\x\ny'"
        ),
        AdapterCursorLiteralEscapingTestCase(
            "snowflake doubles quotes and escapes backslashes",
            SnowflakeAdapter(),
            "'o''k\\\\x\ny'",
            "'o''k\\\\x\ny'",
        ),
        AdapterCursorLiteralEscapingTestCase(
            "clickhouse backslash-escapes",
            ClickHouseAdapter(),
            "'o\\'k\\\\x\ny'",
            "'o\\'k\\\\x\ny'",
        ),
        AdapterCursorLiteralEscapingTestCase(
            "databricks backslash-escapes",
            DatabricksAdapter(),
            "'o\\'k\\\\x\ny'",
            "TIMESTAMP 'o\\'k\\\\x\ny'",
        ),
        AdapterCursorLiteralEscapingTestCase(
            "bigquery backslash-escapes line breaks too",
            BigQueryAdapter(),
            "'o\\'k\\\\x\\ny'",
            "TIMESTAMP 'o\\'k\\\\x\\ny'",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_cursor_bound_with_quote_backslash_and_newline_when_rendering_then_escapes_it(
    test_case: AdapterCursorLiteralEscapingTestCase,
) -> None:
    adapter: StrictAdapter = test_case.adapter

    assert adapter.render_cursor_bound_literal(value=BOUND, cursor_type=None) == (
        test_case.expected_literal
    )
    assert test_case.expected_literal in adapter.render_cursor_bound_literal(
        value=BOUND, cursor_type="timestamp"
    )
    max_sql: str = adapter.render_max_cursor_at_or_before(
        relation="analytics.events",
        cursor_column="updated_at",
        maximum_allowed=BOUND,
        cursor_type="timestamp",
        is_date=True,
    )
    assert test_case.expected_literal in max_sql
    delete_sql: str = "\n".join(
        adapter.render_delete_insert_cursor(
            destination="analytics.events",
            sql="SELECT 1",
            cursor_column="updated_at",
            cursor_start=BOUND,
            cursor_end=BOUND,
        )
    )
    assert delete_sql.count(test_case.expected_delete_bound) == 2


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
