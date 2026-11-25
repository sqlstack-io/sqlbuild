"""Declared relations a converted test reads itself, rather than through its fixtures."""

import pytest

from sqlbuild.executor.local._helpers.conversion import converted, warehouse_relations
from sqlbuild.executor.local.models import LocalConversion
from tests.unit.src.sqlbuild.executor.local._helpers._test_types import WarehouseRelationsTestCase
from tests.unit.src.sqlbuild.executor.local._helpers.helpers import ORDERS_SCHEMA_RELATIONS


@pytest.mark.parametrize(
    "test_case",
    [
        WarehouseRelationsTestCase(
            description="a fixture CTE named like a declared relation reads no warehouse relation",
            sql="WITH orders AS (SELECT 1 AS id) SELECT id FROM orders",
            expected_relations=(),
        ),
        WarehouseRelationsTestCase(
            description="a declared relation read by name beside a fixture is the warehouse's",
            sql=(
                "WITH orders AS (SELECT 1 AS id) "
                "SELECT orders.id FROM orders JOIN shop.raw.customers AS c ON c.id = orders.id"
            ),
            expected_relations=("SHOP.RAW.CUSTOMERS",),
        ),
        WarehouseRelationsTestCase(
            description="each declared relation read is named once",
            sql=(
                "SELECT o.id FROM shop.raw.orders AS o "
                "JOIN shop.raw.customers AS c ON c.id = o.id JOIN shop.raw.orders AS p ON p.id = o.id"
            ),
            expected_relations=("SHOP.RAW.CUSTOMERS", "SHOP.RAW.ORDERS"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_converted_test_sql_when_finding_warehouse_relations_then_fixtures_do_not_count(
    test_case: WarehouseRelationsTestCase,
) -> None:
    schema: dict[str, object] = {"relations": dict(ORDERS_SCHEMA_RELATIONS)}
    conversion: LocalConversion = converted(
        sql=test_case.sql, schema=schema, time_zone=None, path="test.sql"
    )

    relations: tuple[str, ...] = warehouse_relations(
        sql=test_case.sql, schema=schema, conversion=conversion
    )

    assert conversion.converted
    assert relations == test_case.expected_relations


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
