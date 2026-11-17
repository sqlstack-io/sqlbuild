"""Tests for deciding whether a main comparison run did not finish and needs an issue."""

import json
import subprocess
import sys
from pathlib import Path

import pytest

from tests.unit.scripts.release_performance_outcome._helpers._test_types import (
    UnfinishedComparisonTestCase,
)

_REPO_ROOT: Path = Path(__file__).resolve().parents[5]
# Annotations in the form GitHub writes them on failed, cancelled and timed-out jobs.
_JOB_TIMEOUT: tuple[str, str] = (
    "failure",
    "The job has exceeded the maximum execution time of 1h30m0s",
)
_SUPERSEDED: tuple[tuple[str, str], ...] = (
    (
        "failure",
        "Canceling since a higher priority waiting request for "
        "release-performance-pull_request-refs/pull/1515/merge exists",
    ),
    ("warning", "No files were found with the provided path: /home/runner/work/_temp/x."),
    ("failure", "Process completed with exit code 2."),
    ("failure", "The operation was canceled."),
)
_STEP_TIMEOUT: tuple[str, str] = (
    "failure",
    "The action 'Compare candidate and previous release on this runner' has timed out after "
    "65 minutes.",
)
_CANCELLED_BY_HAND: tuple[str, str] = ("failure", "The run was canceled forcefully by @someone.")


@pytest.mark.parametrize(
    "test_case",
    (
        UnfinishedComparisonTestCase(
            description="a job stopped by its own timeout is reported",
            result="cancelled",
            regressed="",
            annotations=(("warning", "Node.js 20 is deprecated."), _JOB_TIMEOUT),
            expected_output=(
                "The comparison job ended with result `cancelled`.\n"
                "- The job has exceeded the maximum execution time of 1h30m0s\n"
            ),
        ),
        UnfinishedComparisonTestCase(
            description="a step timeout fails the job and is reported",
            result="failure",
            regressed="",
            annotations=(_STEP_TIMEOUT,),
            expected_output=(
                "The comparison job ended with result `failure`.\n"
                "- The action 'Compare candidate and previous release on this runner' has timed "
                "out after 65 minutes.\n"
            ),
        ),
        UnfinishedComparisonTestCase(
            description="a lost runner fails the job and is reported",
            result="failure",
            regressed="",
            annotations=(
                (
                    "failure",
                    "The hosted runner: GitHub Actions 7 lost communication with the server.",
                ),
            ),
            expected_output=(
                "The comparison job ended with result `failure`.\n"
                "- The hosted runner: GitHub Actions 7 lost communication with the server.\n"
            ),
        ),
        UnfinishedComparisonTestCase(
            description="a run superseded through concurrency is not reported",
            result="cancelled",
            regressed="",
            annotations=_SUPERSEDED,
            expected_output="",
        ),
        UnfinishedComparisonTestCase(
            description="a run cancelled by hand is not reported",
            result="cancelled",
            regressed="",
            annotations=(_CANCELLED_BY_HAND,),
            expected_output="",
        ),
        UnfinishedComparisonTestCase(
            description="a regression fails the job but has its own issue",
            result="failure",
            regressed="true",
            annotations=(("failure", "Process completed with exit code 1."),),
            expected_output="",
        ),
        UnfinishedComparisonTestCase(
            description="a finished comparison is not reported",
            result="success",
            regressed="false",
            annotations=(),
            expected_output="",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_a_compare_job_result_when_classifying_then_reports_only_unfinished_runs(
    test_case: UnfinishedComparisonTestCase, tmp_path: Path
) -> None:
    annotations: Path = tmp_path / "annotations.json"
    _ = annotations.write_text(
        json.dumps(
            [
                {"annotation_level": level, "message": message}
                for level, message in test_case.annotations
            ]
        ),
        encoding="utf-8",
    )

    # -S drops site-packages: the report job runs this with the runner's Python and no project
    # dependencies installed.
    completed: subprocess.CompletedProcess[str] = subprocess.run(
        [
            sys.executable,
            "-S",
            "-m",
            "scripts.report_unfinished_release_performance",
            "--result",
            test_case.result,
            "--regressed",
            test_case.regressed,
            "--annotations",
            str(annotations),
        ],
        cwd=_REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )

    assert (completed.returncode, completed.stdout) == (0, test_case.expected_output), (
        completed.stderr
    )
