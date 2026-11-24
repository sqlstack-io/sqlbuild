"""CLI e2e coverage for `sqb mv` on a model that Python code names."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.refactor._test_types import (
    PythonNamedMoveE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_CHECK: str = (
    "from sqlbuild.checks import check\nfrom sqlbuild.refs import model\n\n\n"
    '@check(depends_on=model("orders"))\n'
    "def orders_present(ctx):\n"
    "    '''Test check orders_present.'''\n"
    "    return ctx.pass_()\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonNamedMoveE2ETestCase(
            description="a move to a folder keeps the name and moves",
            destination="sql/models/marts/",
            expected_exit_code=0,
            expected_model_path="sql/models/marts/orders.sql",
        ),
        PythonNamedMoveE2ETestCase(
            description="a move to a file of the same name moves",
            destination="sql/models/marts/orders.sql",
            expected_exit_code=0,
            expected_model_path="sql/models/marts/orders.sql",
        ),
        PythonNamedMoveE2ETestCase(
            description="a move that renames the model is refused",
            destination="sql/models/marts/fct_orders.sql",
            expected_exit_code=1,
            expected_model_path="sql/models/orders.sql",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_model_named_by_python_when_moving_then_only_renames_are_refused(
    test_case: PythonNamedMoveE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "shop"\nadapter = "duckdb"\n\n[connection]\ndatabase = "shop.duckdb"\n\n'
                "[scopes]\nenforce_placement = false\n"
            ),
            "sql/models/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1 AS order_id\n",
            "python/nodes/checks/orders/checks.py": _CHECK,
        },
    )

    moved: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "mv", "orders", test_case.destination), project_dir=project_dir
    )

    assert moved.returncode == test_case.expected_exit_code, moved.stdout + moved.stderr
    assert (project_dir / test_case.expected_model_path).is_file()
    assert (project_dir / "python/nodes/checks/orders/checks.py").read_text(
        encoding="utf-8"
    ) == _CHECK
