import json
import sys
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.cli.entry._test_types import HandoffTestCase, NativeCostTestCase
from tests.integration.src.sqlbuild.cli.entry.helpers import (
    PYTHON_CLI,
    LauncherRun,
    counting_environment,
    python_starts,
    run,
    write_cost_project,
)


@pytest.mark.skipif(sys.platform == "win32", reason="the counting environment is a POSIX script")
@pytest.mark.parametrize(
    "test_case",
    [
        NativeCostTestCase(
            "exact ascending decimals and newest tie",
            ("--sort", "cost", "--order", "asc"),
            ("run-negative", "run-tie", "run-small", "run-large", "run-pending"),
        ),
        NativeCostTestCase(
            "missing metrics stay last descending",
            ("--sort", "credits"),
            ("run-large", "run-tie", "run-small", "run-negative", "run-pending"),
        ),
        NativeCostTestCase(
            "duration differs from decimal order",
            ("--sort", "duration", "--order", "asc"),
            ("run-negative", "run-small", "run-large", "run-tie", "run-pending"),
        ),
        NativeCostTestCase(
            "whole day includes its final instant",
            ("--since", "2026-08-24", "--until", "2026-08-25"),
            ("run-large", "run-small"),
        ),
        NativeCostTestCase(
            "basic date and clock",
            ("--since", "20260824T000000Z", "--until", "20260825T000000Z"),
            ("run-large", "run-small"),
        ),
        NativeCostTestCase(
            "week date",
            ("--since", "2026-W35-1T00:00:00Z", "--until", "2026-W35-2T00:00:00Z"),
            ("run-large", "run-small"),
        ),
        NativeCostTestCase(
            "comma and long subsecond precision",
            ("--since", "2026-08-24T00:00:00,0000009Z", "--until", "2026-08-25T00:00:00.0000009Z"),
            ("run-large", "run-small"),
        ),
        NativeCostTestCase(
            "nonstandard separator and offset",
            ("--since", "2026-08-24X01:00:00+01:00", "--until", "2026-08-25X01:00:00+01:00"),
            ("run-large", "run-small"),
        ),
        NativeCostTestCase(
            "limit follows filtering and ordering",
            ("--sort", "cost", "--limit", "2"),
            ("run-large", "run-tie"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_persisted_cost_history_when_launching_then_exact_order_and_output_need_no_python(
    test_case: NativeCostTestCase, tmp_path: Path
) -> None:
    workspace: Path = write_cost_project(tmp_path / "workspace")
    marker: Path = tmp_path / "python-starts.log"
    _ = marker.write_text("", encoding="utf-8")
    program: Path = counting_environment(tmp_path / "environment", marker)
    args: tuple[str, ...] = ("cost", "history", "--json", *test_case.args)

    native: LauncherRun = run(program, args=args, cwd=workspace)
    python: LauncherRun = run(PYTHON_CLI, args=args, cwd=workspace)

    assert native == python
    assert native[0] == 0
    assert python_starts(marker) == 0
    payload: dict[str, Any] = json.loads(native[1])
    assert tuple(record["run_id"] for record in payload["runs"]) == test_case.expected_run_ids


@pytest.mark.skipif(sys.platform == "win32", reason="the counting environment is a POSIX script")
@pytest.mark.parametrize(
    "test_case",
    [HandoffTestCase("completed persisted cost", ("cost", "run-large"), 0)],
    ids=lambda case: case.description,
)
def test_given_completed_cost_when_reporting_then_native_text_json_and_file_match(
    test_case: HandoffTestCase,
    tmp_path: Path,
) -> None:
    workspace: Path = write_cost_project(tmp_path / "workspace")
    marker: Path = tmp_path / "python-starts.log"
    _ = marker.write_text("", encoding="utf-8")
    program: Path = counting_environment(tmp_path / "environment", marker)

    text: LauncherRun = run(program, args=test_case.args, cwd=workspace)
    assert text == run(PYTHON_CLI, args=test_case.args, cwd=workspace)
    assert text[0] == test_case.expected_exit_code
    document: LauncherRun = run(
        program,
        args=(*test_case.args, "--json", "--json-output", "reports/cost.json"),
        cwd=workspace,
    )
    assert document == run(PYTHON_CLI, args=(*test_case.args, "--json"), cwd=workspace)
    assert document[0] == 0
    assert (workspace / "reports" / "cost.json").read_bytes() == document[1]
    assert json.loads(document[1])["build_status"] == "success"
    assert python_starts(marker) == 0


@pytest.mark.skipif(sys.platform == "win32", reason="the counting environment is a POSIX script")
@pytest.mark.parametrize(
    "test_case",
    [
        NativeCostTestCase(
            "pending detail refresh", ("cost", "run-pending", "--json"), ("run-pending",)
        )
    ],
    ids=lambda case: case.description,
)
def test_given_pending_cost_when_selecting_detail_then_existing_adapter_refresh_still_runs(
    test_case: NativeCostTestCase,
    tmp_path: Path,
) -> None:
    workspace: Path = write_cost_project(tmp_path / "workspace")
    marker: Path = tmp_path / "python-starts.log"
    _ = marker.write_text("", encoding="utf-8")
    program: Path = counting_environment(tmp_path / "environment", marker)

    native: LauncherRun = run(program, args=test_case.args, cwd=workspace)

    assert native == run(PYTHON_CLI, args=test_case.args, cwd=workspace)
    assert native[0] == 0
    assert (json.loads(native[1])["run_id"],) == test_case.expected_run_ids
    assert python_starts(marker) == 1


@pytest.mark.skipif(sys.platform == "win32", reason="the counting environment is a POSIX script")
@pytest.mark.parametrize(
    "test_case",
    [
        HandoffTestCase("missing run", ("cost", "missing", "--json"), 1),
        HandoffTestCase("ambiguous prefix", ("cost", "run-", "--json"), 1),
        HandoffTestCase("invalid date", ("cost", "history", "--since", "invalid", "--json"), 1),
        HandoffTestCase(
            "naive datetime", ("cost", "history", "--since", "2026-08-24T00:00:00", "--json"), 1
        ),
        HandoffTestCase(
            "reversed bounds",
            ("cost", "history", "--since", "2026-08-25", "--until", "2026-08-23", "--json"),
            1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_cost_request_when_launching_then_coded_error_matches_without_python(
    test_case: HandoffTestCase,
    tmp_path: Path,
) -> None:
    workspace: Path = write_cost_project(tmp_path / "workspace")
    marker: Path = tmp_path / "python-starts.log"
    _ = marker.write_text("", encoding="utf-8")
    program: Path = counting_environment(tmp_path / "environment", marker)

    native: LauncherRun = run(program, args=test_case.args, cwd=workspace)

    assert native == run(PYTHON_CLI, args=test_case.args, cwd=workspace)
    assert native[0] == test_case.expected_exit_code
    assert native[1] == b""
    assert native[2].startswith(b"error[")
    assert python_starts(marker) == 0


@pytest.mark.skipif(sys.platform == "win32", reason="the counting environment is a POSIX script")
@pytest.mark.parametrize(
    "test_case",
    [
        HandoffTestCase(
            "output parent is a file",
            ("cost", "run-small", "--json-output", "reports/cost.json"),
            1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_cost_output_failure_when_launching_then_failure_never_retries_in_python(
    test_case: HandoffTestCase,
    tmp_path: Path,
) -> None:
    workspace: Path = write_cost_project(tmp_path / "workspace")
    _ = (workspace / "reports").write_text("keep this file", encoding="utf-8")
    marker: Path = tmp_path / "python-starts.log"
    _ = marker.write_text("", encoding="utf-8")
    program: Path = counting_environment(tmp_path / "environment", marker)

    native: LauncherRun = run(program, args=test_case.args, cwd=workspace)

    assert native[0] == test_case.expected_exit_code
    assert native[1] == b""
    assert b"cannot write" in native[2]
    assert (workspace / "reports").read_text(encoding="utf-8") == "keep this file"
    assert python_starts(marker) == 0
