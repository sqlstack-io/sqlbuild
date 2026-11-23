"""E2E coverage that a build's completion line and exit code account for its Python checks."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    PythonCheckBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb
from tests.unit.src.sqlbuild.python_nodes.helpers import node_module_files


@pytest.mark.parametrize(
    "test_case",
    [
        PythonCheckBuildE2ETestCase(
            description="a failing check fails the build",
            check_decorator='@check(depends_on=model("fact_orders"))',
            check_body='return ctx.fail(message="orders present")',
            expected_exit_code=1,
            expected_footer_fragment="✗ Completed with errors  PASS=1  WARN=0  FAIL=1",
        ),
        PythonCheckBuildE2ETestCase(
            description="a check that raises fails the build",
            check_decorator='@check(depends_on=model("fact_orders"))',
            check_body='raise RuntimeError("boom")',
            expected_exit_code=1,
            expected_footer_fragment="✗ Completed with errors  PASS=1  WARN=0  FAIL=1",
        ),
        PythonCheckBuildE2ETestCase(
            description="a failing warn-severity check warns",
            check_decorator='@check(depends_on=model("fact_orders"), severity="warn")',
            check_body='return ctx.fail(message="orders present")',
            expected_exit_code=0,
            expected_footer_fragment="Completed with warnings  PASS=1  WARN=1  FAIL=0",
        ),
        PythonCheckBuildE2ETestCase(
            description="a passing check passes",
            check_decorator='@check(depends_on=model("fact_orders"))',
            check_body='return ctx.pass_(message="orders checked")',
            expected_exit_code=0,
            expected_footer_fragment="✓ Completed successfully  PASS=2  WARN=0  FAIL=0",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_check_when_building_then_completion_line_reports_its_outcome(
    test_case: PythonCheckBuildE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_check_outcomes",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_check_outcomes"\nadapter = "duckdb"\n\n'
                '[connection]\ndatabase = "warehouse.duckdb"\n\n[defaults]\nmaterialized = "table"\n'
            ),
            "sql/models/fact_orders.sql": (
                "MODEL (description 'Test model fact_orders.');\nSELECT 1 AS order_id\n"
            ),
            **node_module_files(
                "python/nodes/checks/orders.py",
                "from sqlbuild.checks import check\nfrom sqlbuild.refs import model\n\n\n"
                f"{test_case.check_decorator}\n"
                "def orders_checked(ctx):\n"
                "    '''Test check orders_checked.'''\n"
                f"    {test_case.check_body}\n",
            ),
        },
    )

    built: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    assert built.returncode == test_case.expected_exit_code, built.stdout + built.stderr
    assert test_case.expected_footer_fragment in built.stdout, built.stdout
