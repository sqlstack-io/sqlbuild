"""DuckDB E2E coverage for timezone-aware microbatch cursors fed by DATE inputs."""

import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    AwareCursorDateWatermarkE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": dedent(
        """
        name = "aware_cursor_orders"
        adapter = "duckdb"

        [connection]
        database = "orders.duckdb"
        """
    ).strip()
    + "\n",
    "sql/sources/raw.yml": dedent(
        """
        sources:
          - name: raw_events
            description: Test source raw_events.
            schema: main
            table: raw_events
            columns:
              - name: event_id
                type: INTEGER
              - name: event_date
                type: DATE
        """
    ).strip()
    + "\n",
    "sql/models/events.sql": dedent(
        """
        MODEL (
          description "Events (web, store or phone); one row per event.",
          materialized incremental,
          incremental_strategy delete_insert,
          unique_key event_id,
          cursor observed_at,
          cursor_type timestamp,
          cursor_grain day,
          cursor_start "2026-01-01",
          incremental_mode microbatch,
          microbatch_strategy watermark,
          cursor_watermark_mode all,
          cursor_inputs (
            raw_events (column event_date, roles [filter, watermark]),
          ),
          batch_size 1d,
          lookback 1d,
          columns (
            event_id (type INTEGER),
            observed_at (type TIMESTAMP_TZ),
          ),
        );

        SELECT event_id, CAST(event_date AS TIMESTAMPTZ) AS observed_at
        FROM __source("raw_events")
        """
    ).strip()
    + "\n",
}


@pytest.mark.parametrize(
    "test_case",
    (
        AwareCursorDateWatermarkE2ETestCase(
            description="plan and rebuild without a batch limit",
            plan_args=(),
            expected_plan_fragment="batches  4 x 1d",
            expected_event_ids=(1, 2, 3, 4, 5),
        ),
        AwareCursorDateWatermarkE2ETestCase(
            description="plan and rebuild with a batch limit",
            plan_args=("--max-microbatches", "50"),
            expected_plan_fragment="batches  4 x 1d",
            expected_event_ids=(1, 2, 3, 4, 5),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_aware_cursor_with_date_watermark_when_planning_and_rebuilding_then_batches_are_counted(
    tmp_path: Path,
    test_case: AwareCursorDateWatermarkE2ETestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="aware_cursor_orders", repo_files=_PROJECT_FILES
    )
    db_path: Path = project_dir / "orders.duckdb"
    execute_duckdb(
        db_path=db_path,
        sql=(
            "CREATE TABLE main.raw_events (event_id INTEGER, event_date DATE);"
            "INSERT INTO main.raw_events VALUES "
            "(1, '2026-01-01'), (2, '2026-01-02'), (3, '2026-01-03')"
        ),
    )
    first_build: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", *test_case.plan_args), project_dir=project_dir
    )
    assert first_build.returncode == 0, first_build.stdout + first_build.stderr
    execute_duckdb(
        db_path=db_path,
        sql="INSERT INTO main.raw_events VALUES (4, '2026-01-04'), (5, '2026-01-05')",
    )

    plan: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan", "--select", "events", *test_case.plan_args),
        project_dir=project_dir,
    )
    rebuild: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", *test_case.plan_args), project_dir=project_dir
    )

    assert plan.returncode == 0, plan.stdout + plan.stderr
    assert test_case.expected_plan_fragment in plan.stdout
    assert rebuild.returncode == 0, rebuild.stdout + rebuild.stderr
    assert (
        tuple(
            row[0]
            for row in query_duckdb(
                db_path=db_path, sql="SELECT event_id FROM main.events ORDER BY event_id"
            )
        )
        == test_case.expected_event_ids
    )


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-vv"]))
