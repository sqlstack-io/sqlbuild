"""E2E coverage for `@loader` options that SQLLoad checks while `sqb compile` reads the project."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    LoaderOptionsCompileTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    (
        LoaderOptionsCompileTestCase(
            description="a merge loader without unique_key fails",
            loader_options="write_strategy='merge'",
            expected_exit_code=1,
            expected_stderr_fragment=(
                "error[D011]: python/nodes/loaders/raw_orders/loader.py @loader 'raw_orders': "
                'write_strategy = "merge" needs a unique_key'
            ),
        ),
        LoaderOptionsCompileTestCase(
            description="a merge loader with unique_key compiles",
            loader_options="write_strategy='merge', unique_key=['id']",
            expected_exit_code=0,
            expected_stderr_fragment="",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_loader_strategy_options_when_compiling_then_sqlload_checks_how_they_combine(
    test_case: LoaderOptionsCompileTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="loader_options",
        repo_files={
            "sqlbuild_project.toml": 'name = "loader_options"\nadapter = "duckdb"\n',
            "sql/sources/raw.yml": "sources:\n  - name: raw_orders\n    managed: true\n",
            "python/nodes/loaders/raw_orders/loader.py": (
                "from sqlbuild.loaders import loader\n\n\n"
                f"@loader({test_case.loader_options})\n"
                "def raw_orders(ctx):\n"
                "    '''Load the orders.'''\n"
                "    return [{'id': 1}]\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert test_case.expected_stderr_fragment in result.stderr, result.stderr
