"""E2E coverage of text columns as timestamp cursors, which SQLite stores timestamps as."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.sqlite._test_types import (
    TextTimestampCursorE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_TWO_ORDERS: str = (
    "MODEL (description 'Test model orders.');\n"
    "SELECT 1 AS order_id, '2026-01-01 09:30:00' AS ordered_at\n"
    "UNION ALL SELECT 2 AS order_id, '2026-01-02 09:30:00' AS ordered_at\n"
)
_THREE_ORDERS: str = _TWO_ORDERS + (
    "UNION ALL SELECT 3 AS order_id, '2026-01-03 09:30:00' AS ordered_at\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        TextTimestampCursorE2ETestCase(
            description="sqlite accepts a text timestamp cursor and builds incrementally",
            adapter="sqlite",
            connection='path = "shop.sqlite"',
            day_expression="strftime('%Y-%m-%d 00:00:00', o.ordered_at)",
            expected_exit_code=0,
            expected_fragment="✓ Completed successfully  PASS=2",
        ),
        TextTimestampCursorE2ETestCase(
            description="duckdb still rejects a text timestamp cursor",
            adapter="duckdb",
            connection='database = "shop.duckdb"',
            day_expression="CAST(o.ordered_at AS VARCHAR)",
            expected_exit_code=1,
            expected_fragment="cursor_type TIMESTAMP does not match column 'order_day' type",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_text_timestamp_cursor_when_building_twice_then_only_sqlite_accepts_it(
    test_case: TextTimestampCursorE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": (
                f'name = "shop"\nadapter = "{test_case.adapter}"\n\n'
                f"[connection]\n{test_case.connection}\n\n"
                '[defaults]\nmaterialized = "table"\n\n[scopes]\nenforce_placement = false\n'
            ),
            "sql/models/orders.sql": _TWO_ORDERS,
            "sql/models/daily_orders.sql": (
                "MODEL (description 'Test model daily_orders.', materialized incremental,\n"
                "  incremental_strategy delete_insert, cursor order_day, cursor_type timestamp,\n"
                "  cursor_grain day, cursor_inputs (orders ordered_at));\n"
                f"SELECT o.order_id, {test_case.day_expression} AS order_day\n"
                'FROM __ref("orders") AS o\n'
            ),
        },
    )
    first: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    _ = (project_dir / "sql/models/orders.sql").write_text(_THREE_ORDERS, encoding="utf-8")

    second: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    output: str = first.stdout + first.stderr + second.stdout + second.stderr
    assert (first.returncode, second.returncode) == (
        test_case.expected_exit_code,
        test_case.expected_exit_code,
    ), output
    assert test_case.expected_fragment in output
