from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pytest

from sqlbuild._native import CompilePipelineResult
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from tests.integration.src.sqlbuild.compiler.pipeline._test_types import (
    SelectionLineageIntegrationTestCase,
)
from tests.integration.src.sqlbuild.compiler.pipeline.helpers import (
    run_compile_pipeline_for_project,
)

_PROJECT_TOML: str = 'name = "demo"\nadapter = "duckdb"\n\n[connection]\ndatabase = ":memory:"\n'

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": _PROJECT_TOML,
    "sql/sources/raw.yml": (
        "sources:\n"
        "  - name: raw_payments\n    description: Test source raw_payments.\n"
        "    expression: SELECT 1 AS payment_id\n"
        "  - name: raw_orders\n    description: Test source raw_orders.\n"
        "    expression: SELECT 1 AS order_id\n"
    ),
    "sql/models/stg_payments.sql": (
        "MODEL (description 'Test model stg_payments.', materialized table, audits [orders_backed]);\n\n"
        'SELECT payment_id FROM __source("raw_payments")'
    ),
    "sql/audits/generic/orders_backed.sql": (
        "AUDIT ();\n\n"
        "SELECT m.*\n"
        'FROM __ref("@model") m\n'
        'LEFT JOIN __source("raw_orders") o ON 1 = 1\n'
        "WHERE o.order_id IS NULL"
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        SelectionLineageIntegrationTestCase(
            description="upstream selection follows lineage; audit scope-deps stay unselected",
            project_files=_PROJECT_FILES,
            select=("+stg_payments",),
            expected_selected_names=frozenset({"stg_payments", "raw_payments"}),
            expected_unselected_names=frozenset({"raw_orders"}),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_audit_scope_deps_when_selecting_upstream_then_selection_follows_lineage(
    test_case: SelectionLineageIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    result: CompilePipelineResult = run_compile_pipeline_for_project(
        project_dir=tmp_path,
        adapter=DuckDbAdapter(),
        select=test_case.select,
    )

    selected_names: frozenset[str] = frozenset(key.name for key in result.plan_output.selected_keys)
    assert test_case.expected_selected_names <= selected_names
    assert not (test_case.expected_unselected_names & selected_names)


@pytest.mark.parametrize(
    "test_case",
    [
        SelectionLineageIntegrationTestCase(
            description="planner path and unified python path select identical keys",
            project_files=_PROJECT_FILES,
            select=("+stg_payments",),
            expected_selected_names=frozenset({"stg_payments", "raw_payments"}),
            expected_unselected_names=frozenset({"raw_orders"}),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_pure_sql_selection_when_planning_with_and_without_python_then_keys_match(
    test_case: SelectionLineageIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    planner_result: CompilePipelineResult = run_compile_pipeline_for_project(
        project_dir=tmp_path,
        adapter=DuckDbAdapter(),
        select=test_case.select,
        resolve_python_run_selectors=False,
    )
    unified_result: CompilePipelineResult = run_compile_pipeline_for_project(
        project_dir=tmp_path,
        adapter=DuckDbAdapter(),
        select=test_case.select,
        resolve_python_run_selectors=True,
    )

    assert planner_result.plan_output.selected_keys == unified_result.plan_output.selected_keys
    assert test_case.expected_selected_names <= frozenset(
        key.name for key in unified_result.plan_output.selected_keys
    )
