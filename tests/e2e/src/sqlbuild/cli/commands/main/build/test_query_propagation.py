"""E2E tests for query-change propagation behavior."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    QueryPropagationBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import take_raw_data_sql
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        QueryPropagationBuildE2ETestCase(
            description="warn-only query change does not propagate downstream",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
            name = "query_propagation_project"
            adapter = "duckdb"

            [connection]
            database = "propagation.duckdb"

            [defaults]
            materialized = "table"
                """
                ).strip()
                + "\n",
                "seed_raw_data.sql": dedent(
                    """
                CREATE TABLE IF NOT EXISTS raw_orders (
                  id INTEGER,
                  ordered_at TIMESTAMP,
                  amount_cents INTEGER
                );

                INSERT INTO raw_orders VALUES
                  (1, '2026-01-01 00:30:00', 100),
                  (2, '2026-01-01 01:30:00', 200);
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

                SELECT id AS order_id, ordered_at, amount_cents FROM __source("raw_orders")
                """
                ).strip()
                + "\n",
                "sql/models/marts/fact_orders.sql": dedent(
                    """
                MODEL (description "Test model fact_orders.", materialized table);

                SELECT order_id, ordered_at, amount_cents AS line_total_cents
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
                  replay_on_change bounded-14d
                );

                SELECT
                  DATE_TRUNC('day', activity_hour) AS activity_day,
                  SUM(orders_placed) AS orders_placed,
                  SUM(revenue_cents) AS revenue_cents
                FROM __ref("hourly_order_activity")
                GROUP BY DATE_TRUNC('day', activity_hour)
                """
                ).strip()
                + "\n",
            },
            initial_build_command=("--no-color", "build"),
            plan_command=("plan", "--json"),
            mutation_file="sql/models/marts/hourly_order_activity.sql",
            before_text="SUM(line_total_cents) AS revenue_cents",
            after_text="SUM(line_total_cents) + 0 AS revenue_cents",
            expected_exit_code=0,
            expected_reasons={
                "hourly_order_activity": "query_changed",
                "daily_activity_rollup": "normal_incremental",
            },
            expected_fingerprint_models=(
                "daily_activity_rollup",
                "fact_orders",
                "hourly_order_activity",
                "stg_orders",
            ),
        ),
        QueryPropagationBuildE2ETestCase(
            description="changed SQL UDF applies the direct caller's own full replay",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
            name = "function_query_propagation_project"
            adapter = "duckdb"

            [connection]
            database = "propagation.duckdb"
                """
                ).strip()
                + "\n",
                "seed_raw_data.sql": dedent(
                    """
                CREATE TABLE IF NOT EXISTS raw_orders (
                  id INTEGER,
                  amount_cents INTEGER
                );

                INSERT INTO raw_orders VALUES
                  (1, 100),
                  (2, 200);
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
                "sql/functions/sql/is_high_value_order.sql": dedent(
                    """
                FUNCTION (description "Test function is_high_value_order.",
                  arguments (amount_cents INTEGER),
                  returns BOOLEAN,
                );

                amount_cents > 100
                """
                ).strip()
                + "\n",
                "sql/models/fact_orders.sql": dedent(
                    """
                MODEL (description "Test model fact_orders.",
                  materialized incremental,
                  incremental_strategy delete_insert,
                  cursor order_id,
                  cursor_type integer,
                  cursor_inputs (
                    raw_orders id,
                  ),
                  unique_key order_id,
                  replay_on_change full
                );

                SELECT
                  id AS order_id,
                  amount_cents,
                  __udf("is_high_value_order")(amount_cents) AS is_high_value
                FROM __source("raw_orders")
                """
                ).strip()
                + "\n",
            },
            initial_build_command=("--no-color", "build"),
            plan_command=("plan", "--json"),
            mutation_file="sql/functions/sql/is_high_value_order.sql",
            before_text="amount_cents > 100",
            after_text="amount_cents >= 100",
            expected_exit_code=0,
            expected_reasons={"fact_orders": "function_changed"},
            expected_actions={"fact_orders": "create_table"},
            expected_fingerprint_models=("fact_orders", "is_high_value_order"),
        ),
        QueryPropagationBuildE2ETestCase(
            description="bounded query change does not propagate downstream",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
            name = "query_propagation_project"
            adapter = "duckdb"

            [connection]
            database = "propagation.duckdb"

            [defaults]
            materialized = "table"
                """
                ).strip()
                + "\n",
                "seed_raw_data.sql": dedent(
                    """
                CREATE TABLE IF NOT EXISTS raw_orders (
                  id INTEGER,
                  ordered_at TIMESTAMP,
                  amount_cents INTEGER
                );

                INSERT INTO raw_orders VALUES
                  (1, '2026-01-01 00:30:00', 100),
                  (2, '2026-01-01 01:30:00', 200);
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

                SELECT id AS order_id, ordered_at, amount_cents FROM __source("raw_orders")
                """
                ).strip()
                + "\n",
                "sql/models/marts/fact_orders.sql": dedent(
                    """
                MODEL (description "Test model fact_orders.", materialized table);

                SELECT order_id, ordered_at, amount_cents AS line_total_cents
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
                  replay_on_change bounded-14d
                );

                SELECT
                  DATE_TRUNC('hour', ordered_at) AS activity_hour,
                  COUNT(*) AS orders_placed,
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
                  replay_on_change bounded-14d
                );

                SELECT
                  DATE_TRUNC('day', activity_hour) AS activity_day,
                  SUM(orders_placed) AS orders_placed,
                  SUM(revenue_cents) AS revenue_cents
                FROM __ref("hourly_order_activity")
                GROUP BY DATE_TRUNC('day', activity_hour)
                """
                ).strip()
                + "\n",
            },
            initial_build_command=("--no-color", "build"),
            plan_command=("plan", "--json"),
            mutation_file="sql/models/marts/hourly_order_activity.sql",
            before_text="SUM(line_total_cents) AS revenue_cents",
            after_text="SUM(line_total_cents) + 0 AS revenue_cents",
            expected_exit_code=0,
            expected_reasons={
                "hourly_order_activity": "query_changed",
                "daily_activity_rollup": "normal_incremental",
            },
            expected_fingerprint_models=(
                "daily_activity_rollup",
                "fact_orders",
                "hourly_order_activity",
                "stg_orders",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_changed_upstream_when_planning_then_downstream_reason_matches_policy(
    test_case: QueryPropagationBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="query_propagation_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "propagation.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(take_raw_data_sql(project_dir))
    connection.close()

    initial_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.initial_build_command,
        project_dir=project_dir,
    )
    assert initial_result.returncode == test_case.expected_exit_code, (
        initial_result.stdout + initial_result.stderr
    )

    fingerprint_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT node_name FROM main._sqlbuild_fingerprints ORDER BY node_name",
    )
    assert tuple(row[0] for row in fingerprint_rows) == test_case.expected_fingerprint_models

    mutation_path: Path = project_dir / test_case.mutation_file
    original_text: str = mutation_path.read_text(encoding="utf-8")
    mutation_path.write_text(
        original_text.replace(test_case.before_text, test_case.after_text),
        encoding="utf-8",
    )

    try:
        plan_result: subprocess.CompletedProcess[str] = run_sqb(
            command=test_case.plan_command,
            project_dir=project_dir,
        )
        assert plan_result.returncode == test_case.expected_exit_code, (
            plan_result.stdout + plan_result.stderr
        )
        payload: dict[str, object] = json.loads(plan_result.stdout)
        entries_by_name: dict[str, dict[str, object]] = {
            str(entry["name"]): entry for entry in payload["models"]
        }
        assert len(entries_by_name) == len(payload["models"])
        reasons_by_name: dict[str, str] = {
            model_name: str(entries_by_name[model_name]["reason"])
            for model_name in test_case.expected_reasons
        }
        assert reasons_by_name == test_case.expected_reasons
        actions_by_name: dict[str, str] = {
            model_name: str(entries_by_name[model_name]["action"])
            for model_name in test_case.expected_actions
        }
        assert actions_by_name == test_case.expected_actions
    finally:
        mutation_path.write_text(original_text, encoding="utf-8")
