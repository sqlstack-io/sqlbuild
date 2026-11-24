"""CLI e2e coverage for `sqb mv` rewriting Python imports of the Python files it moves."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.refactor._test_types import (
    PythonImportMoveE2ETestCase,
    PythonImportRefusalE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.refactor.helpers import (
    CENTS_CHECK,
    cents_check,
    fragments_present,
    project_text,
    sqb,
    write_orders_project,
)

_MOVED: str = "models.finance._sqlbuild._macros.python.cents"


@pytest.mark.parametrize(
    "test_case",
    [
        PythonImportMoveE2ETestCase(
            description="direct, aliased and grouped imports of a carried macro module",
            files={
                **cents_check(
                    "from sql.models.staging._sqlbuild._macros.python.cents import to_cents\n"
                    "import sql.models.staging._sqlbuild._macros.python.cents as cents_module\n"
                    "from sql.models.staging._sqlbuild._macros.python.cents import (\n    to_cents as cents,\n)",
                    body='    assert to_cents("1") == cents_module.to_cents("1") == cents("1")\n',
                ),
            },
            expected_fragments={
                CENTS_CHECK: (
                    f"from {_MOVED} import to_cents\n",
                    f"import {_MOVED} as cents_module\n",
                    f"from {_MOVED} import (\n    to_cents as cents,\n)",
                ),
            },
        ),
        PythonImportMoveE2ETestCase(
            description="a relative import from a macro module that stays",
            files={
                **cents_check(
                    "from sql.models.staging._sqlbuild._macros.python import cents as cents_module",
                    body='    assert cents_module.to_cents("1")\n',
                ),
                "sql/models/staging/_sqlbuild/_macros/python/totals.py": (
                    "from .cents import to_cents\n\n\n"
                    "def order_total(expression: str) -> str:\n"
                    '    return f"SUM({to_cents(expression)})"\n'
                ),
                "sql/models/staging/stg_order_totals.sql": (
                    "MODEL (description 'Test model stg_order_totals.',\n  materialized view,\n);\n\n"
                    'SELECT\n  @py.order_total("amount") AS total_cents\nFROM __ref("stg_orders")\n'
                ),
            },
            expected_fragments={
                CENTS_CHECK: ("from sql.macros.python import cents as cents_module",),
                "sql/models/staging/_sqlbuild/_macros/python/totals.py": (
                    "from sql.macros.python.cents import to_cents\n",
                ),
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_imports_of_a_moved_macro_when_moving_then_imports_follow_and_project_builds(
    tmp_path: Path, test_case: PythonImportMoveE2ETestCase
) -> None:
    project_dir: Path = write_orders_project(tmp_path=tmp_path, files=test_case.files)

    moved: subprocess.CompletedProcess[str] = sqb(
        project_dir, "mv", "stg_order_cents", "sql/models/finance/"
    )
    compiled: subprocess.CompletedProcess[str] = sqb(project_dir, "compile")
    checked: subprocess.CompletedProcess[str] = sqb(project_dir, "build")

    assert moved.returncode == 0, moved.stdout + moved.stderr
    assert "import " in moved.stdout, moved.stdout
    assert fragments_present(
        files={
            **project_text(project_dir),
            CENTS_CHECK: (project_dir / CENTS_CHECK).read_text(encoding="utf-8"),
        },
        expected=test_case.expected_fragments,
    ), moved.stdout
    assert compiled.returncode == 0, compiled.stdout + compiled.stderr
    assert checked.returncode == 0, checked.stdout + checked.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        PythonImportRefusalE2ETestCase(
            description="a dynamic import naming the macro module",
            files=cents_check(
                "import importlib\n\n"
                'CENTS = importlib.import_module("models.staging._sqlbuild._macros.python.cents")',
                body='    assert CENTS.to_cents("1")\n',
            ),
            expected_location=f"{CENTS_CHECK}:6:9",
            expected_reason=(
                "imports models.staging._sqlbuild._macros.python.cents dynamically by name, which "
                "cannot be rewritten safely"
            ),
        ),
        PythonImportRefusalE2ETestCase(
            description="a star import from the macro module's package",
            files=cents_check(
                "from sql.models.staging._sqlbuild._macros.python.cents import *\n"
                "from sql.models.staging._sqlbuild._macros.python import *"
            ),
            expected_location=f"{CENTS_CHECK}:5:1",
            expected_reason=(
                "star-imports the package of models.staging._sqlbuild._macros.python.cents, which moves"
            ),
        ),
        PythonImportRefusalE2ETestCase(
            description="an import without an alias",
            files=cents_check(
                "import sql.models.staging._sqlbuild._macros.python.cents",
                body='    assert models.staging._sqlbuild._macros.python.cents.to_cents("1")\n',
            ),
            expected_location=f"{CENTS_CHECK}:4:8",
            expected_reason="imports models.staging._sqlbuild._macros.python.cents without an alias",
        ),
        PythonImportRefusalE2ETestCase(
            description="a project Python file that is not UTF-8",
            files=cents_check(
                "from sql.models.staging._sqlbuild._macros.python.cents import to_cents"
            ),
            binary_files={"rules/legacy.py": b"LABEL = '\xe9'\n"},
            expected_location="rules/legacy.py",
            expected_reason="this Python file cannot be read as UTF-8",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unrewritable_python_import_when_moving_then_move_is_refused_naming_the_line(
    tmp_path: Path, test_case: PythonImportRefusalE2ETestCase
) -> None:
    project_dir: Path = write_orders_project(tmp_path=tmp_path, files=test_case.files)
    for path, contents in test_case.binary_files.items():
        (project_dir / path).parent.mkdir(parents=True, exist_ok=True)
        _ = (project_dir / path).write_bytes(contents)
    before: dict[str, str] = project_text(project_dir)
    check_before: str = (project_dir / CENTS_CHECK).read_text(encoding="utf-8")

    moved: subprocess.CompletedProcess[str] = sqb(
        project_dir, "mv", "stg_order_cents", "sql/models/finance/"
    )

    output: str = " ".join(moved.stdout.split())
    assert moved.returncode == 1, moved.stdout + moved.stderr
    assert f"{test_case.expected_location} {test_case.expected_reason}" in output, moved.stdout
    assert "Refused; no files changed" in moved.stdout
    assert project_text(project_dir) == before
    assert (project_dir / CENTS_CHECK).read_text(encoding="utf-8") == check_before
