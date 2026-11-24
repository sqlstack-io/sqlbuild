"""Integration test proving dependency ordering is maintained under concurrency."""

from __future__ import annotations

import threading
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import (
    CompilePipelineOptions,
    CompilePipelineResult,
    DiscoveredProjectInputs,
    PlanOutput,
    discover_project_inputs,
    run_compile_pipeline,
)
from sqlbuild.adapter.contract.types import TablePromotionMode
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.build.main._execute import execute_build_plan
from sqlbuild.executor.build.models import (
    BuildCallbacks,
    BuildExecutionResult,
    BuildRuntimeParams,
)
from sqlbuild.executor.build.types import BuildStatus
from tests.integration.src.sqlbuild.executor.build.concurrent._test_types import (
    OrderingInvariantTestCase,
)
from tests.integration.src.sqlbuild.executor.build.concurrent.helpers import (
    build_ordering_trace_callbacks,
    extract_upstream_model_deps,
    verify_ordering_invariant,
)

_PROJECT_YML: str = (
    'name = "demo"\n'
    'adapter = "duckdb"\n\n'
    "[connection]\n"
    'database = "test.duckdb"\n\n'
    "[settings]\n"
    'default_audit_severity = "error"\n'
)


@pytest.mark.parametrize(
    "test_case",
    [
        OrderingInvariantTestCase(
            description=(
                "diamond DAG never dispatches a node before all upstream model deps complete"
            ),
            max_concurrency=3,
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "sql/models/staging/stg_a.sql": (
                    "MODEL (description 'Test model stg_a.', materialized view);\n\nSELECT 1 AS id"
                ),
                "sql/models/staging/stg_b.sql": (
                    "MODEL (description 'Test model stg_b.', materialized view);\n\nSELECT 2 AS id"
                ),
                "sql/models/mid_a.sql": (
                    'MODEL (description "Test model mid_a.", materialized table);\n\nSELECT id FROM __ref("stg_a")'
                ),
                "sql/models/mid_b.sql": (
                    'MODEL (description "Test model mid_b.", materialized table);\n\nSELECT id FROM __ref("stg_b")'
                ),
                "sql/models/final.sql": (
                    "MODEL (description 'Test model final.', materialized table);\n\n"
                    "SELECT a.id AS a_id, b.id AS b_id "
                    'FROM __ref("mid_a") a '
                    'CROSS JOIN __ref("mid_b") b'
                ),
            },
            expected_upstream_model_deps=(
                ("stg_a", ()),
                ("stg_b", ()),
                ("mid_a", ("stg_a",)),
                ("mid_b", ("stg_b",)),
                ("final", ("mid_a", "mid_b")),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dag_when_building_concurrently_then_no_node_starts_before_deps_complete(
    test_case: OrderingInvariantTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    db_path: Path = tmp_path / "test.duckdb"
    config: dict[str, object] = {"database": str(db_path)}
    adapter: DuckDbAdapter = DuckDbAdapter()

    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    pipeline_result: CompilePipelineResult = run_compile_pipeline(
        discovered_inputs=discovered,
        adapter=adapter,
        options=CompilePipelineOptions(no_sql_validation=True, connection_config=config),
    )
    plan: PlanOutput = pipeline_result.plan_output

    completed_at_start: list[tuple[str, frozenset[str]]] = []
    completed_names: set[str] = set()
    lock: threading.Lock = threading.Lock()
    on_start: Any
    on_complete: Any
    on_start, on_complete = build_ordering_trace_callbacks(
        completed_at_start=completed_at_start,
        completed_names=completed_names,
        lock=lock,
    )

    scheduler_connection: Any = adapter.connect(config)
    worker_connections: list[Any] = []
    _i: int
    for _i in range(test_case.max_concurrency):
        worker_connections.append(adapter.connect(config))
    try:
        result: BuildExecutionResult = execute_build_plan(
            plan=plan,
            adapter=adapter,
            connection_config=config,
            connections=tuple(worker_connections),
            scheduler_connection=scheduler_connection,
            runtime=BuildRuntimeParams(
                promotion_mode=TablePromotionMode.STAGED,
                run_id="test_ordering",
                query_change_tracking=True,
            ),
            callbacks=BuildCallbacks(
                on_node_start=on_start,
                on_node_complete=on_complete,
            ),
        )
    finally:
        conn: Any
        for conn in worker_connections:
            adapter.close(conn)
        adapter.close(scheduler_connection)

    assert result.status == BuildStatus.SUCCESS

    upstream_model_deps: dict[str, frozenset[str]] = extract_upstream_model_deps(plan)

    expected_name: str
    expected_deps: tuple[str, ...]
    for expected_name, expected_deps in test_case.expected_upstream_model_deps:
        assert upstream_model_deps.get(expected_name, frozenset()) == frozenset(expected_deps), (
            f"upstream deps for {expected_name} do not match expected"
        )

    verify_ordering_invariant(
        completed_at_start=completed_at_start,
        upstream_model_deps=upstream_model_deps,
    )
