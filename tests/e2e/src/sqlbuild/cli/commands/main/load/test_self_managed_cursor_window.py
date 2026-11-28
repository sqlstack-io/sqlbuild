"""Real-CLI coverage of the cursor window a self-managed loader sees, and of window refusals."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.load._test_types import (
    LoaderCursorWindowRefusalE2ETestCase,
    SelfManagedCursorWindowE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.load.helpers import (
    build_self_managed_cursor_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

_DATABASE: str = "source_loader_schema_behavior.duckdb"


@pytest.mark.parametrize(
    "test_case",
    [
        SelfManagedCursorWindowE2ETestCase(
            description="without window flags a later load starts at the target's cursor",
            first_command=("--no-color", "load", "--select", "raw_events"),
            second_command=("--no-color", "load", "--select", "raw_events"),
            expected_windows=((1, "None", "None"), (2, "1", "None")),
        ),
        SelfManagedCursorWindowE2ETestCase(
            description="--start-cursor and --end-cursor set the window",
            first_command=("--no-color", "load", "--select", "raw_events"),
            second_command=(
                "--no-color",
                "load",
                "--select",
                "raw_events",
                "--start-cursor",
                "7",
                "--end-cursor",
                "9",
            ),
            expected_windows=((1, "None", "None"), (2, "7", "9")),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_self_managed_cursor_loader_when_loading_twice_then_it_sees_its_window(
    tmp_path: Path, test_case: SelfManagedCursorWindowE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_schema_behavior",
        repo_files=build_self_managed_cursor_project_files(),
    )

    first: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.first_command, project_dir=project_dir
    )
    second: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.second_command, project_dir=project_dir
    )

    assert first.returncode == 0, first.stdout + first.stderr
    assert second.returncode == 0, second.stdout + second.stderr
    windows: list[tuple[object, ...]] = query_duckdb(
        db_path=project_dir / _DATABASE,
        sql="SELECT load_seq, seen_start, seen_end FROM raw_events ORDER BY load_seq",
    )
    assert tuple(windows) == test_case.expected_windows


@pytest.mark.parametrize(
    "test_case",
    [
        LoaderCursorWindowRefusalE2ETestCase(
            description="a window flag with a loader that declares no cursor_column is refused",
            command=("--no-color", "load", "--start-cursor", "1", "--end-cursor", "5"),
            expected_return_code=1,
            expected_error_fragment="loader 'raw_codes' declares no cursor_column",
            expected_loaded_tables=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_loader_without_cursor_when_loading_with_a_window_then_the_run_is_refused(
    tmp_path: Path, test_case: LoaderCursorWindowRefusalE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_loader_schema_behavior",
        repo_files=build_self_managed_cursor_project_files(),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    assert result.returncode == test_case.expected_return_code, result.stdout + result.stderr
    assert test_case.expected_error_fragment in result.stdout + result.stderr
    tables: list[tuple[object, ...]] = query_duckdb(
        db_path=project_dir / _DATABASE,
        sql="SELECT table_name FROM information_schema.tables WHERE table_name LIKE 'raw_%'",
    )
    assert tuple(tables) == test_case.expected_loaded_tables
