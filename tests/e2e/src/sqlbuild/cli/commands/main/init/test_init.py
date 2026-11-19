"""E2E tests for sqb init."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.init._test_types import (
    InitE2ETestCase,
    InitPythonRootE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        InitE2ETestCase(
            description="init scaffolds typed hook and audit directories",
            expected_exit_code=0,
            expected_paths=(
                "sqlbuild_project.toml",
                "python",
                "python/.gitkeep",
                "hooks/sql",
                "hooks/sql/.gitkeep",
                "hooks/python",
                "hooks/python/.gitkeep",
                "audits/generic",
                "audits/generic/.gitkeep",
                "audits/singular",
                "audits/singular/.gitkeep",
            ),
            expected_output_fragments=(
                "SQLBuild project created",
                "Add Python loaders, tasks, assets, checks, and factories to python/",
                "Add SQL hooks to hooks/sql/, Python hooks to hooks/python/",
                "reusable audits to audits/generic/",
                "standalone audits to audits/singular/",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_empty_project_directory_when_running_init_then_typed_resource_directories_are_scaffolded(
    test_case: InitE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = tmp_path / "init_hooks_project"
    project_dir.mkdir()

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "init"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    for fragment in test_case.expected_output_fragments:
        assert fragment in result.stdout
    for expected_path in test_case.expected_paths:
        assert (project_dir / expected_path).exists()


@pytest.mark.parametrize(
    "test_case",
    [
        InitPythonRootE2ETestCase(
            description="initialized project builds Python nodes and helpers under python",
            project_files={
                "models/marts/fact_orders.sql": (
                    "MODEL (description 'Test model fact_orders.', materialized table);\n\nSELECT 1 AS order_id\n"
                ),
                "python/_helpers.py": "def order_count(rows):\n    return len(rows)\n",
                "python/tasks/orders.py": (
                    "from python._helpers import order_count\n"
                    "from sqlbuild.assets import asset\n\n\n"
                    "@asset\n"
                    "def count_orders(ctx):\n"
                    "    '''Test task count_orders.'''\n    return ctx.result(payload={'order_count': order_count([1, 2])})\n"
                ),
                "python/assets/orders_export.py": (
                    "from python.tasks.orders import count_orders\n"
                    "from sqlbuild.assets import asset\n"
                    "from sqlbuild.refs import model\n\n\n"
                    "@asset(depends_on=(model('fact_orders'), count_orders))\n"
                    "def orders_export(ctx):\n"
                    "    '''Test asset orders_export.'''\n    return ctx.result(payload={'ready': True})\n"
                ),
            },
            unexpected_paths=("tasks", "assets", "checks", "loaders", "factories", "libs"),
            expected_build_fragments=(
                "fact_orders",
                "count_orders",
                "orders_export",
                "\u2713 Completed successfully",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_initialized_project_with_python_root_nodes_when_building_then_nodes_run(
    test_case: InitPythonRootE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = tmp_path / "init_python_root_project"
    project_dir.mkdir()
    init_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "init"),
        project_dir=project_dir,
    )
    assert init_result.returncode == 0, init_result.stdout + init_result.stderr
    relative_path: str
    contents: str
    for relative_path, contents in test_case.project_files.items():
        file_path: Path = project_dir / relative_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(contents, encoding="utf-8")

    compile_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"),
        project_dir=project_dir,
    )
    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
    )

    assert compile_result.returncode == 0, compile_result.stdout + compile_result.stderr
    build_output: str = build_result.stdout + build_result.stderr
    assert build_result.returncode == 0, build_output
    for fragment in test_case.expected_build_fragments:
        assert fragment in build_output
    for unexpected_path in test_case.unexpected_paths:
        assert not (project_dir / unexpected_path).exists()
