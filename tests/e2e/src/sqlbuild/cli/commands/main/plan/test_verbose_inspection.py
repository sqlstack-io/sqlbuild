"""E2E coverage for verbose warehouse inspection query output during plan."""

from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.plan._test_types import (
    VerboseInspectionPlanE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.plan.helpers import (
    prepare_changed_incremental_diamond_project,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb

_INSPECTION_LINE: re.Pattern[str] = re.compile(r"^  inspect +\d+\.\d{2}s  \d+ rows?  \S", re.M)
_METADATA_LINE: re.Pattern[str] = re.compile(r"^  inspect .*list_relations\(", re.M)
_CURSOR_LINE: re.Pattern[str] = re.compile(
    r"^  inspect .*SELECT (?:\d+ AS _read, )?CAST\((?:MIN|MAX)\(", re.M
)
_TOTAL_LINE: re.Pattern[str] = re.compile(r"^Inspection queries: (\d+) \(", re.M)


@pytest.mark.parametrize(
    "test_case",
    [
        VerboseInspectionPlanE2ETestCase(
            description="verbose plan logs each inspection read to stderr",
            command=("--no-color", "plan", "-v"),
            expected_inspection_output=True,
            expected_json_stdout=False,
        ),
        VerboseInspectionPlanE2ETestCase(
            description="verbose json plan keeps stdout machine readable",
            command=("--no-color", "plan", "-v", "--json"),
            expected_inspection_output=True,
            expected_json_stdout=True,
        ),
        VerboseInspectionPlanE2ETestCase(
            description="default plan omits inspection reads",
            command=("--no-color", "plan"),
            expected_inspection_output=False,
            expected_json_stdout=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_built_project_when_planning_then_verbose_reports_inspection_queries(
    test_case: VerboseInspectionPlanE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_changed_incremental_diamond_project(tmp_path=tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    reads: list[str] = _INSPECTION_LINE.findall(result.stderr)
    totals: list[str] = _TOTAL_LINE.findall(result.stderr)
    assert _INSPECTION_LINE.search(result.stdout) is None
    assert bool(reads) is test_case.expected_inspection_output
    assert bool(_METADATA_LINE.search(result.stderr)) is test_case.expected_inspection_output
    assert bool(_CURSOR_LINE.search(result.stderr)) is test_case.expected_inspection_output
    assert totals == [str(len(reads))] * test_case.expected_inspection_output
    assert result.stdout.lstrip().startswith("{") is test_case.expected_json_stdout


@pytest.mark.parametrize(
    "test_case",
    [
        VerboseInspectionPlanE2ETestCase(
            description="verbose json plan stdout parses as one json document",
            command=("--no-color", "plan", "-v", "--json"),
            expected_inspection_output=True,
            expected_json_stdout=True,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_verbose_json_plan_when_planning_then_stdout_parses_as_json(
    test_case: VerboseInspectionPlanE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_changed_incremental_diamond_project(tmp_path=tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert isinstance(json.loads(result.stdout), dict) is test_case.expected_json_stdout
    assert bool(_TOTAL_LINE.search(result.stderr)) is test_case.expected_inspection_output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
