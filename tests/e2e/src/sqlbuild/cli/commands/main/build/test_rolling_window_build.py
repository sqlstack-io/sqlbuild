"""DuckDB E2E coverage: a rolling-window microbatch model loads the current period and first-builds from cursor_start."""

import subprocess
from datetime import UTC, date, datetime, timedelta
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import RollingWindowBuildTestCase
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import (
    rolling_window_rows,
    rolling_window_seed,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    run_sqb,
)

_MODEL: str = dedent(
    """
    MODEL (
      description "Daily events in a rolling window",
      materialized incremental,
      incremental_strategy delete_insert,
      incremental_mode microbatch,
      microbatch_strategy rolling_window,
      cursor event_date,
      cursor_type timestamp,
      cursor_grain day,
      lookback 2d,
      batch_size 1d,{cursor_start}
      cursor_inputs (
        events event_date,
      ),
      columns (
        event_date (type DATE),
        value (type INTEGER),
      ),
    );

    SELECT
      CAST(event_date AS DATE) AS event_date,
      CAST(value AS INTEGER) AS value
    FROM __seed("events")
    WHERE event_date >= __cursor_start() AND event_date < __cursor_end()
    """
).strip()


@pytest.mark.parametrize(
    "test_case",
    [
        RollingWindowBuildTestCase(
            description="a first build starts at cursor_start, and each run replaces the lookback and today",
            cursor_start_line='\n      cursor_start "{start}",',
            cursor_start_days_ago=5,
            expected_first_build={5: 1, 4: 1, 3: 1, 2: 1, 1: 1, 0: 1},
            expected_second_build={5: 1, 4: 1, 3: 1, 2: 2, 1: 2, 0: 2},
        ),
        RollingWindowBuildTestCase(
            description="without cursor_start a first build covers the lookback and today",
            cursor_start_line="",
            cursor_start_days_ago=0,
            expected_first_build={2: 1, 1: 1, 0: 1},
            expected_second_build={2: 2, 1: 2, 0: 2},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_rolling_window_model_when_building_twice_then_the_current_period_is_replaced(
    test_case: RollingWindowBuildTestCase, tmp_path: Path
) -> None:
    today: date = datetime.now(UTC).date()
    cursor_start: str = test_case.cursor_start_line.format(
        start=today - timedelta(days=test_case.cursor_start_days_ago)
    )
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="rolling_events",
        repo_files={
            "sqlbuild_project.toml": 'name = "rolling_events"\nadapter = "duckdb"\n\n'
            '[connection]\ndatabase = "events.duckdb"\n',
            "sql/seeds/seeds.yml": (
                "seeds:\n- name: events\n  description: Daily events.\n  columns:\n"
                "  - name: event_date\n    type: DATE\n  - name: value\n    type: INTEGER\n"
            ),
            "sql/seeds/events.csv": rolling_window_seed(today, 1),
            "sql/models/daily_events.sql": _MODEL.format(cursor_start=cursor_start) + "\n",
        },
    )

    first: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert first.returncode == 0, first.stdout + first.stderr
    first_rows: dict[int, int] = rolling_window_rows(project_dir, today)
    (project_dir / "sql/seeds/events.csv").write_text(rolling_window_seed(today, 2))
    second: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert second.returncode == 0, second.stdout + second.stderr
    second_rows: dict[int, int] = rolling_window_rows(project_dir, today)

    assert first_rows == test_case.expected_first_build
    assert second_rows == test_case.expected_second_build
