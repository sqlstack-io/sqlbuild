"""Tests for node result reads."""

from __future__ import annotations

import pytest

from sqlbuild.executor.node_results.main._read import read_node_results
from sqlbuild.executor.node_results.models import NodeResultEnvelope, NodeResultQuery
from tests.unit.src.sqlbuild.executor.node_results.main._test_types import NodeResultReadTestCase
from tests.unit.src.sqlbuild.executor.node_results.main.helpers import (
    NodeResultReadFakeResult,
    fake_relation_exists,
    fake_render_qualified_name,
)


@pytest.mark.parametrize(
    "test_case",
    [
        NodeResultReadTestCase(
            description="normalizes non dict metadata to empty dict",
            metadata_json_b64="WzFd",
            expected_metadata={},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_non_dict_metadata_when_reading_node_results_then_returns_empty_metadata(
    test_case: NodeResultReadTestCase,
) -> None:
    def execute(*, connection: object, sql: str) -> NodeResultReadFakeResult:
        del connection
        return NodeResultReadFakeResult(
            [
                (
                    "asset",
                    "produce_result",
                    None,
                    "main",
                    None,
                    "run_1",
                    "success",
                    "eyJ2YWx1ZSI6NDJ9",
                    test_case.metadata_json_b64,
                    None,
                    None,
                    "2026-01-01T00:00:00+00:00",
                )
            ]
        )

    results: tuple[NodeResultEnvelope, ...] = read_node_results(
        connection=object(),
        execute=execute,
        relation_exists=fake_relation_exists,
        database=None,
        schema="main",
        query=NodeResultQuery(
            node_type="asset",
            node_name="produce_result",
            target_database=None,
            target_schema="main",
            target_name=None,
            statuses=("success",),
            run_id=None,
            limit=1,
        ),
        render_qualified_name=fake_render_qualified_name,
    )

    assert len(results) == 1
    assert results[0].metadata == test_case.expected_metadata
