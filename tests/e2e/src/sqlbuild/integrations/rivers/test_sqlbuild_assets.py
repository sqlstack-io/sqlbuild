"""E2E tests for executing SQLBuild through Rivers assets."""

from __future__ import annotations

import os
import runpy
import subprocess
from collections.abc import Iterator
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import PlaygroundCommandRequest, load_sqlbuild_dag
from sqlbuild._native import run_playground_command as run_playground
from sqlbuild.integrations.rivers import SqlBuildProject, sqlbuild_assets
from sqlbuild.integrations.rivers._helpers.assets import build_asset_defs
from sqlbuild.integrations.rivers.classes.sqlbuild_rivers_translator import (
    SqlBuildRiversTranslator,
)
from sqlbuild.integrations.rivers.exceptions import RiversDagInputError
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    REPO_ROOT,
    prepare_waffle_shop,
    table_exists,
)
from tests.e2e.src.sqlbuild.integrations.dagster.helpers import (
    prepare_python_nodes_integration_project,
)
from tests.e2e.src.sqlbuild.integrations.rivers._test_types import (
    RiversPlaygroundE2ETestCase,
    RiversPythonNodesArtifactE2ETestCase,
    RiversSqlBuildE2ETestCase,
)

rs: Any = pytest.importorskip("rivers")


@pytest.mark.parametrize(
    "test_case",
    [
        RiversPythonNodesArtifactE2ETestCase(
            description="rivers consumes real Python-node dag artifact",
            expected_asset_names=frozenset(
                {
                    "main__orders",
                    "asset__prepare_orders",
                    "warehouse_export",
                    "asset__orders_export",
                }
            ),
            expected_task_deps=("main__orders",),
            expected_asset_deps=("asset__prepare_orders", "warehouse_export"),
            expected_task_group="python",
            expected_asset_group="exports",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_nodes_project_when_loading_rivers_assets_then_maps_real_artifact(
    test_case: RiversPythonNodesArtifactE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_python_nodes_integration_project(tmp_path)
    sqb_executable: Path = REPO_ROOT / ".venv" / "bin" / "sqb"
    sqlbuild_project: SqlBuildProject = SqlBuildProject(
        project_dir=project_dir,
        sqb_command=(str(sqb_executable),),
    )
    sqlbuild_project.prepare()

    @sqlbuild_assets(project=sqlbuild_project)
    def sqlbuild_python_nodes(context: Any) -> Iterator[Any]:
        del context
        return
        yield

    output_defs: tuple[Any, ...] = build_asset_defs(
        dag=load_sqlbuild_dag(sqlbuild_project.dag_path, error_class=RiversDagInputError),
        translator=SqlBuildRiversTranslator(),
    )
    repo: Any = rs.CodeRepository(assets=[sqlbuild_python_nodes])
    output_defs_by_name: dict[str, tuple[Any, ...]] = {
        asset.name: (asset,) for asset in output_defs
    }
    assert len(output_defs_by_name) == len(output_defs)
    task_def: Any = next(iter(output_defs_by_name.get("asset__prepare_orders", ())))
    asset_def: Any = next(iter(output_defs_by_name.get("asset__orders_export", ())))

    assert sqlbuild_project.dag_path.exists()
    assert test_case.expected_asset_names <= set(repo.assets)
    assert tuple(dep.name for dep in task_def.deps) == test_case.expected_task_deps
    assert tuple(dep.name for dep in asset_def.deps) == test_case.expected_asset_deps
    assert task_def.group == test_case.expected_task_group
    assert asset_def.group == test_case.expected_asset_group


@pytest.mark.parametrize(
    "test_case",
    [
        RiversSqlBuildE2ETestCase(
            description="rivers loads generated dag artifact and executes sqlbuild build",
            expected_success=True,
            expected_asset_names=frozenset(
                {"raw_customers", "raw_orders", "main__waffle_types", "main__fact_orders"}
            ),
            expected_table_names=("raw_customers", "raw_orders", "fact_orders", "dim_customers"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_waffle_shop_when_executing_sqlbuild_assets_then_rivers_run_succeeds(
    tmp_path: Path,
    test_case: RiversSqlBuildE2ETestCase,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)
    sqb_executable: Path = REPO_ROOT / ".venv" / "bin" / "sqb"
    sqlbuild_project: SqlBuildProject = SqlBuildProject(
        project_dir=project_dir,
        sqb_command=(str(sqb_executable),),
    )
    sqlbuild_project.prepare()

    @sqlbuild_assets(project=sqlbuild_project)
    def sqlbuild_waffle_shop(context: Any) -> Iterator[Any]:
        completed: subprocess.CompletedProcess[str] = subprocess.run(
            [str(sqb_executable), "build"],
            cwd=project_dir,
            capture_output=True,
            check=False,
            text=True,
        )
        assert completed.returncode == 0, completed.stderr
        for output_name in context.output_selection:
            yield rs.Materialization(output_name=output_name)

    repo: Any = rs.CodeRepository(
        assets=[sqlbuild_waffle_shop],
        jobs=[
            rs.Job(
                name="waffle_shop",
                assets=[sqlbuild_waffle_shop],
                executor=rs.Executor.in_process(),
            )
        ],
        default_executor=rs.Executor.in_process(),
    )
    result: Any = repo.get_job("waffle_shop").execute()

    assert result.success is test_case.expected_success
    assert sqlbuild_project.dag_path.exists()
    assert test_case.expected_asset_names <= set(repo.assets)
    for table_name in test_case.expected_table_names:
        assert table_exists(db_path=project_dir / "waffle_shop.duckdb", table_name=table_name)


@pytest.mark.parametrize(
    "test_case",
    [
        RiversPlaygroundE2ETestCase(
            description="generated rivers playground materializes loader-backed waffle shop",
            expected_success=True,
            expected_table_names=("raw__customers", "raw__orders", "fact_orders"),
            expected_schema="dev",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_generated_rivers_playground_when_materializing_assets_then_build_succeeds(
    test_case: RiversPlaygroundE2ETestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    playground_name: str = "rivers_waffle_shop"
    assert (
        run_playground(
            PlaygroundCommandRequest(
                project_dir=tmp_path, target_path=playground_name, template="rivers"
            )
        )
        == 0
    )
    orchestrator_dir: Path = tmp_path / playground_name
    project_dir: Path = orchestrator_dir / "waffle_shop"
    sqb_bin_dir: Path = REPO_ROOT / ".venv" / "bin"
    monkeypatch.setenv("RIVERS_DEPLOYMENT", "dev")
    monkeypatch.setenv("PATH", f"{sqb_bin_dir}{os.pathsep}{os.environ.get('PATH', '')}")

    generated_defs: dict[str, object] = runpy.run_path(str(orchestrator_dir / "definitions.py"))
    repo: Any = generated_defs["repo"]
    sqlbuild_project: SqlBuildProject = generated_defs["SQLBUILD_PROJECT"]  # type: ignore[assignment]

    result: Any = repo.materialize()

    assert result.success is test_case.expected_success
    assert sqlbuild_project.dag_path.exists()
    for table_name in test_case.expected_table_names:
        assert table_exists(
            db_path=project_dir / "waffle_shop_control.duckdb",
            table_name=table_name,
            schema=test_case.expected_schema,
        )
