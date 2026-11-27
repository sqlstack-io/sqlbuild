"""E2E tests for helper CTEs used by expected and assertion SQL in `sqb test`."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import HelperScopeE2ETestCase
from tests.e2e.src.sqlbuild.cli.commands.main.test.helpers import build_helper_scope_project_files
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_OUTCOME_FRAGMENTS: tuple[str, ...] = (
    "PASS=4  FAIL=2  TOTAL=6",
    "expected rows of orders differ: 1 missing, 1 extra",
    "amount: expected 41, actual 40",
    "assertion all_expected_rows_built returned 1 row",
)


@pytest.mark.parametrize(
    "test_case",
    (
        HelperScopeE2ETestCase(
            description="helpers are in scope with sql analysis",
            command=("--no-color", "test"),
            expected_output_fragments=_OUTCOME_FRAGMENTS,
        ),
        HelperScopeE2ETestCase(
            description="helpers are in scope without sql analysis",
            command=("--no-color", "test", "--no-sql-analysis"),
            expected_output_fragments=_OUTCOME_FRAGMENTS,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_helper_ctes_when_expected_and_assertions_use_them_then_tests_compare_rows(
    test_case: HelperScopeE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="helper_scope_project",
        repo_files=build_helper_scope_project_files(),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )
    compile_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 1, output
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output
    assert "Catalog Error" not in output, output
    assert "unreachable" not in output, output
    assert compile_result.returncode == 0, compile_result.stdout + compile_result.stderr
    compiled_sql: str = next(
        (project_dir / "target" / "compiled" / "tests").rglob("helper_reads_mock_and_helper.sql")
    ).read_text(encoding="utf-8")
    assert compiled_sql.count("\n__helper__doubled AS (") == 1, compiled_sql
    assert compiled_sql.count("\n__helper__expected_rows AS (") == 1, compiled_sql
    assert "\ndoubled AS (" not in compiled_sql, compiled_sql
    assert "\nexpected_rows AS (" not in compiled_sql, compiled_sql
