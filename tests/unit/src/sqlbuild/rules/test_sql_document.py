"""Behavior tests for the syntax tree custom rules read through `SqlDocument`."""

import gc
import json
import time
from types import SimpleNamespace

import pytest

from sqlbuild.rules import SqlDocument, SqlTreeNode
from tests.unit.src.sqlbuild.rules._test_types import (
    ClauseTreeTestCase,
    CommonStructureKindTestCase,
    DeepTreeTestCase,
    FilePositionTimeLimitTestCase,
    UnplacedNodeTestCase,
)
from tests.unit.src.sqlbuild.rules.helpers import clause_children

_CLAUSES_SQL: str = (
    "SELECT o.order_id, o.total > 2, COUNT(*) AS c\n"
    "FROM orders o\n"
    "WHERE o.is_paid\n"
    "GROUP BY o.order_id, o.region\n"
    "HAVING COUNT(*) > 1\n"
    "QUALIFY ROW_NUMBER() OVER (PARTITION BY o.order_id ORDER BY o.placed_at) = 1\n"
    "ORDER BY o.order_id\n"
    "LIMIT 5"
)
_EVERY_CLAUSE: dict[str, tuple[str, ...]] = {
    "projection": ("column 1:8", "gt 1:20", "alias 1:33"),
    "from": ("table 2:6",),
    "where": ("column 3:7",),
    "group_by": ("column 4:10", "column 4:22"),
    "having": ("gt 5:8",),
    "qualify": ("eq 6:9",),
    "order_by": ("column 7:10",),
    "limit": ("literal 1:1",),
    "partition_by": ("column 6:41",),
    "window_order_by": ("column 6:61",),
}


@pytest.mark.parametrize(
    "test_case",
    [
        DeepTreeTestCase(
            description="document, select, projection, 100,000 additions and 100,001 literals",
            dialect="duckdb",
            terms=100_000,
            expected_node_count=200_004,
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
    file: str = "\n" * test_case.line_offset + source
    place: str = json.dumps(
        {"authored_query": {"range": [test_case.line_offset, len(file)]}, "passes": []}
    )
    document: SqlDocument = SqlDocument(
        source=source, dialect=test_case.dialect, place=place, file=file
    )

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
        place=json.dumps({"authored_query": {"range": [2, 10]}, "passes": []}),
        file="\n\nSELECT 1",
    )

    node: SimpleNamespace = SimpleNamespace(line=test_case.line, column=test_case.column)
    assert document.file_position(node=node) == test_case.expected_position


@pytest.mark.parametrize(
    "test_case",
    [
        ClauseTreeTestCase(
            description="Snowflake: every clause owns its expressions",
            dialect="snowflake",
            sql=_CLAUSES_SQL,
            expected_clauses=_EVERY_CLAUSE,
        ),
        ClauseTreeTestCase(
            description="DuckDB: every clause owns its expressions",
            dialect="duckdb",
            sql=_CLAUSES_SQL,
            expected_clauses=_EVERY_CLAUSE,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_query_when_walking_its_tree_then_each_clause_holds_its_expressions(
    test_case: ClauseTreeTestCase,
) -> None:
    document: SqlDocument = SqlDocument(source=test_case.sql, dialect=test_case.dialect)

    clauses: dict[str, tuple[str, ...]] = clause_children(
        document.ast(), test_case.expected_clauses
    )

    assert clauses == test_case.expected_clauses


@pytest.mark.parametrize(
    "test_case",
    [
        CommonStructureKindTestCase(
            description=f"{dialect}: common structures carry the tree's snake_case kinds",
            dialect=dialect,
            sql=(
                "WITH x AS (SELECT 1 AS id)\n"
                "SELECT x.*, (SELECT 2) AS two FROM x JOIN x AS y ON x.id = y.id"
            ),
            expected_kinds=("cte", "join", "subquery", "star", "cte"),
        )
        for dialect in ("snowflake", "duckdb")
    ],
    ids=lambda case: case.description,
)
def test_given_common_structures_when_reading_them_then_each_has_the_tree_s_kind(
    test_case: CommonStructureKindTestCase,
) -> None:
    document: SqlDocument = SqlDocument(source=test_case.sql, dialect=test_case.dialect)

    kinds: tuple[str, ...] = tuple(
        node.kind
        for node in (
            *document.ctes(),
            *document.joins(),
            *document.subqueries(),
            *document.star_projections(),
            *document.nodes(kind="CTE"),
        )
    )

    assert kinds == test_case.expected_kinds
