"""E2E tests for mixed timestamp grain replay behavior."""

from __future__ import annotations

import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    MixedTimestampGrainBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        MixedTimestampGrainBuildE2ETestCase(
            description="known current upstream reruns at consumer grain",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """

                name = "mixed_grain_project"

                adapter = "duckdb"



                [connection]

                database = "mixed_grain.duckdb"



                [defaults]

                materialized = "table"

                    """
                ).strip()
                + "\n",
                "seed_raw_data.sql": dedent(
                    """
                CREATE TABLE IF NOT EXISTS raw_orders (
                  id INTEGER,
                  quantity INTEGER,
                  ordered_at TIMESTAMP,
                  amount_cents INTEGER
                );

                INSERT INTO raw_orders VALUES
                  (1, 1, '2026-04-01 09:00:00', 850),
                  (2, 1, '2026-04-01 10:00:00', 850),
                  (3, 2, '2026-04-02 08:00:00', 2100),
                  (4, 1, '2026-04-02 12:00:00', 950),
                  (5, 1, '2026-04-03 09:00:00', 850),
                  (6, 2, '2026-04-03 11:00:00', 1500),
                  (7, 1, '2026-04-04 13:00:00', 850),
                  (8, 2, '2026-04-04 14:00:00', 1700);
                """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                sources:
                  - name: raw_orders
                    description: Test source raw_orders.
                    schema: main
                    table: raw_orders
                """
                ).strip()
                + "\n",
                "sql/models/staging/stg_orders.sql": dedent(
                    """
                MODEL (description "Test model stg_orders.", materialized view);

                SELECT
                  id AS order_id,
                  quantity,
                  ordered_at,
                  amount_cents AS line_total_cents
                FROM __source("raw_orders")
                """
                ).strip()
                + "\n",
                "sql/models/marts/fact_orders.sql": dedent(
                    """
                MODEL (description "Test model fact_orders.", materialized table);

                SELECT order_id, quantity, ordered_at, line_total_cents
                FROM __ref("stg_orders")
                """
                ).strip()
                + "\n",
                "sql/models/marts/hourly_order_activity.sql": dedent(
                    """
                MODEL (description "Test model hourly_order_activity.",
                  materialized incremental,
                  incremental_strategy delete_insert,
                  cursor activity_hour,
                  cursor_type timestamp,
                  cursor_grain hour,
                  cursor_inputs (
            fact_orders (column ordered_at, roles [filter, watermark]),
          ),
                  incremental_mode microbatch,
          microbatch_strategy watermark,
          cursor_watermark_mode all,
                  batch_size 1d,
                );

                SELECT
                  DATE_TRUNC('hour', ordered_at) AS activity_hour,
                  COUNT(*) AS orders_placed,
                  SUM(quantity) AS waffles_ordered,
                  SUM(line_total_cents) AS revenue_cents
                FROM __ref("fact_orders")
                GROUP BY DATE_TRUNC('hour', ordered_at)
                """
                ).strip()
                + "\n",
                "sql/models/marts/daily_activity_rollup.sql": dedent(
                    """
                MODEL (description "Test model daily_activity_rollup.",
                  materialized incremental,
                  incremental_strategy delete_insert,
                  cursor activity_day,
                  cursor_type timestamp,
                  cursor_grain day,
                  cursor_inputs (
            hourly_order_activity (column activity_hour, roles [filter, watermark]),
          ),
                  incremental_mode microbatch,
          microbatch_strategy watermark,
          cursor_watermark_mode all,
                  batch_size 2d,
                );

                SELECT
                  DATE_TRUNC('day', activity_hour) AS activity_day,
                  SUM(orders_placed) AS orders_placed,
                  SUM(waffles_ordered) AS waffles_ordered,
                  SUM(revenue_cents) AS revenue_cents
                FROM __ref("hourly_order_activity")
                GROUP BY DATE_TRUNC('day', activity_hour)
                """
                ).strip()
                + "\n",
                "sql/models/marts/hourly_activity_with_daily_context.sql": dedent(
                    """
                MODEL (description "Test model hourly_activity_with_daily_context.",
                  materialized incremental,
                  incremental_strategy delete_insert,
                  cursor activity_hour,
                  cursor_type timestamp,
                  cursor_grain hour,
                  cursor_inputs (
                    hourly_order_activity (column activity_hour, roles [filter]),
                    daily_activity_rollup (column activity_day, roles [filter, watermark]),
                  ),
                  incremental_mode microbatch,
          microbatch_strategy watermark,
          cursor_watermark_mode all,
                  batch_size 1d,
                );

                SELECT
                  h.activity_hour,
                  h.orders_placed,
                  d.orders_placed AS day_orders_placed,
                  h.waffles_ordered,
                  h.revenue_cents
                FROM __ref("hourly_order_activity") h
                INNER JOIN __ref("daily_activity_rollup") d
                  ON DATE_TRUNC('day', h.activity_hour) = d.activity_day
                """
                ).strip()
                + "\n",
            },
            initial_command=("--no-color", "build"),
            rerun_command=("--debug", "build", "--select", "hourly_activity_with_daily_context"),
            expected_exit_code=0,
            expected_window_fragment="window=2026-04-03T11:00:00..2026-04-04T00:00:00",
            expected_row_count=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_known_current_upstream_when_rerunning_then_cli_uses_consumer_grain(
    test_case: MixedTimestampGrainBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="mixed_grain_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "mixed_grain.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute((project_dir / "seed_raw_data.sql").read_text(encoding="utf-8"))
    connection.close()

    initial_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.initial_command,
        project_dir=project_dir,
    )
    assert initial_result.returncode == test_case.expected_exit_code, (
        initial_result.stdout + initial_result.stderr
    )

    rerun_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.rerun_command,
        project_dir=project_dir,
    )
    assert rerun_result.returncode == test_case.expected_exit_code, (
        rerun_result.stdout + rerun_result.stderr
    )
    assert test_case.expected_window_fragment in rerun_result.stderr

    rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT COUNT(*) FROM main.hourly_activity_with_daily_context",
    )
    assert int(rows[0][0]) >= test_case.expected_row_count
