"""Integration tests for node result read/write operations."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import TargetSettings
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.node_results.classes.direct_store import DirectNodeResultStore
from sqlbuild.executor.node_results.main._direct_store import build_direct_node_result_store
from sqlbuild.executor.node_results.main._read import read_node_results
from sqlbuild.executor.node_results.main._write import write_node_result_record
from sqlbuild.executor.node_results.models import (
    NodeResultEnvelope,
    NodeResultQuery,
    NodeResultRecord,
)
from tests.integration.src.sqlbuild.executor.node_results.main._test_types import (
    EarlierNodeResultLocationIntegrationTestCase,
    NodeResultReadWriteIntegrationTestCase,
)
from tests.integration.src.sqlbuild.executor.node_results.main.helpers import task_result_record


@pytest.mark.parametrize(
    "test_case",
    [
        NodeResultReadWriteIntegrationTestCase(
            description="writes and reads node result through DuckDB",
            expected_payload={"value": 42},
            expected_metadata={"source": "integration"},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_node_result_record_when_writing_and_reading_then_round_trips_envelope(
    test_case: NodeResultReadWriteIntegrationTestCase,
    tmp_path: Path,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": str(tmp_path / "node_results.duckdb")})
    try:
        write_node_result_record(
            connection=connection,
            execute=adapter.execute,
            database=None,
            schema="main",
            record=NodeResultRecord(
                node_type="asset",
                node_name="produce_result",
                target_database=None,
                target_schema="main",
                target_name=None,
                run_id="run_1",
                status="success",
                payload=test_case.expected_payload,
                metadata=test_case.expected_metadata,
            ),
            render_qualified_name=adapter.render_qualified_name,
            render_framework_type=adapter.render_framework_type,
            render_create_index_sqls=adapter.render_create_node_result_index_sqls,
        )

        results: tuple[NodeResultEnvelope, ...] = read_node_results(
            connection=connection,
            execute=adapter.execute,
            relation_exists=adapter.relation_exists,
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
            render_qualified_name=adapter.render_qualified_name,
        )
    finally:
        adapter.close(connection)

    assert len(results) == 1
    assert results[0].payload == test_case.expected_payload
    assert results[0].metadata == test_case.expected_metadata


@pytest.mark.parametrize(
    "test_case",
    [
        EarlierNodeResultLocationIntegrationTestCase(
            description="a result only in the adapter default schema is read",
            earlier_schema="main",
            schema_preserved=False,
            new_run_ids=(),
            expected_run_id="run_old",
        ),
        EarlierNodeResultLocationIntegrationTestCase(
            description="a preserved target reads a result left in a schema named preserve",
            earlier_schema="preserve",
            schema_preserved=True,
            new_run_ids=(),
            expected_run_id="run_old",
        ),
        EarlierNodeResultLocationIntegrationTestCase(
            description="a target that is not preserved ignores a schema named preserve",
            earlier_schema="preserve",
            schema_preserved=False,
            new_run_ids=(),
            expected_run_id=None,
        ),
        EarlierNodeResultLocationIntegrationTestCase(
            description="a result in the store's own schema wins over an earlier one",
            earlier_schema="main",
            schema_preserved=False,
            new_run_ids=("run_new",),
            expected_run_id="run_new",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_result_in_earlier_location_when_reading_then_store_falls_back_only_when_empty(
    test_case: EarlierNodeResultLocationIntegrationTestCase,
    tmp_path: Path,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: Any = adapter.connect({"database": str(tmp_path / "node_results.duckdb")})
    try:
        earlier: DirectNodeResultStore = DirectNodeResultStore(
            adapter=adapter, connection=connection, database=None, schema=test_case.earlier_schema
        )
        earlier.write(task_result_record(run_id="run_old", schema=test_case.earlier_schema))
        store: DirectNodeResultStore = build_direct_node_result_store(
            adapter=adapter,
            connection=connection,
            database=None,
            schema="raw",
            target_config=TargetSettings(schema_preserved=test_case.schema_preserved),
        )
        run_id: str
        for run_id in test_case.new_run_ids:
            store.write(task_result_record(run_id=run_id, schema="raw"))
        reader: DirectNodeResultStore = build_direct_node_result_store(
            adapter=adapter,
            connection=connection,
            database=None,
            schema="raw",
            target_config=TargetSettings(schema_preserved=test_case.schema_preserved),
        )
        found: object = reader.result_of(node_type="asset", node_name="produce_result", default=None)
    finally:
        adapter.close(connection)

    assert getattr(found, "run_id", None) == test_case.expected_run_id
