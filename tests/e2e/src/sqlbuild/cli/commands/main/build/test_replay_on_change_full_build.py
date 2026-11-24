"""E2E tests for replay_on_change rebuilds of existing incremental models."""

from __future__ import annotations

from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    ReplayFullRebuildE2ETestCase,
    WindowLimitedReplayE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import (
    replay_order_rows,
    run_replay_command,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    query_duckdb,
)

_SOURCE_ROWS_SQL: str = (
    "CREATE TABLE raw_orders AS "
    "SELECT i AS order_id, TIMESTAMP '2026-01-01' + to_days(CAST(i - 1 AS INTEGER)) AS order_date, "
    "100 + i AS amount_cents FROM range(1, 61) AS t(i)"
)

_BASE_FILES: dict[str, str] = {
    "sqlbuild_project.toml": dedent(
        """
        name = "replay_full_project"
        adapter = "duckdb"

        [connection]
        database = "replay.duckdb"
        """
    ).lstrip(),
    "sql/sources/raw.yml": dedent(
        """
        sources:
          - name: raw_orders
            description: Test source raw_orders.
            schema: main
            table: raw_orders
        """
    ).lstrip(),
    "sql/models/order_regions.sql": "MODEL (description 'Test model order_regions.', materialized table);\n\nSELECT 1 AS region_id\n",
    "sql/models/stg_orders.sql": dedent(
        """
        MODEL (description "Test model stg_orders.",
          materialized incremental,
          incremental_strategy delete_insert,
          cursor order_date,
          cursor_type timestamp,
          cursor_grain day,
          lookback 3d,
          cursor_start "2026-01-01",
        );

        SELECT order_id, order_date, amount_cents
        FROM __source("raw_orders")
        """
    ).lstrip(),
}

_DAILY_ORDERS_CONFIG: str = dedent(
    """
    MODEL (description "Test model.",
      materialized incremental,
      incremental_strategy delete_insert,
      cursor order_date,
      cursor_type timestamp,
      cursor_grain day,
      lookback 4d,
      cursor_inputs (
        stg_orders order_date,
      ),
      cursor_start "2026-01-01",
      replay_on_change full,
    );
    """
).lstrip()

_DAILY_ORDERS_SQL: str = (
    _DAILY_ORDERS_CONFIG
    + "\nSELECT order_date, SUM(amount_cents) AS amount_cents\n"
    + 'FROM __ref("stg_orders")\nGROUP BY order_date\n'
)

_DAILY_ORDERS_WITH_UNLISTED_REF_SQL: str = (
    _DAILY_ORDERS_CONFIG
    + "\nSELECT o.order_date, SUM(o.amount_cents) AS amount_cents\n"
    + 'FROM __ref("stg_orders") AS o\n'
    + 'JOIN __ref("order_regions") AS r ON r.region_id = 1\n'
    + "GROUP BY o.order_date\n"
)


_SOURCE_ORDERS_SQL: str = dedent(
    """
        MODEL (description "Test model.",
          materialized incremental,
          incremental_strategy delete_insert,
          cursor order_date,
          cursor_type timestamp,
          cursor_grain day,
          lookback 4d,
          cursor_start "2026-01-01",
          replay_on_change full,
        );

        SELECT order_date, amount_cents, {revision} AS revision
        FROM __source("raw_orders")
        """
).lstrip()


_MICROBATCH_ORDERS_SQL: str = dedent(
    """
        MODEL (description "Test model.",
          materialized incremental,
          incremental_strategy delete_insert,
          incremental_mode microbatch,
          microbatch_strategy watermark,
          cursor_watermark_mode all,
          batch_size 7d,
          cursor order_date,
          cursor_type timestamp,
          cursor_grain day,
          lookback 4d,
          cursor_inputs (
            stg_orders (column order_date, roles [filter, watermark]),
          ),
          cursor_start "2026-01-01",
          replay_on_change full,
        );

        SELECT order_date, SUM(amount_cents) AS amount_cents, {revision} AS revision
        FROM __ref("stg_orders")
        GROUP BY order_date
        """
).lstrip()


