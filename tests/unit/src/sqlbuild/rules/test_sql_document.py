"""Behavior tests for the syntax tree custom rules read through `SqlDocument`."""

import gc

import pytest

from sqlbuild.rules import SqlDocument, SqlTreeNode
from tests.unit.src.sqlbuild.rules._test_types import DeepTreeTestCase


@pytest.mark.parametrize(
    "test_case",
    [
        DeepTreeTestCase(
            description="document, select, 100,000 additions and 100,001 literals",
            dialect="duckdb",
            terms=100_000,
            expected_node_count=200_003,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_deep_query_when_reading_and_freeing_its_tree_then_the_process_survives(
    test_case: DeepTreeTestCase,
) -> None:
    document: SqlDocument | None = SqlDocument(
        source="SELECT " + "1 + " * test_case.terms + "1", dialect=test_case.dialect
    )
    assert document is not None
    tree: SqlTreeNode | None = document.ast()
    assert tree is not None
    walked: int = len(tree.walk())

    del tree, document
    _ = gc.collect()

    assert walked == test_case.expected_node_count
