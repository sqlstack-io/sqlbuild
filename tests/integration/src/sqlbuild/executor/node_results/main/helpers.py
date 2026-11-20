"""Shared builders for node result integration tests."""

from __future__ import annotations

from sqlbuild.executor.node_results.models import NodeResultRecord


def task_result_record(*, run_id: str, schema: str) -> NodeResultRecord:
    """A successful `produce_result` task result recorded in `schema`."""

    return NodeResultRecord(
        node_type="asset",
        node_name="produce_result",
        target_database=None,
        target_schema=schema,
        target_name=None,
        run_id=run_id,
        status="success",
        payload={"run": run_id},
        metadata={},
    )
