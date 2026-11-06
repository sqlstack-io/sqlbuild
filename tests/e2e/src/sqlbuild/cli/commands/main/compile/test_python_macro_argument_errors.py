"""E2E coverage that `@py` arguments are read as Python, not with Snowflake's line comments."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    PythonMacroArgumentErrorTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    build_offline_snowflake_project_toml,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_ORDER_MACRO: str = (
    '"""Order macros."""\n\n\n'
    "def order_total(value: int) -> str:\n"
    '    """Return the order total literal."""\n'
    "    return str(value)\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonMacroArgumentErrorTestCase(
            description="floor division is not a literal, not a comment",
            call_sql="@py.order_total(10 // 3)",
            expected_message="must use only Python literals",
        ),
        PythonMacroArgumentErrorTestCase(
            description="a Python comment in a multi-line call is not a comment",
            call_sql="@py.order_total(\n  3  # it's three\n)",
            expected_message="a quoted string in @py.order_total is not closed",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_python_macro_arguments_when_compiling_then_error_names_the_argument_problem(
    test_case: PythonMacroArgumentErrorTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="order_totals",
        repo_files={
            "sqlbuild_project.toml": build_offline_snowflake_project_toml(),
            "models/_macros/python/order_total.py": _ORDER_MACRO,
            "models/order_totals.sql": (
                f'MODEL (description "Order totals.");\nSELECT {test_case.call_sql} AS total\n'
            ),
        },
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    assert compiled.returncode != 0, compiled.stdout + compiled.stderr
    assert test_case.expected_message in compiled.stdout + compiled.stderr, (
        compiled.stdout + compiled.stderr
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
