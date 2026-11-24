"""E2E coverage for runtime-owned seed cursor watermarks."""

from __future__ import annotations

import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    SeedWatermarkBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        SeedWatermarkBuildE2ETestCase(
            description="selected seed watermark is read after same-build seed materialization",
            initial_seed="id,amount\n1,100\n2,200\n",
            changed_seed="id,amount\n1,100\n2,200\n3,300\n",
            expected_initial_rows=((1, 100), (2, 200)),
            expected_changed_rows=((1, 100), (2, 200), (3, 300)),
            expected_plan_bounds=(
                "bounds  computed at run time, once seed order_events is up to date"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_seed_watermark_when_seed_changes_then_incremental_consumes_new_high_watermark(
    test_case: SeedWatermarkBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="seed_watermark_build",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "seed_watermark_build"
                adapter = "duckdb"

                [connection]
                database = "warehouse.duckdb"

                [defaults]
                materialized = "table"
                """
            ).strip()
            + "\n",
            "sql/seeds/schema.yml": (
                "seeds:\n"
                "  - name: order_events\n    description: Test seed order_events.\n"
                "    columns:\n"
                "      - name: id\n"
                "        type: INTEGER\n"
                "      - name: amount\n"
                "        type: INTEGER\n"
            ),
            "sql/seeds/order_events.csv": test_case.initial_seed,
            "sql/models/order_events_incremental.sql": dedent(
                """
                MODEL (description "Test model order_events_incremental.",
                  materialized incremental,
                  incremental_strategy delete_insert,
                  cursor id,
                  cursor_type integer,
                  cursor_inputs (
                    order_events id,
                  ),
                );

                SELECT id, amount
                FROM __seed("order_events")
                """
            ).strip()
            + "\n",
        },
    )
    warehouse_path: Path = project_dir / "warehouse.duckdb"

    initial_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert initial_result.returncode == 0, initial_result.stdout + initial_result.stderr
    assert query_duckdb(
        db_path=warehouse_path,
        sql="SELECT id, amount FROM main.order_events_incremental ORDER BY id",
    ) == list(test_case.expected_initial_rows)

    plan_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan"), project_dir=project_dir
    )
    assert plan_result.returncode == 0, plan_result.stdout + plan_result.stderr
    assert test_case.expected_plan_bounds in plan_result.stdout, plan_result.stdout

    (project_dir / "sql" / "seeds" / "order_events.csv").write_text(
        test_case.changed_seed, encoding="utf-8"
    )
    changed_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert changed_result.returncode == 0, changed_result.stdout + changed_result.stderr
    assert query_duckdb(
        db_path=warehouse_path,
        sql="SELECT id, amount FROM main.order_events_incremental ORDER BY id",
    ) == list(test_case.expected_changed_rows)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
