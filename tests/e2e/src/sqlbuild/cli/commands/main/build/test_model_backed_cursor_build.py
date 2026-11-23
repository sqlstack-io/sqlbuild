"""E2E regression tests for model-backed cursor build workflows."""

from __future__ import annotations

from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    ModelBackedCursorBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
    table_exists,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ModelBackedCursorBuildE2ETestCase(
            description="fresh build succeeds for model-backed normal and microbatch cursor models",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                name = "model_backed_cursor_project"
                adapter = "duckdb"

                [connection]
                database = "regression.duckdb"

                [defaults]
                materialized = "table"
                    """
                ).strip()
                + "\n",
                "seed_raw_data.sql": dedent(
                    """
                    CREATE TABLE IF NOT EXISTS raw_orders (
                      id INTEGER,
                      customer_id INTEGER,
                      quantity INTEGER,
                      ordered_at TIMESTAMP,
                      status VARCHAR,
                      amount_cents INTEGER
                    );

                    INSERT INTO raw_orders VALUES
                      (1, 10, 1, '2026-01-01 00:30:00', 'completed', 100),
                      (2, 11, 2, '2026-01-01 01:30:00', 'completed', 200),
                      (3, 10, 1, '2026-01-02 02:00:00', 'placed', 150);
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
                    MODEL (description "Test model stg_orders.",
                      materialized view
                    );

                    SELECT
                      id AS order_id,
                      customer_id,
                      quantity,
                      ordered_at,
                      status,
                      amount_cents
                    FROM __source("raw_orders")
                    """
                ).strip()
                + "\n",
                "sql/models/marts/fact_orders.sql": dedent(
                    """
                    MODEL (description "Test model fact_orders.",
                      materialized table
                    );

                    SELECT
                      order_id,
                      customer_id,
                      quantity,
                      ordered_at,
                      status AS order_status,
                      amount_cents AS line_total_cents
                    FROM __ref("stg_orders")
                    """
                ).strip()
                + "\n",
                "sql/models/intermediate/order_status_index.sql": dedent(
                    """
                    MODEL (description "Test model order_status_index.",
                      materialized incremental,
                      incremental_strategy delete_insert,
                      cursor order_id,
                      cursor_type integer,
                      cursor_inputs (
                        fact_orders order_id,
                      ),
                    );

                    SELECT
                      order_id,
                      customer_id,
                      order_status,
                      ordered_at,
                      line_total_cents
                    FROM __ref("fact_orders")
                    WHERE order_id >= __cursor_start()
                      AND order_id < __cursor_end()
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
                      microbatch_strategy watermark,
                      cursor_watermark_mode all,
                      cursor_inputs (
                        fact_orders_view (column ordered_at, roles [filter]),
                        fact_orders (column ordered_at, roles [watermark]),
                      ),
                      incremental_mode microbatch,
                      batch_size 1d,
                    );

                    SELECT
                      DATE_TRUNC('hour', ordered_at) AS activity_hour,
                      COUNT(*) AS orders_placed,
                      SUM(quantity) AS quantity_total,
                      SUM(line_total_cents) AS revenue_cents
                    FROM __ref("fact_orders_view")
                    WHERE ordered_at >= __cursor_start()
                      AND ordered_at < __cursor_end()
                    GROUP BY DATE_TRUNC('hour', ordered_at)
                    """
                ).strip()
                + "\n",
                "sql/models/marts/fact_orders_view.sql": dedent(
                    """
                    MODEL (description "Test model fact_orders_view.",
                      materialized view
                    );

                    SELECT * FROM __ref("fact_orders")
                    """
                ).strip()
                + "\n",
            },
            command=("--no-color", "build"),
            expected_exit_code=0,
            expected_table_names=("fact_orders", "order_status_index", "hourly_order_activity"),
            expected_query_results=(
                (
                    "SELECT order_id, customer_id FROM main.order_status_index ORDER BY order_id",
                    ((1, 10), (2, 11), (3, 10)),
                ),
                (
                    (
                        "SELECT CAST(activity_hour AS VARCHAR), orders_placed "
                        "FROM main.hourly_order_activity ORDER BY activity_hour"
                    ),
                    (
                        ("2026-01-01 00:00:00", 1),
                        ("2026-01-01 01:00:00", 1),
                        ("2026-01-02 02:00:00", 1),
                    ),
                ),
            ),
            expected_absent_runtime_fragments=("__SQB_CURSOR_START__", "__SQB_CURSOR_END__"),
            expected_full_refresh_exit_code=1,
            expected_full_refresh_error_fragment=(
                "model 'hourly_order_activity' was not rebuilt: its cursor inputs have no rows "
                "(fact_orders.ordered_at)"
            ),
            expected_full_refresh_counts=(
                ("fact_orders", 0),
                ("order_status_index", 0),
                ("hourly_order_activity", 3),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_inline_project_when_building_model_backed_cursor_models_then_it_succeeds(
    test_case: ModelBackedCursorBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="model_backed_cursor_project",
        repo_files=test_case.repo_files,
    )
    db_path: Path = project_dir / "regression.duckdb"

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute((project_dir / "seed_raw_data.sql").read_text(encoding="utf-8"))
    connection.close()

    result: object = run_sqb(command=test_case.command, project_dir=project_dir)

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr

    table_name: str
    for table_name in test_case.expected_table_names:
        assert table_exists(db_path=db_path, table_name=table_name)

    query: str
    expected_rows: tuple[tuple[object, ...], ...]
    for query, expected_rows in test_case.expected_query_results:
        actual_rows: list[tuple[object, ...]] = query_duckdb(db_path=db_path, sql=query)
        assert tuple(tuple(row) for row in actual_rows) == expected_rows

    runtime_sql_paths: tuple[Path, ...] = (
        project_dir
        / "target"
        / "run"
        / "sql"
        / "models"
        / "intermediate"
        / "order_status_index.sql",
        project_dir / "target" / "run" / "sql" / "models" / "marts" / "hourly_order_activity.sql",
    )
    runtime_sql_path: Path
    for runtime_sql_path in runtime_sql_paths:
        runtime_sql: str = runtime_sql_path.read_text(encoding="utf-8")
        fragment: str
        for fragment in test_case.expected_absent_runtime_fragments:
            assert fragment not in runtime_sql

    connection = duckdb.connect(str(db_path))
    connection.execute("DELETE FROM raw_orders")
    connection.close()
    normal_incremental_path: Path = (
        project_dir / "sql" / "models" / "intermediate" / "order_status_index.sql"
    )
    normal_incremental_sql: str = normal_incremental_path.read_text(encoding="utf-8")
    normal_incremental_path.write_text(
        normal_incremental_sql.replace(
            "WHERE order_id >= __cursor_start()\n  AND order_id < __cursor_end()",
            "",
        ),
        encoding="utf-8",
    )

    full_refresh_result: object = run_sqb(
        command=("--no-color", "build", "--full-refresh"), project_dir=project_dir
    )

    assert full_refresh_result.returncode == test_case.expected_full_refresh_exit_code, (
        full_refresh_result.stdout + full_refresh_result.stderr
    )
    assert test_case.expected_full_refresh_error_fragment is not None
    assert test_case.expected_full_refresh_error_fragment in (
        full_refresh_result.stdout + full_refresh_result.stderr
    )
    expected_count: int
    for table_name, expected_count in test_case.expected_full_refresh_counts:
        assert table_exists(db_path=db_path, table_name=table_name)
        assert query_duckdb(db_path=db_path, sql=f"SELECT COUNT(*) FROM main.{table_name}") == [
            (expected_count,)
        ]
