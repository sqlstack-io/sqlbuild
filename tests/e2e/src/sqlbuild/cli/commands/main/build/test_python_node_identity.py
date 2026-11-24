"""E2E tests for Python node version identity across decorator-input edits."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    PythonHelperPackageIdentityE2ETestCase,
    PythonNodeIdentityBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)
from tests.unit.src.sqlbuild.python_nodes.helpers import node_module_files

_LOADERS_MODULE: str = "python/nodes/loaders/orders.py"
_ASSETS_MODULE: str = "python/nodes/assets/exports.py"
_LOADERS_PATH: str = "python/nodes/loaders/orders/staged_orders/loader.py"
_ASSETS_PATH: str = "python/nodes/assets/exports/export_orders/asset.py"
_ORIGINAL_COLUMNS: str = 'LoaderColumnSpec(name="order_id", type="INTEGER")'
_ORIGINAL_RETRY: str = "RetryPolicy(max_attempts=2, retry_on=RuntimeError)"


@pytest.mark.parametrize(
    "test_case",
    [
        PythonNodeIdentityBuildE2ETestCase(
            description="loader declared column edit records a new loader version",
            edited_path=_LOADERS_PATH,
            original_text=_ORIGINAL_COLUMNS,
            edited_text=(
                'LoaderColumnSpec(name="order_id", type="INTEGER", '
                'description="Staged order identifier")'
            ),
            expected_asset_identity_status="unchanged",
            expected_loader_version_count=2,
        ),
        PythonNodeIdentityBuildE2ETestCase(
            description="asset retry edit keeps every version unchanged",
            edited_path=_ASSETS_PATH,
            original_text=_ORIGINAL_RETRY,
            edited_text="RetryPolicy(max_attempts=5, retry_on=RuntimeError)",
            expected_asset_identity_status="unchanged",
            expected_loader_version_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_built_python_nodes_when_editing_decorator_inputs_then_identity_tracks_columns(
    test_case: PythonNodeIdentityBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_node_identity_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "python_node_identity_project"
                adapter = "duckdb"

                [connection]
                database = "warehouse.duckdb"
                """
            ).strip()
            + "\n",
            **node_module_files(
                _LOADERS_MODULE,
                dedent(
                    f"""
                from sqlbuild.loaders import LoaderColumnSpec, loader

                STAGED_ORDER_COLUMNS = [{_ORIGINAL_COLUMNS}]


                @loader(write_strategy="table", columns=STAGED_ORDER_COLUMNS)
                def staged_orders(ctx):
                    '''Test loader staged_orders.'''
                    return [{{"order_id": 1}}]


                @loader(depends_on=[staged_orders])
                def raw_orders(ctx):
                    '''Test loader raw_orders.'''
                    staged = ctx.loader(staged_orders).destination
                    rows = ctx.query(f"SELECT order_id FROM {{staged}}")
                    return [{{"order_id": row[0]}} for row in rows]
                """
                ).lstrip(),
            ),
            "sql/sources/raw.yml": dedent(
                """
                sources:
                  - name: raw_orders
                    description: Test source raw_orders.
                    managed: true
                    write_strategy: table
                    columns:
                      - name: order_id
                        type: INTEGER
                """
            ).lstrip(),
            **node_module_files(
                _ASSETS_MODULE,
                dedent(
                    f"""
                from sqlbuild.assets import asset
                from sqlbuild.retries import RetryPolicy

                EXPORT_RETRY = {_ORIGINAL_RETRY}


                @asset(retry=EXPORT_RETRY)
                def export_orders(ctx):
                    '''Test asset export_orders.'''
                    return None
                """
                ).lstrip(),
            ),
        },
    )
    first_build: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert first_build.returncode == 0, first_build.stdout + first_build.stderr

    edited: Path = project_dir / test_case.edited_path
    edited.write_text(
        edited.read_text(encoding="utf-8").replace(test_case.original_text, test_case.edited_text),
        encoding="utf-8",
    )
    plan: subprocess.CompletedProcess[str] = run_sqb(
        command=("plan", "--json"), project_dir=project_dir
    )
    second_build: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    assert plan.returncode == 0, plan.stdout + plan.stderr
    assert second_build.returncode == 0, second_build.stdout + second_build.stderr
    payload: dict[str, object] = json.loads(plan.stdout)
    statuses_by_name: dict[str, str] = {
        str(entry["name"]): str(entry["identity_status"]) for entry in payload["python_nodes"]
    }
    assert statuses_by_name["export_orders"] == test_case.expected_asset_identity_status
    assert query_duckdb(
        db_path=project_dir / "warehouse.duckdb",
        sql=(
            "SELECT COUNT(DISTINCT version_hash) FROM main._sqlbuild_fingerprints "
            "WHERE node_type = 'loader' AND node_name = 'staged_orders'"
        ),
    ) == [(test_case.expected_loader_version_count,)]


@pytest.mark.parametrize(
    "test_case",
    [
        PythonHelperPackageIdentityE2ETestCase(
            description="relative-import helper edit changes the dependent task identity",
            edited_path="python/nodes/helpers/clean.py",
            original_text="value.strip().lower()",
            edited_text="value.strip().upper()",
            expected_identity_status="changed",
            expected_version_count=2,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_helper_package_with_relative_imports_when_editing_helper_then_task_identity_changes(
    test_case: PythonHelperPackageIdentityE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_helper_package_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "python_helper_package_project"
                adapter = "duckdb"

                [connection]
                database = "warehouse.duckdb"
                """
            ).strip()
            + "\n",
            "python/nodes/helpers/__init__.py": "",
            "python/nodes/helpers/values.py": 'STATUS = "shipped"\n',
            **node_module_files(
                "python/nodes/helpers/clean.py",
                dedent(
                    """
                from .values import STATUS


                def normalize_status(value):
                    return f"{value.strip().lower()}:{STATUS}"
                """
                ).lstrip(),
            ),
            **node_module_files(
                "python/nodes/orders.py",
                dedent(
                    """
                import python.nodes.helpers.clean
                from sqlbuild.assets import asset


                @asset
                def orders(ctx):
                    '''Test task orders.'''
                    return ctx.result(
                        payload={"status": python.nodes.helpers.clean.normalize_status(" Shipped ")}
                    )
                """
                ).lstrip(),
            ),
        },
    )
    first_build: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert first_build.returncode == 0, first_build.stdout + first_build.stderr

    edited: Path = project_dir / test_case.edited_path
    edited.write_text(
        edited.read_text(encoding="utf-8").replace(test_case.original_text, test_case.edited_text),
        encoding="utf-8",
    )
    plan: subprocess.CompletedProcess[str] = run_sqb(
        command=("plan", "--json"), project_dir=project_dir
    )
    second_build: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    assert plan.returncode == 0, plan.stdout + plan.stderr
    assert second_build.returncode == 0, second_build.stdout + second_build.stderr
    payload: dict[str, object] = json.loads(plan.stdout)
    statuses_by_name: dict[str, str] = {
        str(entry["name"]): str(entry["identity_status"]) for entry in payload["python_nodes"]
    }
    assert statuses_by_name["orders"] == test_case.expected_identity_status
    assert query_duckdb(
        db_path=project_dir / "warehouse.duckdb",
        sql=(
            "SELECT COUNT(DISTINCT version_hash) FROM main._sqlbuild_fingerprints "
            "WHERE node_type = 'asset' AND node_name = 'orders'"
        ),
    ) == [(test_case.expected_version_count,)]


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
