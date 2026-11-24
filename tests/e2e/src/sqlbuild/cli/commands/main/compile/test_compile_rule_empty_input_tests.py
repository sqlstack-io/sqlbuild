"""E2E tests for the empty-input-only SQL test rule and its minimum-test interaction."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    EmptyInputTestRuleCacheTestCase,
    EmptyInputTestRuleCompileTestCase,
    EmptyInputTestRuleOptionErrorTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    build_empty_input_test_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_ALLOWED_TESTS: str = '["customers__reviewed_empty_input", "orders__retired_filler"]'
_FLAGGED_MESSAGE: str = (
    'unit test block 1 ("{name}") mocks only empty inputs and asserts only that no rows '
    "are produced"
)
_ORDERS_FILLER: tuple[str, str, str] = (
    "SQBRTEST203",
    "tests/unit/test_orders__empty_inputs_produce_no_rows.sql",
    _FLAGGED_MESSAGE.format(name="orders__empty_inputs_produce_no_rows"),
)
_LARGE_ORDERS_FILLER: tuple[str, str, str] = (
    "SQBRTEST203",
    "tests/unit/test_large_orders__empty_inputs_produce_no_rows.sql",
    _FLAGGED_MESSAGE.format(name="large_orders__empty_inputs_produce_no_rows"),
)
_STALE_ENTRY: tuple[str, str, str] = (
    "SQBRTEST203",
    "sqlbuild_project.toml",
    'stale allowed_tests entry "orders__retired_filler" names no existing SQL test',
)
_ORDERS_MINIMUM: tuple[str, str, str] = (
    "SQBRTEST202",
    "sql/models/orders.sql",
    'model "orders" has 0 tests; 1 required (1 empty-input-only test not counted; see SQBRTEST203)',
)


@pytest.mark.parametrize(
    "test_case",
    (
        EmptyInputTestRuleCompileTestCase(
            description="empty-input rule and minimum tests selected",
            select=("SQBRTEST202", "SQBRTEST203"),
            expected_diagnostics=frozenset(
                {_ORDERS_FILLER, _LARGE_ORDERS_FILLER, _STALE_ENTRY, _ORDERS_MINIMUM}
            ),
        ),
        EmptyInputTestRuleCompileTestCase(
            description="minimum tests selected alone still excludes filler",
            select=("SQBRTEST202",),
            expected_diagnostics=frozenset({_ORDERS_MINIMUM}),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_empty_input_only_tests_when_compiling_then_rules_report_filler_and_coverage(
    test_case: EmptyInputTestRuleCompileTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="empty_input_orders",
        repo_files=build_empty_input_test_project_files(
            select=test_case.select, allowed_tests_toml=_ALLOWED_TESTS
        ),
    )

    text_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )
    json_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile", "--json"), project_dir=project_dir
    )

    text_output: str = text_result.stdout + text_result.stderr
    assert text_result.returncode == 1, text_output
    for code, path, message in test_case.expected_diagnostics:
        assert f"error[{code}]: {message}" in text_result.stdout, text_output
        assert f"--> {path}:1:1" in text_result.stdout, text_output
    assert (
        f"4 models, 0 seeds, 0 functions, {len(test_case.expected_diagnostics)} error"
        in text_result.stdout
    ), text_output
    assert json_result.returncode == 1, json_result.stdout + json_result.stderr
    payload: dict[str, Any] = json.loads(json_result.stdout)
    assert payload["summary"]["errors"] == len(test_case.expected_diagnostics)
    assert {
        (diagnostic["code"], diagnostic["path"], diagnostic["message"])
        for diagnostic in payload["diagnostics"]
    } == test_case.expected_diagnostics


@pytest.mark.parametrize(
    "test_case",
    (
        EmptyInputTestRuleOptionErrorTestCase(
            description="string instead of a list",
            allowed_tests_toml='"customers__reviewed_empty_input"',
            expected_error="rule SQBRTEST203 option allowed_tests has the wrong type",
        ),
        EmptyInputTestRuleOptionErrorTestCase(
            description="non-string list items",
            allowed_tests_toml="[1]",
            expected_error=(
                "rule SQBRTEST203 option allowed_tests has item values of the wrong type"
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_allowed_tests_option_when_compiling_then_reports_config_error(
    test_case: EmptyInputTestRuleOptionErrorTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="empty_input_orders",
        repo_files=build_empty_input_test_project_files(
            select=("SQBRTEST202", "SQBRTEST203"),
            allowed_tests_toml=test_case.allowed_tests_toml,
        ),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 1, output
    assert test_case.expected_error in output, output


@pytest.mark.parametrize(
    "test_case",
    (
        EmptyInputTestRuleCacheTestCase(
            description="filler edited to mock rows",
            replacement_mock="SELECT 1 AS order_id, 5 AS amount",
            expected_first_codes=("SQBRTEST202",),
            expected_second_codes=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_warm_cache_when_filler_test_gains_rows_then_minimum_tests_recounts(
    test_case: EmptyInputTestRuleCacheTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="empty_input_orders",
        repo_files=build_empty_input_test_project_files(
            select=("SQBRTEST202",), allowed_tests_toml='["customers__reviewed_empty_input"]'
        ),
    )
    test_path: Path = project_dir / "tests/unit/test_orders__empty_inputs_produce_no_rows.sql"

    first: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile", "--json"), project_dir=project_dir
    )
    source: str = test_path.read_text(encoding="utf-8")
    empty_mock: str = (
        "SELECT CAST(NULL AS INTEGER) AS order_id, CAST(NULL AS INTEGER) AS amount\n  WHERE FALSE"
    )
    assert empty_mock in source
    test_path.write_text(source.replace(empty_mock, test_case.replacement_mock), encoding="utf-8")
    second: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile", "--json"), project_dir=project_dir
    )

    first_payload: dict[str, Any] = json.loads(first.stdout)
    second_payload: dict[str, Any] = json.loads(second.stdout)
    assert (
        tuple(diagnostic["code"] for diagnostic in first_payload["diagnostics"])
        == test_case.expected_first_codes
    ), first.stdout
    assert (
        tuple(diagnostic["code"] for diagnostic in second_payload["diagnostics"])
        == test_case.expected_second_codes
    ), second.stdout
