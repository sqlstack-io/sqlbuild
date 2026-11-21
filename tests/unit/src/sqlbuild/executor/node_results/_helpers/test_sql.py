"""Tests for node result SQL helpers."""

from __future__ import annotations

from datetime import datetime, timedelta, timezone

import pytest

from sqlbuild.executor.node_results._helpers.sql import build_insert_sql, build_read_history_sql
from sqlbuild.executor.node_results.models import NodeResultQuery, NodeResultRecord
from tests.unit.src.sqlbuild.executor.node_results._helpers._test_types import (
    NodeResultSqlTestCase,
)
from tests.unit.src.sqlbuild.executor.node_results._helpers.helpers import (
    render_test_qualified_name,
)


@pytest.mark.parametrize(
    "test_case",
    [
        NodeResultSqlTestCase(
            description="builds scoped read SQL with deterministic ordering",
            expected_fragments=(
                "FROM analytics._sqlbuild_node_results",
                "node_type = 'asset'",
                "node_name = 'produce_result'",
                "target_schema = 'analytics'",
                "status IN ('success')",
                "ORDER BY ts DESC, run_id DESC",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_node_result_read_request_when_building_sql_then_scopes_and_orders_rows(
    test_case: NodeResultSqlTestCase,
) -> None:
    sql: str = build_read_history_sql(
        database=None,
        schema="analytics",
        query=NodeResultQuery(
            node_type="asset",
            node_name="produce_result",
            target_database=None,
            target_schema="analytics",
            target_name=None,
            statuses=("success",),
            run_id=None,
            limit=1,
        ),
        render_qualified_name=render_test_qualified_name,
    )

    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in sql


@pytest.mark.parametrize(
    "test_case",
    [
        NodeResultSqlTestCase(
            description="representative node result insert",
            expected_sql=(
                "INSERT INTO analytics._sqlbuild_node_results (node_type, node_name, "
                "target_database, target_schema, target_name, run_id, status, "
                "payload_json_b64, metadata_json_b64, error_message, materialized, ts) VALUES "
                "('asset', 'node''name', NULL, 'analytics', NULL, 'run-1', 'success', 'e30=', "
                "'e30=', 'it''s fine', 'true', '2026-01-01T00:00:00+00:00')"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_representative_node_result_when_rendering_insert_then_sql_matches_golden(
    test_case: NodeResultSqlTestCase,
) -> None:
    sql: str = build_insert_sql(
        database=None,
        schema="analytics",
        record=NodeResultRecord(
            node_type="asset",
            node_name="node'name",
            target_database=None,
            target_schema="analytics",
            target_name=None,
            run_id="run-1",
            status="success",
            payload=None,
            error_message="it's fine",
            materialized=True,
            ts=datetime(2026, 1, 1, 1, tzinfo=timezone(timedelta(hours=1))),
        ),
        payload_json_b64="e30=",
        metadata_json_b64="e30=",
        render_qualified_name=render_test_qualified_name,
    )

    assert sql == test_case.expected_sql


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
