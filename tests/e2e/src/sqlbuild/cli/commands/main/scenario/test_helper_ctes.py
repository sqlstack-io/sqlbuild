"""E2E tests for scenario expected output and assertions that read helper CTEs."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.scenario._test_types import (
    ScenarioHelperCteE2ETestCase,
    ScenarioHelperShadowingE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    build_helper_scenario_project_files,
    selected_fields,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_EXPECTED_ROWS: str = "expected_rows AS (\n  SELECT 'o1' AS order_id, FALSE AS is_cancelled\n)"
_CHAINED_HELPERS: str = (
    'flagged AS (\n  SELECT order_id, is_cancelled FROM __ref("order_flags")\n),\n\n'
    "placed AS (\n  SELECT order_id FROM __ref__orders WHERE status = 'placed'\n),\n\n"
    "placed_flags AS (\n"
    "  SELECT flagged.order_id, flagged.is_cancelled\n"
    "  FROM flagged JOIN placed ON flagged.order_id = placed.order_id\n"
    ")"
)
_PASSED_ASSERTION: dict[str, object] = {
    "name": "flags_match",
    "status": "success",
    "query_status": "completed",
    "failing_row_count": 0,
}


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioHelperCteE2ETestCase(
            description="an assertion reads a helper CTE",
            helper_ctes=_EXPECTED_ROWS,
            check_ctes=(
                "__assert__flags_match AS (\n"
                '  SELECT order_id FROM __ref("order_flags")\n'
                "  WHERE order_id NOT IN (SELECT order_id FROM expected_rows)\n"
                ")"
            ),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "PASS   warehouse",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
            expected_json_assertion_results=(_PASSED_ASSERTION,),
        ),
        ScenarioHelperCteE2ETestCase(
            description="expected output reads a helper CTE",
            helper_ctes=_EXPECTED_ROWS,
            check_ctes=(
                "__expected__order_flags AS (\n"
                "  SELECT order_id, 'placed' AS status, is_cancelled FROM expected_rows\n"
                ")"
            ),
            expected_exit_code=0,
            expected_stdout_fragments=("PASS=1  FAIL=0  TOTAL=1",),
            expected_json_expected_results=(
                {
                    "model_name": "order_flags",
                    "status": "success",
                    "comparison_status": "completed",
                    "mismatched_row_count": 0,
                    "missing_row_count": 0,
                },
            ),
        ),
        ScenarioHelperCteE2ETestCase(
            description="chained helpers read a model and a fixture by name",
            helper_ctes=_CHAINED_HELPERS,
            check_ctes=(
                "__assert__flags_match AS (\n"
                "  SELECT order_id FROM placed_flags WHERE is_cancelled\n"
                ")"
            ),
            expected_exit_code=0,
            expected_stdout_fragments=("PASS=1  FAIL=0  TOTAL=1",),
            expected_json_assertion_results=(_PASSED_ASSERTION,),
        ),
        ScenarioHelperCteE2ETestCase(
            description="an assertion through chained helpers reports its failing rows",
            helper_ctes=_CHAINED_HELPERS,
            check_ctes=(
                "__assert__flags_match AS (\n"
                "  SELECT order_id FROM placed_flags WHERE NOT is_cancelled\n"
                ")"
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "FAIL   1 row",
                "assertion flags_match returned 1 row",
            ),
            expected_json_assertion_results=(
                {
                    "status": "failed",
                    "query_status": "completed",
                    "failing_row_count": 1,
                    "error_code": "X508",
                },
            ),
        ),
        ScenarioHelperCteE2ETestCase(
            description="an assertion that fails to run reports its error without counts",
            helper_ctes=_EXPECTED_ROWS,
            check_ctes=(
                "__assert__flags_match AS (\n"
                '  SELECT order_id FROM __ref("order_flags")\n'
                "  WHERE order_id NOT IN (SELECT order_id FROM missing_rows)\n"
                ")"
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "ERROR",
                "[X507]",
                "missing_rows",
            ),
            unexpected_stdout_fragments=("0 rows", "failing_rows="),
            expected_json_assertion_results=(
                {
                    "status": "failed",
                    "query_status": "not_run",
                    "failing_row_count": None,
                    "error_code": "X507",
                },
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_scenario_helper_ctes_when_testing_then_checks_read_them(
    test_case: ScenarioHelperCteE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_helper_ctes",
        repo_files=build_helper_scenario_project_files(
            helper_ctes=test_case.helper_ctes, check_ctes=test_case.check_ctes
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
    check: dict[str, Any] = json.loads(structured.stdout)["checks"][0]
    expected_results: list[dict[str, Any]] = check["expected_results"]
    assertion_results: list[dict[str, Any]] = check["assertion_results"]
    assert (
        tuple(map(selected_fields, expected_results, test_case.expected_json_expected_results))
        == test_case.expected_json_expected_results
    )
    assert len(expected_results) == len(test_case.expected_json_expected_results)
    assert (
        tuple(map(selected_fields, assertion_results, test_case.expected_json_assertion_results))
        == test_case.expected_json_assertion_results
    )
    assert len(assertion_results) == len(test_case.expected_json_assertion_results)


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioHelperShadowingE2ETestCase(
            description="an assertion redefines a helper CTE name",
            check_ctes=(
                "__assert__flags_match AS (\n"
                "  WITH expected_rows AS (SELECT 'o2' AS order_id)\n"
                '  SELECT order_id FROM __ref("order_flags")\n'
                "  WHERE order_id NOT IN (SELECT order_id FROM expected_rows)\n"
                ")"
            ),
            expected_output_fragment="redefines helper CTE 'expected_rows'",
            unexpected_output_fragment="Duplicate CTE name",
        ),
        ScenarioHelperShadowingE2ETestCase(
            description="expected output redefines a helper CTE name",
            check_ctes=(
                "__expected__order_flags AS (\n"
                "  WITH expected_rows AS (SELECT 'o1' AS order_id, FALSE AS is_cancelled)\n"
                "  SELECT order_id, 'placed' AS status, is_cancelled FROM expected_rows\n"
                ")"
            ),
            expected_output_fragment="redefines helper CTE 'expected_rows'",
            unexpected_output_fragment="Duplicate CTE name",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_scenario_check_redefining_a_helper_when_compiling_then_reports_a_compile_error(
    test_case: ScenarioHelperShadowingE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_helper_shadowing",
        repo_files=build_helper_scenario_project_files(
            helper_ctes=_EXPECTED_ROWS, check_ctes=test_case.check_ctes
        ),
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    output: str = compiled.stdout + compiled.stderr
    assert compiled.returncode != 0, output
    assert test_case.expected_output_fragment in output, output
    assert test_case.unexpected_output_fragment not in output, output
