"""Behavior tests for the syntax tree custom rules read through `SqlDocument`."""

import gc
import json
import time
from types import SimpleNamespace

import pytest

from sqlbuild.rules import SqlDocument, SqlTreeNode
from tests.unit.src.sqlbuild.rules._test_types import (
    DeepTreeTestCase,
    FilePositionTimeLimitTestCase,
    UnplacedNodeTestCase,
)


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


@pytest.mark.parametrize(
    "test_case",
    [
        FilePositionTimeLimitTestCase(
            description="20,000 columns, one per line, five header lines",
            dialect="duckdb",
            columns=20_000,
            line_offset=5,
            expected_max_seconds=2.0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_wide_model_query_when_placing_every_column_then_time_stays_linear(
    test_case: FilePositionTimeLimitTestCase,
) -> None:
    source: str = (
        "SELECT\n" + ",\n".join(f"  c{index}" for index in range(test_case.columns)) + "\nFROM t"
    )
    place: str = json.dumps(
        {
            "authored_sql": source,
            "line_offset": test_case.line_offset,
            "column_offset": 0,
            "authored_query": None,
        }
    )
    document: SqlDocument = SqlDocument(source=source, dialect=test_case.dialect, place=place)

    started: float = time.perf_counter()
    positions: list[tuple[int, int] | None] = [
        document.file_position(node=node) for node in document.nodes(kind="Column")
    ]

    assert time.perf_counter() - started < test_case.expected_max_seconds
    assert positions == [
        (index + 2 + test_case.line_offset, 3) for index in range(test_case.columns)
    ]


@pytest.mark.parametrize(
    "test_case",
    [
        UnplacedNodeTestCase(
            description="integer line and column",
            line=1,
            column=8,
            expected_position=(3, 8),
        ),
        UnplacedNodeTestCase(
            description="no line or column", line=None, column=None, expected_position=None
        ),
        UnplacedNodeTestCase(
            description="text line and column", line="1", column="8", expected_position=None
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_node_position_when_placing_it_then_only_integers_map_into_the_file(
    test_case: UnplacedNodeTestCase,
) -> None:
    document: SqlDocument = SqlDocument(
        source="SELECT 1",
        dialect="duckdb",
        place=json.dumps(
            {
                "authored_sql": "SELECT 1",
                "line_offset": 2,
                "column_offset": 0,
                "authored_query": None,
            }
        ),
    )

    node: SimpleNamespace = SimpleNamespace(line=test_case.line, column=test_case.column)
    assert document.file_position(node=node) == test_case.expected_position
