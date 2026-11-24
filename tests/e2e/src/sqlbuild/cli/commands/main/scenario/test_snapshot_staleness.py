"""E2E coverage of local snapshot staleness when the SQL behind a fixture changes."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.scenario._test_types import (
    SnapshotHelperEditTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    build_helper_fixture_scenario_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_SCENARIO_PATH: str = "tests/scenarios/orders__from_helper.sql"


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotHelperEditTestCase(
            description="editing a helper CTE the fixture reads makes the snapshot stale",
            repo_files=build_helper_fixture_scenario_project_files(),
            scenario_name="orders__from_helper",
            edited_path=_SCENARIO_PATH,
            edited_from="helper_ids AS (SELECT 1 AS order_id)",
            edited_to="helper_ids AS (SELECT 2 AS order_id)",
            expected_exit_code=1,
            expected_stdout_fragment="X602",
        ),
        SnapshotHelperEditTestCase(
            description="editing the expected output alone keeps the snapshot fresh",
            repo_files=build_helper_fixture_scenario_project_files(),
            scenario_name="orders__from_helper",
            edited_path=_SCENARIO_PATH,
            edited_from="__expected__stg_orders AS (SELECT 1 AS order_id)",
            edited_to="__expected__stg_orders AS (SELECT 1 AS order_id\n)",
            expected_exit_code=0,
            expected_stdout_fragment="PASS=1  FAIL=0  ERROR=0  SKIP=0  TOTAL=1",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_captured_snapshot_when_fixture_sql_changes_then_local_replay_reports_staleness(
    test_case: SnapshotHelperEditTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="helper_fixture", repo_files=test_case.repo_files
    )
    captured: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "capture", test_case.scenario_name),
        project_dir=project_dir,
    )
    assert captured.returncode == 0, captured.stdout + captured.stderr
    edited: Path = project_dir / test_case.edited_path
    _ = edited.write_text(
        edited.read_text(encoding="utf-8").replace(test_case.edited_from, test_case.edited_to),
        encoding="utf-8",
    )

    replayed: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "test", "--run-on", "local", test_case.scenario_name),
        project_dir=project_dir,
    )

    assert replayed.returncode == test_case.expected_exit_code, replayed.stdout + replayed.stderr
    assert test_case.expected_stdout_fragment in replayed.stdout
