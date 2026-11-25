"""DuckDB E2E coverage: an explicit cursor window is fitted to each model's own cursor_start (CHI-1662)."""

import json
import subprocess
from pathlib import Path
from textwrap import dedent
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import CursorWindowFloorTestCase
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import (
    picked_plan_fields,
    window_floor_event_dates,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    run_sqb,
)

_MODEL: str = dedent(
    """
    MODEL (
      description "Events from {floor} on",
      materialized incremental,
      incremental_strategy delete_insert,
      incremental_mode microbatch,
      microbatch_strategy watermark,
      cursor_watermark_mode all,
      cursor event_date,
      cursor_type timestamp,
      cursor_grain day,
      cursor_inputs (
        events (column event_date, roles [filter, watermark]),
      ),
      batch_size 1mo,
      cursor_start "{floor}",
      columns (
        event_date (type DATE),
        value (type INTEGER),
      ),
    );

    SELECT
      CAST(event_date AS DATE) AS event_date,
      CAST(value AS INTEGER) AS value
    FROM __seed("events")
    """
).strip()

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": 'name = "window_floors"\nadapter = "duckdb"\n\n'
    '[connection]\ndatabase = "events.duckdb"\n',
    "sql/seeds/seeds.yml": (
        "seeds:\n- name: events\n  description: Test seed events.\n  columns:\n"
        "  - name: event_date\n    type: DATE\n  - name: value\n    type: INTEGER\n"
    ),
    "sql/seeds/events.csv": "event_date,value\n2020-06-01,1\n2021-06-01,2\n",
    "sql/models/staging/stg_events_early.sql": _MODEL.format(floor="2019-12-01") + "\n",
    "sql/models/staging/stg_events_late.sql": _MODEL.format(floor="2021-01-01") + "\n",
    "sql/models/staging/event_count.sql": (
        "MODEL (description 'Events counted.', materialized table);\n\n"
        'SELECT COUNT(*) AS events FROM __seed("events")\n'
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        CursorWindowFloorTestCase(
            description="a window overlapping the late floor starts the late model there, with a warning",
            select=("stg_events_early", "stg_events_late"),
            window=("2020-06-01", "2021-07-01"),
            expected_returncode=0,
            expected_output_fragments=("explicit start 2020-06-01",),
            expected_json_rows={},
            expected_warning_fragments={
                "stg_events_late": "is before cursor_start 2021-01-01; the window starts at"
            },
            expected_rows={
                "stg_events_early": ("2020-06-01", "2021-06-01"),
                "stg_events_late": ("2021-06-01",),
            },
        ),
        CursorWindowFloorTestCase(
            description="a window that ends at the late floor skips the late model, and the early one builds",
            select=("stg_events_early", "stg_events_late"),
            window=("2019-12-01", "2021-01-01"),
            expected_returncode=0,
            expected_output_fragments=(
                "Window before cursor_start",
                "window ends before cursor_start 2021-01-01",
            ),
            expected_json_rows={
                "stg_events_late": {
                    "action": "skip",
                    "reason": "window_before_cursor_start",
                    "skip_reason": "window ends before cursor_start 2021-01-01",
                }
            },
            expected_warning_fragments={},
            expected_rows={"stg_events_early": ("2020-06-01",)},
        ),
        CursorWindowFloorTestCase(
            description="one model whose window ends before its floor has nothing to build",
            select=("stg_events_late",),
            window=("2019-12-01", "2021-01-01"),
            expected_returncode=1,
            expected_output_fragments=(
                "error[P001]",
                "2019-12-01 to 2021-01-01 leaves nothing to build",
                "stg_events_late (cursor_start 2021-01-01)",
            ),
            expected_json_rows={},
            expected_warning_fragments={},
            expected_rows={},
        ),
        CursorWindowFloorTestCase(
            description="every cursor model skipped is nothing to build, whatever non-cursor models run",
            select=("stg_events_early", "stg_events_late", "event_count"),
            window=("2019-01-01", "2019-12-01"),
            expected_returncode=1,
            expected_output_fragments=(
                "error[P001]",
                "stg_events_early (cursor_start 2019-12-01)",
                "stg_events_late (cursor_start 2021-01-01)",
            ),
            expected_json_rows={},
            expected_warning_fragments={},
            expected_rows={},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_explicit_window_and_per_model_floors_when_building_then_each_model_is_clamped_or_skipped(
    test_case: CursorWindowFloorTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="window_floors", repo_files=_PROJECT_FILES
    )
    seeded: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--select", "events"), project_dir=project_dir
    )
    window: tuple[str, ...] = (
        "--start-cursor-ts",
        test_case.window[0],
        "--end-cursor-ts",
        test_case.window[1],
    )
    selection: tuple[str, ...] = ("--select", *test_case.select, *window)

    planned_json: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan", "--json", *selection), project_dir=project_dir
    )
    built: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", *selection), project_dir=project_dir
    )

    output: str = built.stdout + built.stderr
    assert seeded.returncode == 0, seeded.stdout + seeded.stderr
    assert built.returncode == test_case.expected_returncode, output
    assert all(fragment in output for fragment in test_case.expected_output_fragments), output
    document: dict[str, Any] = json.loads(planned_json.stdout)
    rows: dict[str, dict[str, Any]] = {model["name"]: model for model in document.get("models", [])}
    assert {
        name: picked_plan_fields(rows[name], tuple(expected))
        for name, expected in test_case.expected_json_rows.items()
    } == test_case.expected_json_rows, planned_json.stdout
    warnings: dict[str, str] = {
        warning.get("model_name", ""): warning["message"]
        for warning in document.get("warnings", [])
    }
    assert all(
        fragment in warnings.get(name, "")
        for name, fragment in test_case.expected_warning_fragments.items()
    ), planned_json.stdout
    assert {
        name: window_floor_event_dates(project_dir, name) for name in test_case.expected_rows
    } == test_case.expected_rows
