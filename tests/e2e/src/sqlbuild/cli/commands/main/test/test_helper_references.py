"""E2E tests for `__ref()` and mock references inside SQL-test helper CTEs."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import (
    HelperRedefinitionE2ETestCase,
    HelperReferenceE2ETestCase,
    HelperReferenceErrorE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.test.helpers import (
    build_helper_reference_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    (
        HelperReferenceE2ETestCase(
            description="helper and assertion read the model",
            tests=("helper_and_assertion_read_model",),
            command=("--no-color", "test", "--verbose"),
            sql_analysis=True,
            expected_output_fragments=("PASS=1  FAIL=0  TOTAL=1", "assertion doubles_amount"),
            expected_compiled_fragments=("\n__helper__doubled AS (",),
            expected_absent_compiled_fragments=("__ref(",),
        ),
        HelperReferenceE2ETestCase(
            description="only a helper reads the model",
            tests=("only_helper_reads_model",),
            command=("--no-color", "test", "--select", "item_totals"),
            sql_analysis=True,
            expected_output_fragments=("1 selected", "PASS=1  FAIL=0  TOTAL=1"),
            expected_compiled_fragments=("\n__ref__item_totals AS (",),
            expected_absent_compiled_fragments=("__ref(",),
        ),
        HelperReferenceE2ETestCase(
            description="helper reads a helper and a mock",
            tests=("helper_reads_helper_and_mock",),
            command=("--no-color", "test", "--verbose"),
            sql_analysis=True,
            expected_output_fragments=(
                "PASS=1  FAIL=0  TOTAL=1",
                "assertion doubles_mocked_amount",
            ),
            expected_compiled_fragments=("\n__helper__joined AS (",),
            expected_absent_compiled_fragments=("__ref(",),
        ),
        HelperReferenceE2ETestCase(
            description="inlined helpers read by expected rows and assertions",
            tests=("helper_reads_helper_and_mock",),
            command=("--no-color", "test", "--verbose"),
            sql_analysis=False,
            expected_output_fragments=(
                "PASS=1  FAIL=0  TOTAL=1",
                "assertion doubles_mocked_amount",
            ),
            expected_compiled_fragments=(
                "(WITH doubled AS (SELECT item_id, amount_doubled FROM __ref__item_totals)",
            ),
            expected_absent_compiled_fragments=("__ref(", "__helper__"),
        ),
        HelperReferenceE2ETestCase(
            description="helper and assertion read a mocked model",
            tests=("mocked_model_read_by_helper_and_assertion",),
            command=("--no-color", "test", "--verbose"),
            sql_analysis=True,
            expected_output_fragments=("PASS=1  FAIL=0  TOTAL=1", "assertion doubles_amount"),
            expected_compiled_fragments=("\n__helper__doubled AS (",),
            expected_absent_compiled_fragments=("__ref(",),
        ),
        HelperReferenceE2ETestCase(
            description="unread helpers and mocks are ignored",
            tests=("unread_helpers_and_mocks",),
            command=("--no-color", "test"),
            sql_analysis=True,
            expected_output_fragments=("PASS=1  FAIL=0  TOTAL=1",),
            expected_compiled_fragments=(),
            expected_absent_compiled_fragments=("archived_rows", "other_rows", "__ref("),
        ),
        HelperReferenceE2ETestCase(
            description="unread helpers add no tested models",
            tests=("unread_helpers_and_mocks",),
            command=("--no-color", "test", "--select", "item_extras"),
            sql_analysis=True,
            expected_output_fragments=("0 selected",),
            expected_compiled_fragments=(),
            expected_absent_compiled_fragments=("archived_rows", "other_rows"),
        ),
        HelperReferenceE2ETestCase(
            description="expected rows and assertion read a mock",
            tests=("mocked_model_read_by_expected_and_assertion",),
            command=("--no-color", "test", "--verbose"),
            sql_analysis=True,
            expected_output_fragments=(
                "PASS=1  FAIL=0  TOTAL=1",
                "assertion reads_mocked_items",
            ),
            expected_compiled_fragments=(
                "SELECT item_id, amount * 2 AS amount_doubled FROM __ref__items",
                "SELECT item_id FROM __ref__items WHERE item_id <> 7 OR amount <> 5",
            ),
            expected_absent_compiled_fragments=(
                "__ref(",
                "SELECT 1 AS item_id, 10 AS amount",
            ),
        ),
        HelperReferenceE2ETestCase(
            description="expected rows and assertion read a mock without sql analysis",
            tests=("mocked_model_read_by_expected_and_assertion",),
            command=("--no-color", "test", "--verbose"),
            sql_analysis=False,
            expected_output_fragments=(
                "PASS=1  FAIL=0  TOTAL=1",
                "assertion reads_mocked_items",
            ),
            expected_compiled_fragments=(
                "SELECT item_id, amount * 2 AS amount_doubled FROM __ref__items",
                "SELECT item_id FROM __ref__items WHERE item_id <> 7 OR amount <> 5",
            ),
            expected_absent_compiled_fragments=(
                "__ref(",
                "SELECT 1 AS item_id, 10 AS amount",
            ),
        ),
        HelperReferenceE2ETestCase(
            description="selecting a mocked model does not select its test",
            tests=("mocked_model_read_by_expected_and_assertion",),
            command=("--no-color", "test", "--select", "items"),
            sql_analysis=True,
            expected_output_fragments=("0 selected",),
            expected_compiled_fragments=(),
            expected_absent_compiled_fragments=("__ref(",),
        ),
        HelperReferenceE2ETestCase(
            description="expected rows read an unmocked model the test runs",
            tests=("expected_reads_unmocked_model",),
            command=("--no-color", "test", "--select", "item_extras"),
            sql_analysis=True,
            expected_output_fragments=(
                "1 selected",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
            expected_compiled_fragments=("amount * 2 AS amount_doubled FROM __ref__item_extras",),
            expected_absent_compiled_fragments=("__ref(",),
        ),
        HelperReferenceE2ETestCase(
            description="unread helper named like a column reads an unknown model",
            tests=("helper_named_like_column_reads_unknown_model",),
            command=("--no-color", "test"),
            sql_analysis=True,
            expected_output_fragments=("PASS=1  FAIL=0  TOTAL=1",),
            expected_compiled_fragments=(),
            expected_absent_compiled_fragments=("__ref(", "item_archive"),
        ),
        HelperReferenceE2ETestCase(
            description="unread helper named like a column reads an unknown model without sql analysis",
            tests=("helper_named_like_column_reads_unknown_model",),
            command=("--no-color", "test"),
            sql_analysis=False,
            expected_output_fragments=("PASS=1  FAIL=0  TOTAL=1",),
            expected_compiled_fragments=(),
            expected_absent_compiled_fragments=("__ref(", "item_archive"),
        ),
        HelperReferenceE2ETestCase(
            description="unread helper named like a column selects no model",
            tests=("helper_named_like_column_reads_model",),
            command=("--no-color", "test", "--select", "item_extras"),
            sql_analysis=True,
            expected_output_fragments=("0 selected",),
            expected_compiled_fragments=(),
            expected_absent_compiled_fragments=("__ref(", "__ref__item_extras"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_helper_cte_references_when_testing_then_helpers_read_the_tested_models(
    test_case: HelperReferenceE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="helper_reference_project",
        repo_files=build_helper_reference_project_files(
            tests=test_case.tests, sql_analysis=test_case.sql_analysis
        ),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )
    compiled: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 0, output
    assert compiled.returncode == 0, compiled.stdout + compiled.stderr
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output
    compiled_sql: str = "\n".join(
        path.read_text(encoding="utf-8")
        for path in sorted((project_dir / "target" / "compiled" / "tests").rglob("*.sql"))
    )
    for fragment in test_case.expected_compiled_fragments:
        assert fragment in compiled_sql, compiled_sql
    for fragment in test_case.expected_absent_compiled_fragments:
        assert fragment not in compiled_sql, compiled_sql


@pytest.mark.parametrize(
    "test_case",
    (
        HelperReferenceErrorE2ETestCase(
            description="helpers reading each other",
            test_name="helper_cycle",
            expected_code="P013",
            expected_message=(
                "SQL test CTE 'first_rows' reads itself through 'first_rows' -> "
                "'second_rows' -> 'first_rows', so the test query cannot define its CTEs in "
                "dependency order"
            ),
            expected_line=7,
            expected_column=1,
            expected_help_fragment="shared_rows AS (SELECT ...)",
        ),
        HelperReferenceErrorE2ETestCase(
            description="helper reading an unknown model",
            test_name="helper_reads_unknown_model",
            expected_code="P013",
            expected_message=(
                "SQL test helper CTE 'missing_rows' references unknown model 'item_archive'"
            ),
            expected_line=7,
            expected_column=38,
            expected_help_fragment='__ref("<model>")',
        ),
        HelperReferenceErrorE2ETestCase(
            description="mock reading a helper that reads a model",
            test_name="mock_reads_referencing_helper",
            expected_code="P013",
            expected_message=(
                "SQL test mock '__ref__items' reads helper CTE 'base_rows', which calls "
                '__ref("item_totals"); mocks and fixtures are defined before the models the '
                "test runs, so the helper cannot be resolved for them"
            ),
            expected_line=7,
            expected_column=35,
            expected_help_fragment='FROM __ref__item_totals rather than FROM __ref("item_totals")',
        ),
        HelperReferenceErrorE2ETestCase(
            description="assertion reading an unmocked source",
            test_name="assertion_reads_unmocked_source",
            expected_code="P013",
            expected_message=(
                "SQL test assertion CTE '__assert__no_returns' calls "
                '__source("item_returns"), which the test does not mock, so the test query '
                "cannot resolve it"
            ),
            expected_line=11,
            expected_column=23,
            expected_help_fragment="__source__item_returns AS (SELECT ...)",
        ),
        HelperReferenceErrorE2ETestCase(
            description="test reading only mocks",
            test_name="assertion_reads_only_mocks",
            expected_code="P013",
            expected_message=(
                "SQL test 'assertion_reads_only_mocks' mocks the model it tests (__ref__items), "
                "so the test has no model to run against"
            ),
            expected_line=8,
            expected_column=23,
            expected_help_fragment=(
                "Mock the model's inputs instead (for example __source__<source> or "
                '__ref__<upstream model>) and keep __ref("items")'
            ),
        ),
        HelperReferenceErrorE2ETestCase(
            description="expected rows reading an unknown model",
            test_name="expected_reads_unknown_model",
            expected_code="P013",
            expected_message=(
                "SQL test expected CTE '__expected__item_totals' references unknown model "
                "'item_archive'"
            ),
            expected_line=8,
            expected_column=39,
            expected_help_fragment='__ref("<model>")',
        ),
        HelperReferenceErrorE2ETestCase(
            description="a mock a check calls reading a referencing helper through a middle helper",
            test_name="mock_called_reads_referencing_helper",
            expected_code="P013",
            expected_message=(
                "SQL test mock '__source__item_returns' reads helper CTE 'extras', which calls "
                '__ref("item_extras"); mocks and fixtures are defined before the models the '
                "test runs, so the helper cannot be resolved for them"
            ),
            expected_line=4,
            expected_column=40,
            expected_help_fragment='FROM __ref__item_extras rather than FROM __ref("item_extras")',
        ),
        HelperReferenceErrorE2ETestCase(
            description="a mock a check calls reading a referencing helper directly",
            test_name="mock_called_reads_referencing_helper_directly",
            expected_code="P013",
            expected_message=(
                "SQL test mock '__source__item_returns' reads helper CTE 'extras', which calls "
                '__ref("item_extras"); mocks and fixtures are defined before the models the '
                "test runs, so the helper cannot be resolved for them"
            ),
            expected_line=4,
            expected_column=40,
            expected_help_fragment='FROM __ref__item_extras rather than FROM __ref("item_extras")',
        ),
        HelperReferenceErrorE2ETestCase(
            description="test reading a mocked source and a mocked model",
            test_name="assertion_reads_mocked_source_and_model",
            expected_code="P013",
            expected_message=(
                "SQL test 'assertion_reads_mocked_source_and_model' mocks the model it tests "
                "(__ref__items), so the test has no model to run against"
            ),
            expected_line=10,
            expected_column=8,
            expected_help_fragment='and keep __ref("items") in the __assert__ or __expected__ CTE',
        ),
        HelperReferenceErrorE2ETestCase(
            description="test reading a mocked model by CTE name",
            test_name="assertion_reads_mock_by_name",
            expected_code="P013",
            expected_message=(
                "SQL test 'assertion_reads_mock_by_name' mocks the model it tests (__ref__items), "
                "so the test has no model to run against"
            ),
            expected_line=8,
            expected_column=23,
            expected_help_fragment='and call __ref("items") in the __assert__ or __expected__ CTE',
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_unresolvable_helper_references_when_compiling_then_compile_reports_them(
    test_case: HelperReferenceErrorE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="helper_reference_project",
        repo_files=build_helper_reference_project_files(tests=(test_case.test_name,)),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile", "--json"), project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 1, output
    payload: dict[str, Any] = json.loads(result.stdout)
    diagnostic: dict[str, Any] = payload["diagnostics"][0]
    assert diagnostic["code"] == test_case.expected_code, output
    assert diagnostic["message"] == test_case.expected_message, output
    assert diagnostic["location"]["path"] == f"tests/unit/test_{test_case.test_name}.sql"
    assert diagnostic["location"]["line"] == test_case.expected_line, output
    assert diagnostic["location"]["column"] == test_case.expected_column, output
    assert test_case.expected_help_fragment in diagnostic["help"], output


@pytest.mark.parametrize(
    "test_case",
    (
        HelperRedefinitionE2ETestCase(
            description="nested CTE redefining a helper",
            expected_output_fragments=("error[P001]", "which redefines helper CTE 'doubled'"),
            expected_absent_output_fragments=("P013",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_nested_cte_shadowing_a_helper_when_compiling_then_the_redefinition_is_reported(
    test_case: HelperRedefinitionE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="helper_reference_project",
        repo_files=build_helper_reference_project_files(tests=("nested_cte_shadows_helper",)),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 1, output
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output
    for fragment in test_case.expected_absent_output_fragments:
        assert fragment not in output, output
