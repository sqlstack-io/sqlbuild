"""E2E coverage of macro argument dict keys that Python cannot hash."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    MacroDictKeyCompileE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_MACROS: str = (
    "def show(value):\n"
    '    """Show a value."""\n'
    "    return repr(str(value))\n\n\n"
    "def items():\n"
    '    """A list of items."""\n'
    "    return [1]\n\n\n"
    "def label():\n"
    '    """A label."""\n'
    '    return "orders"\n'
)
_HELP: str = (
    "help: Return a string, number, True, False, None, or a tuple of them from a macro used as a "
    "dict key"
)


@pytest.mark.parametrize(
    "test_case",
    [
        MacroDictKeyCompileE2ETestCase(
            description="a nested call returning a list as a dict key is a located P001",
            argument="{@py.items(): 2}",
            expected_exit_code=1,
            expected_fragments=(
                "error[P001]: Macro arguments of '@py.show'",
                "use the value of a nested macro call, which is a list, as a dict key at line 2, "
                "column 18",
                _HELP,
            ),
        ),
        MacroDictKeyCompileE2ETestCase(
            description="the unhashable call inside a tuple key is the one reported",
            argument="{(@py.label(), @py.items()): 2}",
            expected_exit_code=1,
            expected_fragments=("which is a list, as a dict key at line 2, column 32", _HELP),
        ),
        MacroDictKeyCompileE2ETestCase(
            description="a nested call returning a string is a valid dict key",
            argument="{@py.label(): 2}",
            expected_exit_code=0,
            expected_fragments=("0 errors",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_macro_dict_key_from_nested_call_when_compiling_then_unhashable_keys_are_located(
    test_case: MacroDictKeyCompileE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "shop"\nadapter = "duckdb"\n\n[connection]\ndatabase = "shop.duckdb"\n\n'
                "[scopes]\nenforce_placement = false\n"
            ),
            "sql/macros/python/values.py": _MACROS,
            "sql/models/orders.sql": (
                "MODEL (description 'Test model orders.');\n"
                f"SELECT @py.show({test_case.argument}) AS shown\n"
            ),
        },
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    output: str = compiled.stdout + compiled.stderr
    assert compiled.returncode == test_case.expected_exit_code, output
    assert "Traceback" not in output
    assert all(fragment in output for fragment in test_case.expected_fragments), output
