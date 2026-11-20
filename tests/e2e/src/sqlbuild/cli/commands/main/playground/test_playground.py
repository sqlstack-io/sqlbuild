from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from sqlbuild._native import PlaygroundCommandRequest
from sqlbuild._native import run_playground_command as run_playground
from tests.e2e.src.sqlbuild.cli.commands.main.playground._test_types import (
    PlaygroundCompileBuildTestCase,
    PythonNodesPlaygroundLifecycleTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        PythonNodesPlaygroundLifecycleTestCase(
            description="python nodes playground runs plan build and check lifecycle",
            project_name="python_nodes_demo",
            expected_plan_fragments=(
                "raw_orders",
                "fact_orders",
                "Python read-side",
            ),
            expected_build_fragments=(
                "prepare_raw_orders",
                "raw_orders",
                "fact_orders",
                "orders_export",
                "\u2713 Completed successfully",
            ),
            expected_check_fragments=(
                "check_orders_export",
                "PASS",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_nodes_playground_when_running_lifecycle_then_it_succeeds(
    test_case: PythonNodesPlaygroundLifecycleTestCase,
    tmp_path: Path,
) -> None:
    assert (
        run_playground(
            PlaygroundCommandRequest(
                project_dir=tmp_path, target_path=test_case.project_name, template="python_nodes"
            )
        )
        == 0
    )
    project_dir: Path = tmp_path / test_case.project_name

    plan_result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "plan",
            "--select",
            "+fact_orders",
            "--select",
            "+orders_export",
        ),
        project_dir=project_dir,
    )
    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--select",
            "+fact_orders",
            "--select",
            "+orders_export",
        ),
        project_dir=project_dir,
    )
    check_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "check", "--select", "check_orders_export"),
        project_dir=project_dir,
    )

    plan_output: str = plan_result.stdout + plan_result.stderr
    assert plan_result.returncode == 0
    for expected_fragment in test_case.expected_plan_fragments:
        assert expected_fragment in plan_output

    build_output: str = build_result.stdout + build_result.stderr
    assert build_result.returncode == 0
    for expected_fragment in test_case.expected_build_fragments:
        assert expected_fragment in build_output

    check_output: str = check_result.stdout + check_result.stderr
    assert check_result.returncode == 0
    for expected_fragment in test_case.expected_check_fragments:
        assert expected_fragment in check_output


@pytest.mark.parametrize(
    "test_case",
    [
        PlaygroundCompileBuildTestCase(
            description="waffle shop playground loaders under python compile and build",
            template="waffle_shop",
            project_subdir="",
            expected_build_fragments=("raw__customers", "raw__orders", "fact_orders"),
        ),
        PlaygroundCompileBuildTestCase(
            description="loader waffle shop playground loaders under python compile and build",
            template="loader_waffle_shop",
            project_subdir="",
            expected_build_fragments=("raw_orders", "raw_customers", "fact_waffle_orders"),
        ),
        PlaygroundCompileBuildTestCase(
            description="dagster playground nested SQLBuild project compiles and builds",
            template="dagster",
            project_subdir="waffle_shop",
            expected_build_fragments=("raw__customers", "raw__orders", "fact_orders"),
        ),
        PlaygroundCompileBuildTestCase(
            description="rivers playground nested SQLBuild project compiles and builds",
            template="rivers",
            project_subdir="waffle_shop",
            expected_build_fragments=("raw__customers", "raw__orders", "fact_orders"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_playground_template_when_compiling_and_building_then_it_succeeds(
    test_case: PlaygroundCompileBuildTestCase,
    tmp_path: Path,
) -> None:
    assert (
        run_playground(
            PlaygroundCommandRequest(
                project_dir=tmp_path, target_path="playground", template=test_case.template
            )
        )
        == 0
    )
    project_dir: Path = tmp_path / "playground" / test_case.project_subdir
    ignored: list[str] = (
        (tmp_path / "playground" / ".gitignore").read_text(encoding="utf-8").splitlines()
    )
    assert ".env" in ignored

    compile_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )
    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    assert compile_result.returncode == 0, compile_result.stdout + compile_result.stderr
    build_output: str = build_result.stdout + build_result.stderr
    assert build_result.returncode == 0, build_output
    expected_fragment: str
    for expected_fragment in (*test_case.expected_build_fragments, "\u2713 Completed successfully"):
        assert expected_fragment in build_output
