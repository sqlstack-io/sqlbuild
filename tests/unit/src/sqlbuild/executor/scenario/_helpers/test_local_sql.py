from __future__ import annotations

import pytest

from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.scenario._helpers.local.sql import convert_sql_for_local_duckdb
from sqlbuild.executor.scenario.constants import SCENARIO_LOCAL_SQL_TRANSPILE_FAILED
from sqlbuild.executor.scenario.types import LocalSqlColumns
from tests.unit.src.sqlbuild.executor.scenario._helpers._test_types import (
    LocalScenarioSqlRefusalTestCase,
    LocalScenarioSqlTestCase,
)

_SCHEMA: dict[str, LocalSqlColumns] = {
    "__SQB_LOCAL__SOURCE__ORDERS": (("ID", "NUMBER(38,0)"), ("TOTAL", "NUMBER(10,2)")),
}


@pytest.mark.parametrize(
    "test_case",
    [
        LocalScenarioSqlTestCase(
            description="DuckDB captures run their authored SQL without regeneration",
            source_dialect="duckdb",
            sql="select ifnull(total, 0) as total, list_value(1, 2) as ids from orders",
            expected_sql_fragments=(
                "select ifnull(total, 0) as total, list_value(1, 2) as ids from orders",
            ),
            expected_columns=(),
        ),
        LocalScenarioSqlTestCase(
            description="MotherDuck captures are DuckDB SQL and run as authored",
            source_dialect="motherduck",
            sql="select ifnull(total, 0) as total from orders",
            expected_sql_fragments=("select ifnull(total, 0) as total from orders",),
            expected_columns=(),
        ),
        LocalScenarioSqlTestCase(
            description="Snowflake captures convert with SQLQuack against the local schema",
            source_dialect="snowflake",
            sql="select IFNULL(total, 0) as total from __sqb_local__source__orders",
            expected_sql_fragments=("COALESCE(", '"__SQB_LOCAL__SOURCE__ORDERS"'),
            expected_columns=(("TOTAL", "NUMBER(10,2)"),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_captured_dialect_when_preparing_local_scenario_sql_then_only_converts_snowflake(
    test_case: LocalScenarioSqlTestCase,
) -> None:
    sql, columns = convert_sql_for_local_duckdb(
        sql=test_case.sql,
        source_dialect=test_case.source_dialect,
        schema=_SCHEMA,
        scenario_name="order_totals",
        resource_kind="model",
        resource_name="order_totals",
    )

    for fragment in test_case.expected_sql_fragments:
        assert fragment in sql
    assert columns == test_case.expected_columns


@pytest.mark.parametrize(
    "test_case",
    [
        LocalScenarioSqlRefusalTestCase(
            description="a Snowflake statement outside SQLQuack's default policy is refused",
            source_dialect="snowflake",
            sql="select convert_timezone('UTC', current_timestamp()) as converted_at",
            expected_message_fragments=(
                "model 'order_totals'",
                "SQLQuack refuses it",
                "error[SQ0008]",
                "session_timezone_utc",
            ),
        ),
        LocalScenarioSqlRefusalTestCase(
            description="a captured dialect SQLQuack does not convert has no local execution",
            source_dialect="postgres",
            sql="select coalesce(total, 0) as total from orders",
            expected_message_fragments=("local execution is unavailable for postgres",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unconvertible_sql_when_preparing_local_scenario_sql_then_refuses_with_x607(
    test_case: LocalScenarioSqlRefusalTestCase,
) -> None:
    with pytest.raises(ExecutorInputError) as raised:
        convert_sql_for_local_duckdb(
            sql=test_case.sql,
            source_dialect=test_case.source_dialect,
            schema=_SCHEMA,
            scenario_name="order_totals",
            resource_kind="model",
            resource_name="order_totals",
        )

    assert raised.value.code == SCENARIO_LOCAL_SQL_TRANSPILE_FAILED
    for fragment in test_case.expected_message_fragments:
        assert fragment in str(raised.value)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
