from __future__ import annotations

import pytest

from sqlbuild.executor.scenario._helpers.local.sql import convert_sql_for_local_duckdb
from sqlbuild.executor.scenario.exceptions import LocalSqlRefusedError
from tests.unit.src.sqlbuild.executor.scenario._helpers._test_types import (
    LocalScenarioRefusalTestCase,
    LocalScenarioSqlTestCase,
)

_SCHEMA: dict[str, object] = {"relations": {"ORDERS": [["TOTAL", "NUMBER(38,0)"]]}}


@pytest.mark.parametrize(
    "test_case",
    [
        LocalScenarioSqlTestCase(
            description="DuckDB captures run their authored SQL without regeneration",
            source_dialect="duckdb",
            sql="select ifnull(total, 0) as total, list_value(1, 2) as ids from orders",
            expected_sql="select ifnull(total, 0) as total, list_value(1, 2) as ids from orders",
        ),
        LocalScenarioSqlTestCase(
            description="Snowflake captures are converted by SQLQuack",
            source_dialect="snowflake",
            sql="select IFNULL(total, 0) as total from orders",
            expected_sql=(
                'SELECT COALESCE(CAST("ORDERS"."TOTAL" AS DECIMAL(38,0)), '
                'CAST(0 AS DECIMAL(38,0))) AS "TOTAL" FROM "ORDERS"'
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_captured_dialect_when_preparing_local_scenario_sql_then_only_converts_snowflake(
    test_case: LocalScenarioSqlTestCase,
) -> None:
    result: str = convert_sql_for_local_duckdb(
        sql=test_case.sql,
        source_dialect=test_case.source_dialect,
        schema=_SCHEMA,
        time_zone=None,
        resource=("order_totals", "model", "order_totals"),
    ).sql

    assert result == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    [
        LocalScenarioRefusalTestCase(
            description="another adapter's SQL is refused with a dialect reason",
            source_dialect="postgres",
            expected_reasons=(("dialect", "postgres"),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_other_captured_dialect_when_preparing_local_scenario_sql_then_refuses_with_reason(
    test_case: LocalScenarioRefusalTestCase,
) -> None:
    with pytest.raises(LocalSqlRefusedError) as raised:
        convert_sql_for_local_duckdb(
            sql="select 1",
            source_dialect=test_case.source_dialect,
            schema=None,
            time_zone=None,
            resource=("order_totals", "model", "order_totals"),
        )

    assert (
        tuple((refusal["category"], refusal["construct"]) for refusal in raised.value.refusals)
        == test_case.expected_reasons
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
