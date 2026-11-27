from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.sqlite._test_types import (
    SqliteLoaderE2ETestCase,
    SqliteWaffleShopE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.sqlite.helpers import (
    LOADER_PROJECT_FILES,
    prepare_sqlite_waffle_shop,
    run_sqb_commands,
    waffle_row_counts,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    prepare_waffle_shop,
)


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteWaffleShopE2ETestCase(
            description="waffle shop full build succeeds on sqlite",
            commands=(("build",), ("test",), ("build",)),
            expected_fragments=("Completed successfully", "FAIL=0", "Completed successfully"),
            expected_row_counts={
                "raw_customers": 5,
                "raw_orders": 10,
                "waffle_types": 6,
                "stg_customers": 5,
                "stg_orders": 10,
                "stg_payments": 8,
                "fact_orders": 10,
                "dim_customers": 5,
                "daily_revenue": 3,
                "scenario_order_prices": 10,
                "customer_status_snapshot": 5,
                "order_status_index": 10,
                "hourly_order_activity": 8,
                "daily_activity_rollup": 3,
                "hourly_activity_with_daily_context": 0,
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_waffle_shop_when_building_testing_and_rebuilding_on_sqlite_then_rows_match_duckdb(
    tmp_path: Path, test_case: SqliteWaffleShopE2ETestCase
) -> None:
    project_dirs: dict[str, Path] = {
        "sqlite": prepare_sqlite_waffle_shop(tmp_path=tmp_path / "sqlite"),
        "duckdb": prepare_waffle_shop(tmp_path / "duckdb"),
    }
    row_counts: dict[str, dict[str, int]] = {}
    adapter: str
    project_dir: Path
    for adapter, project_dir in project_dirs.items():
        results: tuple[subprocess.CompletedProcess[str], ...] = run_sqb_commands(
            project_dir=project_dir, commands=test_case.commands
        )
        result: subprocess.CompletedProcess[str]
        fragment: str
        for result, fragment in zip(results, test_case.expected_fragments, strict=True):
            output: str = result.stdout + result.stderr
            assert result.returncode == test_case.expected_return_code, (adapter, output)
            assert fragment in result.stdout, (adapter, output)
        row_counts[adapter] = waffle_row_counts(adapter=adapter, project_dir=project_dir)

    assert row_counts["sqlite"] == row_counts["duckdb"]
    assert row_counts["sqlite"] == test_case.expected_row_counts


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteLoaderE2ETestCase(
            description="a loader is refused at compile, since SQLLoad can't write to SQLite",
            expected_fragment=(
                "loads and loaders can't write to sqlite yet; SQLLoad loads into duckdb, "
                "motherduck, snowflake, databricks, bigquery, postgres and clickhouse"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_loader_when_compiling_on_sqlite_then_it_is_refused(
    tmp_path: Path, test_case: SqliteLoaderE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="events_shop", repo_files=LOADER_PROJECT_FILES
    )

    results: tuple[subprocess.CompletedProcess[str], ...] = run_sqb_commands(
        project_dir=project_dir, commands=(("compile",),)
    )

    output: str = results[0].stdout + results[0].stderr
    assert results[0].returncode != 0, output
    assert test_case.expected_fragment in output
