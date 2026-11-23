"""Integration coverage for sources declared by selected Python nodes through the real pipeline."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pytest

from sqlbuild._native import CompilePipelineResult, SourceEntry
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from tests.integration.src.sqlbuild.compiler.pipeline._test_types import (
    PythonSourceReadPlanTestCase,
)
from tests.integration.src.sqlbuild.compiler.pipeline.helpers import (
    run_compile_pipeline_for_project,
)

_PROJECT_TOML: str = (
    'name = "python_reads"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
    '[connection]\ndatabase = ":memory:"\n\n'
    '[targets.dev]\nschema = "dev"\nloader_schema = "raw_dev"\n{target_config}\n'
    '[targets.prod]\nschema = "prod"\nloader_schema = "raw_prod"\n'
)
_PROJECT_FILES: dict[str, str] = {
    "sql/sources/raw.yml": (
        "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n    managed: true\n    write_strategy: table\n"
        "    columns:\n      - name: order_id\n        type: INTEGER\n"
    ),
    "python/nodes/loaders/raw.py": (
        "from sqlbuild.loaders import loader\n\n\n"
        "@loader\ndef raw_orders(ctx):\n    '''Test loader raw_orders.'''\n    return [{'order_id': 1}]\n"
    ),
    "sql/models/order_marker.sql": "MODEL (description 'Test model order_marker.', materialized table);\nSELECT 1 AS n\n",
    "python/nodes/tasks/count.py": (
        "from sqlbuild.refs import source\nfrom sqlbuild.assets import asset\n\n\n"
        '@asset(depends_on=source("raw_orders"))\n'
        "def count_task(ctx):\n"
        "    '''Test task count_task.'''\n    ctx.query(f\"SELECT count(*) FROM {ctx.relation(source('raw_orders'))}\")\n"
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        PythonSourceReadPlanTestCase(
            description="deferred source read resolves to the deferral target",
            target_config='defer_sources_to = "prod"\n',
            expected_schema="raw_prod",
        ),
        PythonSourceReadPlanTestCase(
            description="undeferred source read resolves to the active target",
            target_config="",
            expected_schema="raw_dev",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_only_a_task_selected_when_planning_then_its_source_resolves_like_a_sql_read(
    test_case: PythonSourceReadPlanTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": _PROJECT_TOML.format(target_config=test_case.target_config),
            **_PROJECT_FILES,
        },
    )

    result: CompilePipelineResult = run_compile_pipeline_for_project(
        project_dir=tmp_path,
        adapter=DuckDbAdapter(),
        select=("asset:count_task",),
        resolve_python_run_selectors=True,
    )

    source: SourceEntry = result.plan_output.python_source_entries["raw_orders"]
    assert source.schema == test_case.expected_schema
    assert "raw_orders" not in result.plan_output.source_map
