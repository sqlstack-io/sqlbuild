"""E2E coverage for runtime-owned managed-source cursor watermarks."""

from __future__ import annotations

import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    LoaderWatermarkBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        LoaderWatermarkBuildE2ETestCase(
            description="managed loader watermark is read after same-build source load",
            initial_maximum=2,
            changed_maximum=3,
            expected_initial_rows=((1, 100), (2, 200)),
            expected_changed_rows=((1, 100), (2, 200), (3, 300)),
            expected_plan_bounds=(
                "bounds  computed at run time, once source raw_orders is up to date"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_managed_loader_watermark_when_reloaded_then_incremental_consumes_new_maximum(
    test_case: LoaderWatermarkBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="loader_watermark_build",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                extra_paths = ["maximum.txt"]
                name = "loader_watermark_build"
                adapter = "duckdb"

                [connection]
                database = "warehouse.duckdb"

                [defaults]
                materialized = "table"
                """
            ).strip()
            + "\n",
            "python/nodes/loaders/raw/loader.py": (
                "from pathlib import Path\n"
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='replace')\n"
                "def raw_orders(ctx):\n"
                "    '''Test loader raw_orders.'''\n    maximum = int(Path(__file__).parents[4].joinpath('maximum.txt').read_text())\n"
                "    return [{'id': value, 'amount': value * 100} "
                "for value in range(1, maximum + 1)]\n"
            ),
            "sql/sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    managed: true\n"
                "    columns:\n"
                "      - name: id\n"
                "        type: INTEGER\n"
                "      - name: amount\n"
                "        type: INTEGER\n"
            ),
            "sql/models/raw_orders_incremental.sql": dedent(
                """
                MODEL (description "Test model raw_orders_incremental.",
                  materialized incremental,
                  incremental_strategy delete_insert,
                  cursor id,
                  cursor_type integer,
                  cursor_inputs (
                    raw_orders id,
                  ),
                );

                SELECT id, amount
                FROM __source("raw_orders")
                """
            ).strip()
            + "\n",
        },
    )
    warehouse_path: Path = project_dir / "warehouse.duckdb"
    maximum_path: Path = project_dir / "maximum.txt"
    maximum_path.write_text(str(test_case.initial_maximum), encoding="utf-8")

    initial_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--select", "+raw_orders_incremental"),
        project_dir=project_dir,
    )
    assert initial_result.returncode == 0, initial_result.stdout + initial_result.stderr
    assert query_duckdb(
        db_path=warehouse_path,
        sql="SELECT id, amount FROM main.raw_orders_incremental ORDER BY id",
    ) == list(test_case.expected_initial_rows)

    plan_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan", "--select", "+raw_orders_incremental"),
        project_dir=project_dir,
    )
    assert plan_result.returncode == 0, plan_result.stdout + plan_result.stderr
    assert test_case.expected_plan_bounds in plan_result.stdout, plan_result.stdout

    maximum_path.write_text(str(test_case.changed_maximum), encoding="utf-8")
    changed_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--reload", "--select", "+raw_orders_incremental"),
        project_dir=project_dir,
    )
    assert changed_result.returncode == 0, changed_result.stdout + changed_result.stderr
    assert query_duckdb(
        db_path=warehouse_path,
        sql="SELECT id, amount FROM main.raw_orders_incremental ORDER BY id",
    ) == list(test_case.expected_changed_rows)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
