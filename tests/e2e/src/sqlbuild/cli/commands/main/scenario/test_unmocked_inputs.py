"""Real-CLI coverage of scenarios that leave some of a model's inputs unmocked."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.scenario._test_types import (
    UnmockedInputScenarioE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    build_unmocked_input_scenario_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        UnmockedInputScenarioE2ETestCase(
            description="error names the unmocked input and the run ends with the failure",
            expected_fragments=(
                "model 'order_reps' reads 'stg_sales_reps', which is not mocked",
                "Add a CTE named __ref__stg_sales_reps AS (...)",
                "requires 8 sources without fixtures: raw_reps_0, raw_reps_1, raw_reps_2,",
                "raw_reps_3, raw_reps_4 and 3 more.",
                "Failures:\n\n  order_reps_unmocked  (scenario, warehouse)\n    error     [S504]",
            ),
            unexpected_fragment="requires source 'raw_reps_0'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_scenario_with_unmocked_model_input_when_testing_then_reports_the_boundary(
    test_case: UnmockedInputScenarioE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="unmocked_inputs",
        repo_files=build_unmocked_input_scenario_project_files(),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "test"), project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 1, output
    for fragment in test_case.expected_fragments:
        assert fragment in output, output
    assert test_case.unexpected_fragment not in output, output
    assert output.rstrip().splitlines()[-1].startswith("\u2717 Completed with errors"), output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