_WINDOWED_ORDERS_SQL: str = dedent(
    """
        MODEL (description "Test model.",
          materialized incremental,
          incremental_strategy delete_insert,
          cursor order_date,
          cursor_type timestamp,
          cursor_grain day,
          lookback 4d,
          cursor_start "2026-01-01",
          replay_on_change bounded-3d,
        );

        SELECT order_date, amount_cents, {revision} AS revision
        FROM __source("raw_orders")
        """
).lstrip()


_FULL_HISTORY_RANGE: str = "range  2026-01-01 \u2192 2026-03-01"


@pytest.mark.parametrize(
    "test_case",
    [
        ReplayFullRebuildE2ETestCase(
            description="planner_resolved_model_backed_input_with_added_unlisted_ref",
            target_table="daily_orders",
            changed_model_path="sql/models/daily_orders.sql",
            changed_model_sql=_DAILY_ORDERS_WITH_UNLISTED_REF_SQL,
            rebuild_command=("--select", "daily_orders"),
            expected_plan_fragments=(
                "daily_orders",
                "full rebuild",
                _FULL_HISTORY_RANGE,
                "bounds  planner-resolved",
                "policy  replay_on_change=full",
            ),
        ),
        ReplayFullRebuildE2ETestCase(
            description="runtime_owned_bounds_when_upstream_incremental_runs_in_same_build",
            target_table="daily_orders",
            changed_model_path="sql/models/daily_orders.sql",
            changed_model_sql=_DAILY_ORDERS_WITH_UNLISTED_REF_SQL,
            rebuild_command=(),
            expected_plan_fragments=(
                "full rebuild",
                "bounds  computed at run time, once model stg_orders is up to date",
                "policy  replay_on_change=full",
            ),
        ),
        ReplayFullRebuildE2ETestCase(
            description="query_change_without_cursor_inputs",
            target_table="source_orders",
            changed_model_path="sql/models/source_orders.sql",
            changed_model_sql=_SOURCE_ORDERS_SQL.format(revision=2),
            rebuild_command=("--select", "source_orders"),
            expected_plan_fragments=(
                "full rebuild",
                _FULL_HISTORY_RANGE,
                "bounds  planner-resolved",
            ),
        ),
        ReplayFullRebuildE2ETestCase(
            description="microbatch_model",
            target_table="microbatch_orders",
            changed_model_path="sql/models/microbatch_orders.sql",
            changed_model_sql=_MICROBATCH_ORDERS_SQL.format(revision=2),
            rebuild_command=("--select", "microbatch_orders"),
            expected_plan_fragments=(
                "full rebuild",
                _FULL_HISTORY_RANGE,
                "batches  9 x 7d",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_existing_incremental_when_replay_on_change_full_rebuilds_then_history_matches_first_run(
    test_case: ReplayFullRebuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="replay_full_project",
        repo_files={
            **_BASE_FILES,
            "sql/models/daily_orders.sql": _DAILY_ORDERS_SQL,
            "sql/models/source_orders.sql": _SOURCE_ORDERS_SQL.format(revision=1),
            "sql/models/microbatch_orders.sql": _MICROBATCH_ORDERS_SQL.format(revision=1),
        },
    )
    db_path: Path = project_dir / "replay.duckdb"
    execute_duckdb(db_path=db_path, sql=_SOURCE_ROWS_SQL)
    run_replay_command(command=("build",), project_dir=project_dir)
    first_run_rows: list[tuple[object, ...]] = replay_order_rows(
        db_path=db_path, table=test_case.target_table
    )
    assert len(first_run_rows) == 60

    (project_dir / test_case.changed_model_path).write_text(
        test_case.changed_model_sql, encoding="utf-8"
    )
    plan_output: str = run_replay_command(
        command=("plan", *test_case.rebuild_command), project_dir=project_dir
    ).stdout
    fragment: str
    for fragment in test_case.expected_plan_fragments:
        assert fragment in plan_output, plan_output

    run_replay_command(command=("build", *test_case.rebuild_command), project_dir=project_dir)

    assert replay_order_rows(db_path=db_path, table=test_case.target_table) == first_run_rows

    fresh_project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="fresh_replay_project",
        repo_files={
            **_BASE_FILES,
            "sql/models/daily_orders.sql": _DAILY_ORDERS_SQL,
            "sql/models/source_orders.sql": _SOURCE_ORDERS_SQL.format(revision=1),
            "sql/models/microbatch_orders.sql": _MICROBATCH_ORDERS_SQL.format(revision=1),
            test_case.changed_model_path: test_case.changed_model_sql,
        },
    )
    fresh_db_path: Path = fresh_project_dir / "replay.duckdb"
    execute_duckdb(db_path=fresh_db_path, sql=_SOURCE_ROWS_SQL)
    run_replay_command(command=("build",), project_dir=fresh_project_dir)

    assert query_duckdb(
        db_path=db_path, sql=f"SELECT * FROM main.{test_case.target_table} ORDER BY 1"
    ) == query_duckdb(
        db_path=fresh_db_path, sql=f"SELECT * FROM main.{test_case.target_table} ORDER BY 1"
    )


@pytest.mark.parametrize(
    "test_case",
    [
        WindowLimitedReplayE2ETestCase(
            description="bounded_replay_rebuilds_only_bounded_window",
            changed_model_sql=_WINDOWED_ORDERS_SQL.format(revision=2),
            source_mutation_sql="SELECT 1",
            expected_plan_fragments=(
                "rebuild last 3d",
                "range  2026-02-27 \u2192 2026-03-01",
                "policy  replay_on_change=bounded-3d",
            ),
            changed_rows_sql="SELECT COUNT(*) FROM main.windowed_orders WHERE revision = 2",
            expected_changed_rows=3,
            expected_total_rows=60,
        ),
        WindowLimitedReplayE2ETestCase(
            description="unchanged_model_uses_incremental_lookback_window",
            changed_model_sql=_WINDOWED_ORDERS_SQL.format(revision=1),
            source_mutation_sql=(
                "UPDATE raw_orders SET amount_cents = amount_cents + 1000; "
                "INSERT INTO raw_orders VALUES "
                "(61, TIMESTAMP '2026-03-02', 1161), (62, TIMESTAMP '2026-03-03', 1162)"
            ),
            expected_plan_fragments=("range  2026-02-25 \u2192 2026-03-03",),
            changed_rows_sql=(
                "SELECT COUNT(*) FROM main.windowed_orders WHERE amount_cents >= 1000"
            ),
            expected_changed_rows=7,
            expected_total_rows=62,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_window_limited_run_when_building_existing_incremental_then_only_window_is_replayed(
    test_case: WindowLimitedReplayE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="replay_window_project",
        repo_files={
            **_BASE_FILES,
            "sql/models/windowed_orders.sql": _WINDOWED_ORDERS_SQL.format(revision=1),
        },
    )
    db_path: Path = project_dir / "replay.duckdb"
    execute_duckdb(db_path=db_path, sql=_SOURCE_ROWS_SQL)
    run_replay_command(command=("build",), project_dir=project_dir)

    (project_dir / "sql" / "models" / "windowed_orders.sql").write_text(
        test_case.changed_model_sql, encoding="utf-8"
    )
    execute_duckdb(db_path=db_path, sql=test_case.source_mutation_sql)
    plan_output: str = run_replay_command(
        command=("plan", "--select", "windowed_orders"), project_dir=project_dir
    ).stdout
    fragment: str
    for fragment in test_case.expected_plan_fragments:
        assert fragment in plan_output, plan_output

    run_replay_command(command=("build", "--select", "windowed_orders"), project_dir=project_dir)

    assert query_duckdb(db_path=db_path, sql=test_case.changed_rows_sql) == [
        (test_case.expected_changed_rows,)
    ]
    assert query_duckdb(db_path=db_path, sql="SELECT COUNT(*) FROM main.windowed_orders") == [
        (test_case.expected_total_rows,)
    ]


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
