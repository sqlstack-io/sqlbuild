"""Run selectors read a past run from the local Recall history; audit: selects an audit by name."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest
from _pytest.capture import CaptureResult

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    RunSelectorPlanTestCase,
    RunSelectorRefusalTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import (
    recalled_run_ids,
    write_run_selector_project,
)


@pytest.mark.parametrize(
    "test_case",
    [
        RunSelectorPlanTestCase(
            description="failed: plans the failed models and the failed audit",
            selectors=("failed:{run}",),
            expected_models=["b", "c"],
            expected_audits=["a_matches_c"],
        ),
        RunSelectorPlanTestCase(
            description="failed:+ adds what reads the failed models, as rerunning from failure",
            selectors=("failed:{prefix}+",),
            expected_models=["b", "c", "d"],
            expected_audits=["a_matches_c"],
        ),
        RunSelectorPlanTestCase(
            description="run: less skipped: plans what the run selected but didn't skip",
            selectors=("run:{run}", "--exclude", "skipped:{run}"),
            expected_models=["a", "b", "c"],
            expected_audits=[],
        ),
        RunSelectorPlanTestCase(
            description="audit: plans one audit and no models",
            selectors=("audit:a_matches_c",),
            expected_models=[],
            expected_audits=["a_matches_c"],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_failed_local_build_when_planning_with_run_selectors_then_its_results_are_selected(
    test_case: RunSelectorPlanTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_run_selector_project(tmp_path)
    main(["--project-dir", str(tmp_path), "--no-color", "build", "--no-tests"])
    capsys.readouterr()
    [run_id] = recalled_run_ids(tmp_path)
    selectors: list[str] = [
        selector.format(run=run_id, prefix=run_id[:8]) for selector in test_case.selectors
    ]

    exit_code: int = main(
        ["--project-dir", str(tmp_path), "--no-color", "plan", "--json", *selectors]
    )
    captured: CaptureResult[str] = capsys.readouterr()

    assert exit_code == 0, captured.err
    plan: dict[str, Any] = json.loads(captured.out)
    assert (
        sorted(model["name"] for model in plan["models"]),
        plan["audits"],
    ) == (test_case.expected_models, test_case.expected_audits), captured.err


@pytest.mark.parametrize(
    "test_case",
    [
        RunSelectorRefusalTestCase(
            description="a run ID shorter than 8 characters is refused",
            selectors=("failed:{short}",),
            expected_error="is too short: give the full ID or at least 8 characters of it",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_failed_local_build_when_a_run_selector_is_too_short_then_the_plan_is_refused(
    test_case: RunSelectorRefusalTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_run_selector_project(tmp_path)
    main(["--project-dir", str(tmp_path), "--no-color", "build", "--no-tests"])
    capsys.readouterr()
    [run_id] = recalled_run_ids(tmp_path)
    selectors: list[str] = [selector.format(short=run_id[:7]) for selector in test_case.selectors]

    exit_code: int = main(["--project-dir", str(tmp_path), "--no-color", "plan", *selectors])
    captured: CaptureResult[str] = capsys.readouterr()

    assert (exit_code != 0, test_case.expected_error in captured.err + captured.out) == (
        True,
        True,
    ), test_case.description
