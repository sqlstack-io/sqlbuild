"""Keyed cursor delete_insert on Snowflake keeps its deletes and insert in one transaction."""

from __future__ import annotations

import pytest

from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from tests.unit.src.sqlbuild.adapters.snowflake._test_types import (
    SnowflakeKeyedCursorDeleteInsertTestCase,
)
from tests.unit.src.sqlbuild.adapters.snowflake.helpers import (
    RecordingSnowflakeAdapter,
)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeKeyedCursorDeleteInsertTestCase(
            description="DDL that commits stays outside the transaction around the deletes",
            unique_key=("order_id",),
            expected_prefixes=(
                "CREATE",
                "BEGIN",
                "DELETE FROM ANALYTICS.MARTS.ORDERS WHERE",
                "DELETE FROM ANALYTICS.MARTS.ORDERS USING",
                "INSERT",
                "COMMIT",
                "DROP",
            ),
            expected_inserted=3,
        ),
        SnowflakeKeyedCursorDeleteInsertTestCase(
            description="an unkeyed window keeps its delete and insert in one transaction",
            unique_key=(),
            expected_prefixes=(
                "BEGIN",
                "DELETE FROM ANALYTICS.MARTS.ORDERS WHERE",
                "INSERT",
                "COMMIT",
            ),
            expected_inserted=3,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cursor_window_when_replacing_then_snowflake_commits_deletes_with_insert(
    test_case: SnowflakeKeyedCursorDeleteInsertTestCase,
) -> None:
    adapter: RecordingSnowflakeAdapter = RecordingSnowflakeAdapter()

    inserted: int | None = adapter.delete_insert_cursor(
        connection=object(),
        destination="ANALYTICS.MARTS.ORDERS",
        sql="SELECT * FROM ANALYTICS.MARTS.ORDERS__DELTA",
        cursor_column="updated_at",
        cursor_start="2026-01-02",
        cursor_end="2026-01-03",
        statement_recorder=StatementRecorder(),
        unique_key=test_case.unique_key,
    )

    assert len(adapter.executed) == len(test_case.expected_prefixes)
    assert all(
        statement.startswith(prefix)
        for statement, prefix in zip(adapter.executed, test_case.expected_prefixes, strict=True)
    )
    assert inserted == test_case.expected_inserted
