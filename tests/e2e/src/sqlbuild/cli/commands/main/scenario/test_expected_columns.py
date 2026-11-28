"""E2E tests for scenario expected CTEs whose columns differ from their model's output."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.scenario._test_types import (
    ScenarioExpectedColumnsE2ETestCase,
    ScenarioExpectedColumnsHeaderE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    build_expected_columns_scenario_project_files,
    selected_fields,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioExpectedColumnsE2ETestCase(
            description="complete expected output compares every column",
            expected_sql="SELECT 'o1' AS order_id, 'placed' AS status, FALSE AS is_cancelled",
            expected_exit_code=0,
            expected_stdout_fragments=("PASS=1  FAIL=0  TOTAL=1",),
            expected_json_results=(
                {
                    "model_name": "order_flags",
                    "status": "success",
                    "comparison_status": "completed",
                    "actual_row_count": 1,
                    "expected_row_count": 1,
                    "mismatched_row_count": 0,
                    "missing_row_count": 0,
                },
            ),
        ),
        ScenarioExpectedColumnsE2ETestCase(
            description="listed columns are compared by name and unlisted columns ignored",
            expected_sql="SELECT 'placed' AS status, 'o1' AS order_id",
            expected_exit_code=0,
            expected_stdout_fragments=("PASS=1  FAIL=0  TOTAL=1",),
            unexpected_stdout_fragments=("X504", "S512"),
            expected_json_results=(
                {
                    "status": "success",
                    "comparison_status": "completed",
                    "actual_row_count": 1,
                    "expected_row_count": 1,
                    "mismatched_row_count": 0,
                    "missing_row_count": 0,
                },
            ),
        ),
        ScenarioExpectedColumnsE2ETestCase(
            description="a wrong value in a listed column fails with both difference counts",
            expected_sql="SELECT 'o1' AS order_id, 'shipped' AS status",
            expected_exit_code=1,
            expected_stdout_fragments=(
                "FAIL   2 rows differ",
                "expected rows of order_flags differ: 1 missing, 1 extra (1 expected, 1 actual)",
            ),
            unexpected_stdout_fragments=("X504",),
            expected_json_results=(
                {
                    "status": "failed",
                    "comparison_status": "completed",
                    "actual_row_count": 1,
                    "expected_row_count": 1,
                    "mismatched_row_count": 1,
                    "missing_row_count": 1,
                    "error_code": "X506",
                },
            ),
        ),
        ScenarioExpectedColumnsE2ETestCase(
            description="an extra expected row fails on row counts",
            expected_sql=(
                "SELECT 'o1' AS order_id, 'placed' AS status "
                "UNION ALL SELECT 'o2' AS order_id, 'placed' AS status"
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "expected rows of order_flags differ: 1 missing (2 expected, 1 actual)",
            ),
            expected_json_results=(
                {
                    "status": "failed",
                    "comparison_status": "completed",
                    "actual_row_count": 1,
                    "expected_row_count": 2,
                    "mismatched_row_count": 0,
                    "missing_row_count": 1,
                },
            ),
        ),
        ScenarioExpectedColumnsE2ETestCase(
            description="strict expected columns reject an unlisted column before execution",
            expected_sql="SELECT 'o1' AS order_id, 'placed' AS status",
            header_options=', expected_columns "all"',
            expected_exit_code=1,
            expected_stdout_fragments=(
                "[S512]",
                "does not list columns that",
                "model 'order_flags' outputs: is_cancelled",
            ),
            unexpected_stdout_fragments=("X504", "mismatched", "actual="),
        ),
        ScenarioExpectedColumnsE2ETestCase(
            description="strict expected columns accept every column in any order",
            expected_sql="SELECT FALSE AS is_cancelled, 'placed' AS status, 'o1' AS order_id",
            header_options=', expected_columns "all"',
            expected_exit_code=0,
            expected_stdout_fragments=("PASS=1  FAIL=0  TOTAL=1",),
            expected_json_results=(
                {
                    "status": "success",
                    "comparison_status": "completed",
                    "mismatched_row_count": 0,
                    "missing_row_count": 0,
                },
            ),
        ),
        ScenarioExpectedColumnsE2ETestCase(
            description="expected output listing a column the model lacks is rejected",
            expected_sql=(
                "SELECT 'o1' AS order_id, 'placed' AS status, FALSE AS is_cancelled, "
                "1 AS line_count"
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "[S512]",
                "'order_flags' does not output: line_count",
            ),
            unexpected_stdout_fragments=("X504", "mismatched"),
        ),
        ScenarioExpectedColumnsE2ETestCase(
            description="comparison that fails to run reports its error without counts",
            expected_sql=(
                "SELECT 'o1' AS order_id, 'placed' AS status, "
                "CAST('maybe' AS BOOLEAN) AS is_cancelled"
            ),
            expected_exit_code=1,
            expected_stdout_fragments=("expected  order_flags", "ERROR", "[X504]"),
            unexpected_stdout_fragments=("mismatched", "actual="),
            expected_json_results=(
                {
                    "model_name": "order_flags",
                    "status": "failed",
                    "comparison_status": "not_run",
                    "actual_row_count": None,
                    "expected_row_count": None,
                    "mismatched_row_count": None,
                    "missing_row_count": None,
                    "error_code": "X504",
                },
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_scenario_expected_columns_when_testing_then_comparison_is_reported_honestly(
    test_case: ScenarioExpectedColumnsE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_expected_columns",
        repo_files=build_expected_columns_scenario_project_files(
            expected_sql=test_case.expected_sql, header_options=test_case.header_options
        ),
    )

    text: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "test"), project_dir=project_dir
    )
    structured: subprocess.CompletedProcess[str] = run_sqb(
        command=("scenario", "test", "--json"), project_dir=project_dir
    )

    assert text.returncode == test_case.expected_exit_code, text.stdout + text.stderr
    assert structured.returncode == test_case.expected_exit_code, structured.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in text.stdout, text.stdout
    for fragment in test_case.unexpected_stdout_fragments:
        assert fragment not in text.stdout, text.stdout
    payload: dict[str, Any] = json.loads(structured.stdout)
    results: list[dict[str, Any]] = payload["checks"][0]["expected_results"]
    assert len(results) == len(test_case.expected_json_results), results
    assert (
        tuple(map(selected_fields, results, test_case.expected_json_results))
        == test_case.expected_json_results
    )


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioExpectedColumnsHeaderE2ETestCase(
            description="an unknown expected_columns value is rejected at discovery",
            header_options=', expected_columns "some"',
            expected_exit_code=1,
            expected_stderr_fragment="expected_columns in",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_expected_columns_option_when_testing_then_discovery_rejects_it(
    test_case: ScenarioExpectedColumnsHeaderE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_expected_columns",
        repo_files=build_expected_columns_scenario_project_files(
            expected_sql="SELECT 'o1' AS order_id", header_options=test_case.header_options
        ),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "test"), project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert test_case.expected_stderr_fragment in result.stderr, result.stderr
    assert 'must be "listed" or "all"' in result.stderr, result.stderr
