"""Unit coverage for the planner's SQL dialect: the adapter's syntax without warehouse access."""

from __future__ import annotations

import pytest

from sqlbuild._native import Dialect
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from tests.unit.src.sqlbuild.compiler.planner.classes._test_types import (
    DialectAttributeTestCase,
    DialectRefusalTestCase,
)

_ADAPTER: DuckDbAdapter = DuckDbAdapter()


@pytest.mark.parametrize(
    "test_case",
    [
        DialectAttributeTestCase(
            description="a render method is the adapter's own",
            attribute="render_qualified_name",
            expected_value=_ADAPTER.render_qualified_name,
        ),
        DialectAttributeTestCase(
            description="a dialect fact is the adapter's own",
            attribute="adapter_name",
            expected_value=_ADAPTER.adapter_name,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_syntax_attribute_when_read_through_dialect_then_matches_adapter(
    test_case: DialectAttributeTestCase,
) -> None:
    assert getattr(Dialect(_ADAPTER), test_case.attribute) == test_case.expected_value


@pytest.mark.parametrize(
    "test_case",
    [
        DialectRefusalTestCase(
            description="executing SQL",
            attribute="execute",
            expected_message="`execute` would reach the warehouse",
        ),
        DialectRefusalTestCase(
            description="listing relations",
            attribute="list_relations",
            expected_message="`list_relations` would reach the warehouse",
        ),
        DialectRefusalTestCase(
            description="opening a connection",
            attribute="connect",
            expected_message="`connect` would reach the warehouse",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_warehouse_attribute_when_read_through_dialect_then_refuses(
    test_case: DialectRefusalTestCase,
) -> None:
    with pytest.raises(AttributeError, match=test_case.expected_message):
        getattr(Dialect(_ADAPTER), test_case.attribute)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
