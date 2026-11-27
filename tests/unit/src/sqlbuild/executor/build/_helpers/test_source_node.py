"""Unit tests for build source-load node execution helper."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import CompiledObjectKey, CompiledResourceType, PlanOutput
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.build._helpers.source_node import execute_build_source_node
from sqlbuild.executor.build.models import BuildCallbacks, BuildRuntimeParams
from sqlbuild.executor.load.models import LoadExecutionResult
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.runtime.contracts.types import ExecutionResourceKind
from tests.unit.src.sqlbuild.executor.build._helpers._test_types import (
    BuildSourceNodeExecutionTestCase,
    BuildSourceNodeResultLocationTestCase,
)
from tests.unit.src.sqlbuild.executor.build._helpers.helpers import (
    build_discovered_source_loader,
    build_loader_projects,
    build_source_load_plan_output,
)


@pytest.mark.parametrize(
    "test_case",
    [
        BuildSourceNodeExecutionTestCase(
            description="executes source load and records callbacks",
            source_name="raw_orders",
            loader_name="raw_orders_loader",
            expected_progress_event="source: raw_orders",
            expected_start_event=("raw_orders", ExecutionResourceKind.SOURCE),
            expected_status=ExecutionStatus.SUCCESS,
            expected_rows=((1, "loaded"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_build_source_node_when_executing_then_runs_loader_runtime(
    test_case: BuildSourceNodeExecutionTestCase,
    tmp_path: Path,
) -> None:
    progress_events: list[str] = []
    start_events: list[tuple[str, ExecutionResourceKind]] = []
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: object = adapter.connect({"database": str(tmp_path / "source_node.duckdb")})
    plan: PlanOutput = build_source_load_plan_output(
        source_name=test_case.source_name,
        loader_name=test_case.loader_name,
    )
    try:
        result: LoadExecutionResult = execute_build_source_node(
            key=CompiledObjectKey(CompiledResourceType.SOURCE, test_case.source_name),
            plan=plan,
            loader_functions_by_name={
                test_case.loader_name: build_discovered_source_loader(
                    loader_name=test_case.loader_name
                )
            },
            loader_ref_entries={},
            adapter=adapter,
            connection_config={"database": str(tmp_path / "source_node.duckdb")},
            connection=connection,
            runtime=BuildRuntimeParams(
                run_id="run-1",
                target="dev",
                effective_vars={"region": "west"},
                loader_projects=build_loader_projects(loader_name=test_case.loader_name),
            ),
            callbacks=BuildCallbacks(
                on_progress=progress_events.append,
                on_node_start=lambda name, *, resource_kind: start_events.append(
                    (name, resource_kind)
                ),
            ),
        )
        rows: tuple[tuple[object, ...], ...] = tuple(
            connection.execute("SELECT id, status FROM raw_orders ORDER BY id").fetchall()
        )
    finally:
        adapter.close(connection)

    assert result.status == test_case.expected_status
    assert result.rows_loaded == len(test_case.expected_rows)
    assert result.duration_ms is not None
    assert rows == test_case.expected_rows
    assert progress_events == [test_case.expected_progress_event]
    assert start_events == [test_case.expected_start_event]


@pytest.mark.parametrize(
    "test_case",
    [
        BuildSourceNodeResultLocationTestCase(
            description="an adapter without a default schema still records the result",
            adapter_default_schema=None,
            runtime_schema="analytics",
            expected_result_schemas=("analytics",),
        ),
        BuildSourceNodeResultLocationTestCase(
            description="the runtime location wins over the adapter default schema",
            adapter_default_schema="main",
            runtime_schema="analytics",
            expected_result_schemas=("analytics",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_runtime_location_when_executing_source_node_then_result_is_written_there(
    test_case: BuildSourceNodeResultLocationTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    monkeypatch.setattr(adapter, "default_schema", lambda: test_case.adapter_default_schema)
    connection: object = adapter.connect({"database": str(tmp_path / "source_node.duckdb")})
    try:
        result: LoadExecutionResult = execute_build_source_node(
            key=CompiledObjectKey(CompiledResourceType.SOURCE, "raw_orders"),
            plan=build_source_load_plan_output(
                source_name="raw_orders", loader_name="raw_orders_loader"
            ),
            loader_functions_by_name={
                "raw_orders_loader": build_discovered_source_loader(loader_name="raw_orders_loader")
            },
            loader_ref_entries={},
            adapter=adapter,
            connection_config={"database": str(tmp_path / "source_node.duckdb")},
            connection=connection,
            runtime=BuildRuntimeParams(
                run_id="run-1",
                target="dev",
                default_schema=test_case.runtime_schema,
                loader_projects=build_loader_projects(loader_name="raw_orders_loader"),
            ),
            callbacks=BuildCallbacks(),
        )
        result_schemas: tuple[str, ...] = tuple(
            row[0]
            for row in connection.execute(
                "SELECT table_schema FROM information_schema.tables "
                "WHERE table_name = '_sqlbuild_node_results' ORDER BY ALL"
            ).fetchall()
        )
        recorded: list[tuple[object, ...]] = connection.execute(
            f"SELECT node_type, target_schema, run_id "
            f"FROM {test_case.runtime_schema}._sqlbuild_node_results"
        ).fetchall()
    finally:
        adapter.close(connection)

    assert result.status == ExecutionStatus.SUCCESS
    assert result_schemas == test_case.expected_result_schemas
    assert recorded == [("loader", test_case.runtime_schema, "run-1")]
