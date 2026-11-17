"""Unit coverage for the planner's SQL dialect: the adapter's syntax without warehouse access."""

from __future__ import annotations

import pytest

from sqlbuild._native import Dialect
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from tests.unit.src.sqlbuild.compiler.planner.classes._test_types import (
    DialectAttributeTestCase,
    DialectRefusalTestCase,
)

ATTRIBUTE_CASES: list[DialectAttributeTestCase] = [
    DialectAttributeTestCase(
        description="renders a qualified name",
        attribute="render_qualified_name",
        arguments={"database": "shop", "schema": "sales", "name": "orders"},
        expected=DuckDbAdapter().render_qualified_name(
            database="shop", schema="sales", name="orders"
        ),
    ),
    DialectAttributeTestCase(
        description="reports the adapter name",
        attribute="adapter_name",
        arguments={},
        expected=DuckDbAdapter().adapter_name,
    ),
]

REFUSAL_CASES: list[DialectRefusalTestCase] = [
    DialectRefusalTestCase(description="executing SQL", attribute="execute"),
    DialectRefusalTestCase(description="listing relations", attribute="list_relations"),
    DialectRefusalTestCase(description="opening a connection", attribute="connect"),
    DialectRefusalTestCase(description="an unknown attribute", attribute="no_such_attribute"),
]


@pytest.mark.parametrize(
    "case", ATTRIBUTE_CASES, ids=[case.description for case in ATTRIBUTE_CASES]
)
def test_given_syntax_attribute_when_read_through_dialect_then_matches_adapter(
    case: DialectAttributeTestCase,
) -> None:
    value: object = getattr(Dialect(DuckDbAdapter()), case.attribute)
    result: object = value(**case.arguments) if callable(value) else value
    assert result == case.expected


@pytest.mark.parametrize("case", REFUSAL_CASES, ids=[case.description for case in REFUSAL_CASES])
def test_given_warehouse_attribute_when_read_through_dialect_then_refuses(
    case: DialectRefusalTestCase,
) -> None:
    with pytest.raises(AttributeError, match=f"`{case.attribute}` would reach the warehouse"):
        getattr(Dialect(DuckDbAdapter()), case.attribute)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
