"""Execute SQL resolved through custom adapter hooks."""

from typing import Any

import pytest

from sqlbuild._native import Dialect, SourceColumnEntry, SourceEntry, resolve_source_references
from sqlbuild.adapter.contract.models import ColumnInfo
from tests.integration.src.sqlbuild.compiler.planner._helpers.resolve._test_types import (
    AdapterOverrideTestCase,
)
from tests.integration.src.sqlbuild.compiler.planner._helpers.resolve.helpers import (
    ResolutionOverrideAdapter,
)


@pytest.mark.parametrize(
    "test_case",
    [
        AdapterOverrideTestCase(
            description="custom source rendering preserves case and changes data",
            expected_sql=(
                'SELECT orderid FROM (SELECT CAST("OrderId" AS BIGINT) + 1 AS "orderid" '
                'FROM (SELECT 7 AS "OrderId"))'
            ),
            expected_rows=[(8,)],
        )
    ],
    ids=lambda case: case.description,
)
def test_given_custom_adapter_when_resolving_source_then_executes_overridden_rendering(
    test_case: AdapterOverrideTestCase,
    connection: Any,
) -> None:
    source: SourceEntry = SourceEntry(
        name="orders",
        expression='SELECT 1 AS "OrderId"',
        type_enforcement=True,
        columns=(SourceColumnEntry(name="orderid", type="BIGINT"),),
    )
    sql: str = resolve_source_references(
        query_sql='SELECT orderid FROM __source("orders")',
        source_map={"orders": source},
        source_warehouse_columns={"orders": (ColumnInfo(name="OrderId", type="INTEGER"),)},
        star_exclude_keyword="EXCLUDE",
        cursor_bounds=None,
        cursor_filter_inputs={},
        dialect=Dialect(ResolutionOverrideAdapter()),
        cursor_type=None,
        lower_bound_inclusive=True,
    )
    assert sql == test_case.expected_sql
    assert connection.execute(sql).fetchall() == test_case.expected_rows


if __name__ == "__main__":
    pytest.main([__file__])
