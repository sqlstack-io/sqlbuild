from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.clickhouse._test_types import (
    ClickHouseWaffleShopE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.clickhouse.helpers import (
    cleanup_clickhouse_schema,
    create_clickhouse_schema,
    fetch_clickhouse_rows,
    prepare_clickhouse_waffle_shop,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseWaffleShopE2ETestCase(
            description="waffle shop full build succeeds on clickhouse",
            expected_return_codes=(0, 0, 0),
            expected_summary_fragments=(
                "Completed successfully",
                "FAIL=0",
                "Completed successfully",
            ),
            expected_row_counts={
                "customer_status_snapshot": 5,
                "daily_activity_rollup": 3,
                "daily_order_partitioned": 4,
                "daily_revenue": 3,
                "dim_customers": 5,
                "fact_orders": 10,
                "hourly_activity_with_daily_context": 0,
                "hourly_order_activity": 8,
                "order_status_index": 10,
                "scenario_order_prices": 10,
                "stg_customers": 5,
                "stg_orders": 10,
                "waffle_types": 6,
            },
            replanned_model="hourly_activity_with_daily_context",
            # 6h batches from 2026-03-31T18:00 to 2026-04-04T00:00, as in the first build.
            expected_replan=("normal_incremental", 13),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_waffle_shop_when_building_testing_and_rebuilding_on_clickhouse_then_tables_match(
    tmp_path: Path,
    test_case: ClickHouseWaffleShopE2ETestCase,
    clickhouse_e2e_config: dict[str, object],
) -> None:
    config: dict[str, object] = clickhouse_e2e_config
    project_dir: Path
    schema_name: str
    project_dir, schema_name = prepare_clickhouse_waffle_shop(tmp_path=tmp_path, config=config)
    create_clickhouse_schema(config=config, schema_name=schema_name)
    try:
        first: tuple[subprocess.CompletedProcess[str], ...] = tuple(
            run_sqb(command=command, project_dir=project_dir)
            for command in (
                ("--no-color", "build", "--concurrency", "4"),
                ("--no-color", "test"),
            )
        )
        replan: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "plan", "-s", test_case.replanned_model, "--json"),
            project_dir=project_dir,
        )
        planned: dict[str, Any] = json.loads(replan.stdout)["models"][0]
        # The model is empty after the first build: neither its nullable columns nor an empty
        # non-nullable cursor may send the next build back to 1970.
        assert (planned["reason"], planned["cursor"]["planned_batch_count"]) == (
            test_case.expected_replan
        ), replan.stdout + replan.stderr
        results: tuple[subprocess.CompletedProcess[str], ...] = (
            *first,
            run_sqb(command=("--no-color", "build", "--concurrency", "4"), project_dir=project_dir),
        )

        assert tuple(result.returncode for result in results) == (
            test_case.expected_return_codes
        ), "\n".join(result.stdout + result.stderr for result in results)
        assert [
            fragment in result.stdout
            for fragment, result in zip(test_case.expected_summary_fragments, results, strict=True)
        ] == [True, True, True], "\n".join(result.stdout for result in results)
        assert {
            name: int(
                str(
                    fetch_clickhouse_rows(
                        config=config, sql=f"SELECT count() FROM {schema_name}.{name}"
                    )[0][0]
                )
            )
            for name in test_case.expected_row_counts
        } == (test_case.expected_row_counts)
    finally:
        cleanup_clickhouse_schema(config=config, schema_name=schema_name)
