"""Record a stateful planner baseline, check the same tree, and catch a changed plan."""

from __future__ import annotations

from pathlib import Path

import pytest

from scripts.planner_differential.main.differential import run_planner_differential
from tests.e2e.scripts.planner_differential._test_types import RecordCheckTestCase
from tests.e2e.scripts.planner_differential.helpers import (
    baseline_steps,
    harness_arguments,
    tamper_plan_exit_code,
)


@pytest.mark.parametrize(
    "test_case",
    [
        RecordCheckTestCase(
            description="edited_model_on_a_built_project",
            project="example/tidy-shop",
            steps=("initial", "edit-query"),
            tampered_step_index=1,
            expected_artifacts=("plan_json", "plan_output", "plan_text", "build", "state"),
            expected_clean_line="Planner differential passed: 1 projects matched the baseline",
            expected_difference_line="  - step edit-query: plan_json",
            expected_diff_line='+ "exit_code": 0,',
            expected_note="edited sql/models/daily_revenue.sql",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_recorded_baseline_when_checking_then_only_a_changed_plan_differs(
    test_case: RecordCheckTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    baseline_dir: Path = tmp_path / "baseline"
    record_arguments: list[str] = harness_arguments(
        mode="record", baseline_dir=baseline_dir, project=test_case.project, steps=test_case.steps
    )
    check_arguments: list[str] = harness_arguments(
        mode="check", baseline_dir=baseline_dir, project=test_case.project, steps=test_case.steps
    )

    assert run_planner_differential(record_arguments) == 0
    baseline: Path = baseline_dir / f"{test_case.project.replace('/', '__')}.json"
    edited: dict[str, object] = baseline_steps(baseline)[-1]
    assert edited["mutation"] == {"applied": True, "note": test_case.expected_note}
    assert all(edited[artifact] for artifact in test_case.expected_artifacts)
    _ = capsys.readouterr()

    assert run_planner_differential(check_arguments) == 0
    assert test_case.expected_clean_line in capsys.readouterr().out

    tamper_plan_exit_code(path=baseline, step_index=test_case.tampered_step_index)
    assert run_planner_differential(check_arguments) == 1
    output: str = capsys.readouterr().out
    assert test_case.expected_difference_line in output
    assert test_case.expected_diff_line in output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
