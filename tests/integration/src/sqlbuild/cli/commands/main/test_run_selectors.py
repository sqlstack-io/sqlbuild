"""Run selectors read a past run from the local Recall history; audit: selects an audit by name."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import RunSelectorPlanTestCase
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
        RunSelectorPlanTestCase(
            description="a run ID shorter than 8 characters is refused",
            selectors=("failed:{short}",),
            expected_models=[],
            expected_audits=[],
            expected_error="is too short: give the full ID or at least 8 characters of it",
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
        selector.format(run=run_id, prefix=run_id[:8], short=run_id[:7])
        for selector in test_case.selectors
    ]

    exit_code: int = main(
        ["--project-dir", str(tmp_path), "--no-color", "plan", "--json", *selectors]
    )
    captured = capsys.readouterr()

    assert (exit_code == 0) == (test_case.expected_error is None), captured.err
    assert (
        test_case.expected_error is None or test_case.expected_error in captured.err + captured.out
    )
    plan: dict[str, Any] = (
        json.loads(captured.out) if exit_code == 0 else {"models": [], "audits": []}
    )
    assert (
        sorted(model["name"] for model in plan["models"]),
        plan["audits"],
    ) == (test_case.expected_models, test_case.expected_audits), captured.err
