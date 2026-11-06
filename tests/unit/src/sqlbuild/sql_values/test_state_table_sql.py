"""Golden SQL tests for native append-only state table rendering."""

from __future__ import annotations

from datetime import datetime, timedelta, timezone

import pytest

from sqlbuild._native import (
    SourceFreshnessRecord,
    StateSqlValueType,
    render_insert_source_freshness_records_sql,
    render_state_table_create_sql,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from tests.unit.src.sqlbuild.sql_values._test_types import (
    StateSqlGoldenTestCase,
    StateSqlTypeErrorTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        StateSqlGoldenTestCase(
            description="representative source freshness insert",
            expected_sql=(
                "INSERT INTO analytics._sqlbuild_source_freshness (source_name, "
                "target_database, target_schema, target_name, run_id, strategy, value_kind, "
                "data_version, data_version_hash, observed_at, event_id) VALUES ('raw.o''rders', NULL, "
                "'raw', 'orders', 'run-1', 'adapter', 'timestamp', "
                "'2026-01-01T01:00:00+01:00', 'hash', '2026-01-01T00:00:00+00:00', "
                "'a7a96057679375d1ba89ff2842c691f288d410dc1f502726feeb9cc0675bfd26')"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_representative_freshness_record_when_rendering_then_sql_matches_golden(
    test_case: StateSqlGoldenTestCase,
) -> None:
    sql: str = render_insert_source_freshness_records_sql(
        database=None,
        schema="analytics",
        records=(
            SourceFreshnessRecord(
                source_name="raw.o'rders",
                target_database=None,
                target_schema="raw",
                target_name="orders",
                run_id="run-1",
                strategy="adapter",
                value_kind="timestamp",
                data_version="2026-01-01T01:00:00+01:00",
                data_version_hash="hash",
                observed_at=datetime(2026, 1, 1, 1, tzinfo=timezone(timedelta(hours=1))),
            ),
        ),
        render_qualified_name=DuckDbAdapter().render_qualified_name,
    )

    assert sql == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    [
        StateSqlGoldenTestCase(
            description="string integer and text timestamp columns with required columns",
            expected_sql=(
                "CREATE TABLE IF NOT EXISTS main.state "
                "(name VARCHAR NOT NULL, row_count BIGINT, observed_at TIMESTAMP NOT NULL)"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_state_columns_when_rendering_table_ddl_then_sql_matches_golden(
    test_case: StateSqlGoldenTestCase,
) -> None:
    sql: str = render_state_table_create_sql(
        qualified_name="main.state",
        columns=("name", "row_count", "observed_at"),
        column_types={
            "name": StateSqlValueType.STRING,
            "row_count": StateSqlValueType.INTEGER,
            "observed_at": StateSqlValueType.TEXT_TIMESTAMP,
        },
        required_columns=frozenset({"name", "observed_at"}),
        render_framework_type=DuckDbAdapter().render_framework_type,
    )

    assert sql == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    [
        StateSqlTypeErrorTestCase(
            description="boolean columns are not state table columns",
            column_type=StateSqlValueType.BOOLEAN,
            expected_error=KeyError,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unsupported_column_type_when_rendering_table_ddl_then_raises_error(
    test_case: StateSqlTypeErrorTestCase,
) -> None:
    with pytest.raises(test_case.expected_error):
        render_state_table_create_sql(
            qualified_name="main.state",
            columns=("flag",),
            column_types={"flag": test_case.column_type},
            required_columns=frozenset(),
            render_framework_type=DuckDbAdapter().render_framework_type,
        )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
