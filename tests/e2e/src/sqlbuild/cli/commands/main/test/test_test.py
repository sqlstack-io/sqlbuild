"""E2E tests for sqb test command."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import (
    ComplexValuesFixtureE2ETestCase,
    ExpectedColumnSubsetE2ETestCase,
    FixtureCompatibilityE2ETestCase,
    MissingMockCliE2ETestCase,
    ParameterCaseSelectionE2ETestCase,
    PartialFixtureE2ETestCase,
    SharedGraphChainE2ETestCase,
    SharedGraphMissingMockE2ETestCase,
    SqlAnalysisChainSqlTestE2ETestCase,
    SqlTestE2ETestCase,
    SqlTestFixtureValidationE2ETestCase,
    SqlTestInspectConflictE2ETestCase,
    SqlTestPlanInspectionE2ETestCase,
    TableFunctionFixtureE2ETestCase,
    UnknownTableFunctionFixtureE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.test.helpers import (
    MOCKED_UNSATISFIED_LEAF_TEST_SQL,
    build_assertion_test_project_files,
    build_chain_test_project_files,
    build_clause_partial_source_fixture_project_files,
    build_complex_values_fixture_project_files,
    build_contract_empty_fixture_project_files,
    build_cte_derived_output_fixture_project_files,
    build_cte_partial_source_fixture_project_files,
    build_deep_shared_missing_mock_project_files,
    build_diamond_chain_test_project_files,
    build_dynamic_pivot_test_project_files,
    build_empty_partial_fixture_project_files,
    build_expected_column_subset_project_files,
    build_explicit_typed_null_fixture_project_files,
    build_incompatible_fixture_type_project_files,
    build_invalid_partial_fixture_project_files,
    build_irrelevant_omitted_column_project_files,
    build_macro_test_project_files,
    build_missing_mock_columns_project_files,
    build_mixed_case_partial_fixture_project_files,
    build_mock_boundary_test_project_files,
    build_multiple_invalid_fixtures_project_files,
    build_open_expected_empty_fixture_project_files,
    build_open_schema_ref_fixture_project_files,
    build_open_source_empty_fixture_project_files,
    build_parameterized_test_project_files,
    build_partial_ref_fixture_project_files,
    build_partial_seed_fixture_project_files,
    build_partial_source_fixture_project_files,
    build_qualified_star_other_relation_project_files,
    build_recursive_ref_fixture_project_files,
    build_star_mock_fixture_project_files,
    build_star_partial_fixture_project_files,
    build_table_function_fixture_project_files,
    build_transformed_collection_project_files,
    build_union_distinct_expected_project_files,
    build_unsatisfied_leaf_test_project_files,
    build_unspecified_nullability_fixture_project_files,
    build_untyped_null_fixture_project_files,
    build_upper_case_star_import_partial_fixture_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    assert_fragments_in_order,
    prepare_inline_project,
    prepare_waffle_shop,
    run_sqb,
)

_DIAMOND_ONCE_RENDERED_FRAGMENTS: tuple[str, ...] = (
    "__ref__stg_orders AS (",
    "__ref__large_orders AS (",
    "__ref__small_orders AS (",
)
_DIAMOND_STDOUT_FRAGMENTS: tuple[str, ...] = (
    "PASS=1  FAIL=1  TOTAL=2",
    "expect  assertion no_negative_large_orders",
    "differing column large_count: actual=0, expected=3",
)


_ORDER_ROWS_BY_COLUMN_SUBSET: dict[str, str] = {
    "partial_match": (
        "SELECT 1 AS order_id, 'paid' AS status UNION ALL SELECT 2 AS order_id, 'open' AS status"
    ),
    "partial_wrong": (
        "SELECT 1 AS order_id, 'paid' AS status UNION ALL SELECT 2 AS order_id, 'closed' AS status"
    ),
    "reordered": (
        "SELECT 10 AS amount, 'paid' AS status, 100 AS customer_id, 1 AS order_id "
        "UNION ALL SELECT 14 AS amount, 'open' AS status, 200 AS customer_id, 2 AS order_id"
    ),
    "full_row": (
        "SELECT 1 AS order_id, 100 AS customer_id, 'paid' AS status, 10 AS amount "
        "UNION ALL SELECT 2 AS order_id, 200 AS customer_id, 'open' AS status, 14 AS amount"
    ),
}
_UNKNOWN_EXPECTED_COLUMN: dict[str, str] = {
    "unknown_column": (
        "SELECT 1 AS order_id, 'north' AS region UNION ALL SELECT 2 AS order_id, 'south' AS region"
    ),
}
_SUBSET_OUTCOME_FRAGMENTS: tuple[str, ...] = (
    "PASS=3  FAIL=1  TOTAL=4",
    "unexpected sample 1: order_id=2, status=open; missing sample 1: order_id=2, status=closed",
    "differing column status: actual=open, expected=closed",
)
_UNKNOWN_COLUMN_MESSAGE: str = (
    "expected output 'orders' lists columns that model 'orders' does not output: region"
)


@pytest.mark.parametrize(
    "test_case",
    (
        PartialFixtureE2ETestCase(
            description="known nullable source column receives an implicit typed null",
            repo_files=build_partial_source_fixture_project_files(),
            expected_stdout_fragment="PASS=1",
            expected_artifact_fragments=(
                'CAST(NULL AS VARCHAR) AS "status"',
                "__sqlbuild_partial_fixture",
            ),
        ),
        PartialFixtureE2ETestCase(
            description="nullable source column through CTE receives an implicit typed null",
            repo_files=build_cte_partial_source_fixture_project_files(),
            expected_stdout_fragment="PASS=1",
            expected_artifact_fragments=(
                'CAST(NULL AS VARCHAR) AS "status"',
                "__sqlbuild_partial_fixture",
            ),
        ),
        PartialFixtureE2ETestCase(
            description="nullable source column used by a filter receives an implicit typed null",
            repo_files=build_clause_partial_source_fixture_project_files(),
            expected_stdout_fragment="PASS=1",
            expected_artifact_fragments=(
                'CAST(NULL AS VARCHAR) AS "status"',
                "__sqlbuild_partial_fixture",
            ),
        ),
        PartialFixtureE2ETestCase(
            description="known source type with unspecified nullability receives a typed null",
            repo_files=build_unspecified_nullability_fixture_project_files(),
            expected_stdout_fragment="PASS=1",
            expected_artifact_fragments=(
                'CAST(NULL AS VARCHAR) AS "status"',
                "__sqlbuild_partial_fixture",
            ),
        ),
        PartialFixtureE2ETestCase(
            description="known nullable model-ref column receives an implicit typed null",
            repo_files=build_partial_ref_fixture_project_files(),
            expected_stdout_fragment="PASS=1",
            expected_artifact_fragments=(
                'CAST(NULL AS VARCHAR) AS "status"',
                "__sqlbuild_partial_fixture",
            ),
        ),
        PartialFixtureE2ETestCase(
            description="known nullable seed column receives an implicit typed null",
            repo_files=build_partial_seed_fixture_project_files(),
            expected_stdout_fragment="PASS=1",
            expected_artifact_fragments=(
                'CAST(NULL AS VARCHAR) AS "country_name"',
                "__sqlbuild_partial_fixture",
            ),
            artifact_filename="test_countries.sql",
        ),
        PartialFixtureE2ETestCase(
            description="nullable column outside the closure leaves fixture unchanged",
            repo_files=build_irrelevant_omitted_column_project_files(),
            expected_stdout_fragment="PASS=1",
            expected_artifact_fragments=("__source__raw_orders AS (SELECT 1 AS order_id)",),
            unexpected_artifact_fragments=("__sqlbuild_partial_fixture",),
        ),
        PartialFixtureE2ETestCase(
            description="contracted star closure receives all required nullable columns",
            repo_files=build_star_partial_fixture_project_files(),
            expected_stdout_fragment="PASS=1",
            expected_artifact_fragments=(
                'CAST(NULL AS VARCHAR) AS "status"',
                "__sqlbuild_partial_fixture",
            ),
        ),
        PartialFixtureE2ETestCase(
            description="zero-row partial fixture preserves emptiness after completion",
            repo_files=build_empty_partial_fixture_project_files(),
            expected_stdout_fragment="PASS=1",
            expected_artifact_fragments=(
                "FALSE",
                'CAST(NULL AS VARCHAR) AS "status"',
                "__sqlbuild_partial_fixture",
            ),
        ),
        PartialFixtureE2ETestCase(
            description="explicit typed null keeps complete fixture on unchanged path",
            repo_files=build_explicit_typed_null_fixture_project_files(),
            expected_stdout_fragment="PASS=1",
            expected_artifact_fragments=("CAST(NULL AS VARCHAR) AS status",),
            unexpected_artifact_fragments=("__sqlbuild_partial_fixture",),
        ),
        PartialFixtureE2ETestCase(
            description="supplied bare null receives authoritative fixture type",
            repo_files=build_untyped_null_fixture_project_files(),
            expected_stdout_fragment="PASS=1",
            expected_artifact_fragments=(
                'CAST("__sqlbuild_partial_fixture".ordered_at AS DATE) AS ordered_at',
                'CAST("__sqlbuild_partial_fixture".order_year AS BIGINT) AS order_year',
                "__sqlbuild_partial_fixture",
            ),
        ),
        PartialFixtureE2ETestCase(
            description="empty fixture receives complete authoritative relation shape",
            repo_files=build_contract_empty_fixture_project_files(),
            expected_stdout_fragment="PASS=1",
            expected_artifact_fragments=(
                'CAST(NULL AS INTEGER) AS "order_id"',
                'CAST(NULL AS VARCHAR) AS "status"',
                "FALSE",
            ),
            unexpected_artifact_fragments=("__empty_fixture",),
        ),
        PartialFixtureE2ETestCase(
            description="qualified star on real relation does not expand mocked relation",
            repo_files=build_qualified_star_other_relation_project_files(),
            expected_stdout_fragment="PASS=1",
            expected_artifact_fragments=("__source__raw_orders AS (",),
            unexpected_artifact_fragments=("__sqlbuild_partial_fixture",),
        ),
        PartialFixtureE2ETestCase(
            description="mixed-case unquoted supplied alias remains referenced through star",
            repo_files=build_mixed_case_partial_fixture_project_files(),
            expected_stdout_fragment="PASS=1",
            expected_artifact_fragments=(
                '"__sqlbuild_partial_fixture".*',
                'CAST(NULL AS VARCHAR) AS "note"',
            ),
            unexpected_artifact_fragments=('"__sqlbuild_partial_fixture"."Status"',),
        ),
        PartialFixtureE2ETestCase(
            description="upper-case reads through a star import CTE fill each column once",
            repo_files=build_upper_case_star_import_partial_fixture_project_files(),
            expected_stdout_fragment="PASS=1",
            expected_artifact_fragments=("__sqlbuild_partial_fixture",),
            single_artifact_fragments=('CAST(NULL AS VARCHAR) AS "status"',),
            artifact_filename="orders__keeps_status.sql",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_required_typed_column_omitted_when_testing_then_completes_fixture_implicitly(
    test_case: PartialFixtureE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="partial_fixture_project",
        repo_files=test_case.repo_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert test_case.expected_stdout_fragment in result.stdout
    artifacts: tuple[Path, ...] = tuple(
        (project_dir / "target").glob(f"**/{test_case.artifact_filename}")
    )
    assert artifacts
    artifact_sql: str = artifacts[0].read_text(encoding="utf-8")
    for fragment in test_case.expected_artifact_fragments:
        assert fragment in artifact_sql
    for fragment in test_case.unexpected_artifact_fragments:
        assert fragment not in artifact_sql
    for fragment in test_case.single_artifact_fragments:
        assert artifact_sql.count(fragment) == 1, artifact_sql


@pytest.mark.parametrize(
    "test_case",
    [
        SqlTestInspectConflictE2ETestCase(
            description="inspect rejects structured output file",
            expected_exit_code=2,
            expected_stderr_fragment=(
                "test --inspect cannot be combined with --json or --json-output"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_inspection_when_requesting_json_file_then_parser_rejects_conflict(
    test_case: SqlTestInspectConflictE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="inspect_output_conflict_project",
        repo_files=build_parameterized_test_project_files(),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("test", "--inspect", "--json-output", str(tmp_path / "result.json")),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code
    assert test_case.expected_stderr_fragment in result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        FixtureCompatibilityE2ETestCase(
            description="star mock fixture defers static shape validation",
            repo_files=build_star_mock_fixture_project_files(),
            expected_stdout_fragment="PASS=1",
        ),
        FixtureCompatibilityE2ETestCase(
            description="union distinct expected branches are validated per branch",
            repo_files=build_union_distinct_expected_project_files(),
            expected_stdout_fragment="PASS=1",
        ),
        FixtureCompatibilityE2ETestCase(
            description="aggregation output does not inherit scalar input type",
            repo_files=build_transformed_collection_project_files(),
            expected_stdout_fragment="PASS=1",
        ),
        FixtureCompatibilityE2ETestCase(
            description="open star-model schema accepts required fixture payload columns",
            repo_files=build_open_schema_ref_fixture_project_files(),
            expected_stdout_fragment="PASS=1",
        ),
        FixtureCompatibilityE2ETestCase(
            description="CTE-derived outputs require only their resolved source inputs",
            repo_files=build_cte_derived_output_fixture_project_files(),
            expected_stdout_fragment="PASS=1",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_valid_fixture_when_static_inference_is_ambiguous_then_test_still_executes(
    test_case: FixtureCompatibilityE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="fixture_compatibility_project",
        repo_files=test_case.repo_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert test_case.expected_stdout_fragment in result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        ComplexValuesFixtureE2ETestCase(
            description="duckdb executes complex string values",
            adapter_name="duckdb",
            command=("--no-color", "test"),
            expected_stdout_fragment="PASS=1",
        ),
        ComplexValuesFixtureE2ETestCase(
            description="snowflake compiles complex string values unchanged",
            adapter_name="snowflake",
            command=("--no-color", "compile"),
            expected_stdout_fragment="Project compiled",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_complex_strings_in_values_when_processing_then_literals_remain_intact(
    test_case: ComplexValuesFixtureE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="complex_values_project",
        repo_files=build_complex_values_fixture_project_files(adapter_name=test_case.adapter_name),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert test_case.expected_stdout_fragment in result.stdout
    artifacts: tuple[Path, ...] = tuple((project_dir / "target").glob("**/test_orders.sql"))
    assert artifacts
    artifact_sql: str = artifacts[0].read_text(encoding="utf-8")
    assert "alpha,beta [one] (two)" in artifact_sql
    assert "gamma(delta),[epsilon]" in artifact_sql


@pytest.mark.parametrize(
    "test_case",
    [
        SqlTestFixtureValidationE2ETestCase(
            description="missing source columns are reported together",
            repo_files=build_missing_mock_columns_project_files(),
            expected_stderr_fragments=(
                "tests/unit/test_orders.sql:4",
                "mock source 'raw_orders' is missing required columns: customer_id, status",
                "read by: orders",
            ),
        ),
        SqlTestFixtureValidationE2ETestCase(
            description="required non-nullable source column needs an explicit value",
            repo_files=build_invalid_partial_fixture_project_files(
                status_column_attributes="        type: VARCHAR\n        nullable: false\n"
            ),
            expected_stderr_fragments=("must provide required non-nullable columns: status",),
        ),
        SqlTestFixtureValidationE2ETestCase(
            description="required source column with unknown type is not guessed",
            repo_files=build_invalid_partial_fixture_project_files(
                status_column_attributes="        nullable: true\n"
            ),
            expected_stderr_fragments=(
                "missing required columns: status",
                "types are not authoritative",
            ),
        ),
        SqlTestFixtureValidationE2ETestCase(
            description="misspelled supplied source column is rejected for authoritative shape",
            repo_files=build_invalid_partial_fixture_project_files(
                status_column_attributes="        type: VARCHAR\n        nullable: true\n",
                supplied_column="order_identifer",
            ),
            expected_stderr_fragments=(
                "[B302] tests/unit/test_orders.sql:4:38:",
                "SQL test '__source__raw_orders' names unknown column 'order_identifer'",
            ),
        ),
        SqlTestFixtureValidationE2ETestCase(
            description="duckdb array and scalar types identify the expected column",
            repo_files=build_incompatible_fixture_type_project_files(adapter_name="duckdb"),
            expected_stderr_fragments=(
                "tests/unit/test_orders.sql:5",
                "expected output 'orders' column 'item_ids' has incompatible type VARCHAR",
                "resource type is ARRAY",
                "ARRAY_CONSTRUCT, or PARSE_JSON",
            ),
        ),
        SqlTestFixtureValidationE2ETestCase(
            description="snowflake array and scalar types identify the expected column",
            repo_files=build_incompatible_fixture_type_project_files(adapter_name="snowflake"),
            expected_stderr_fragments=(
                "tests/unit/test_orders.sql:5",
                "expected output 'orders' column 'item_ids' has incompatible type VARCHAR",
                "resource type is ARRAY",
                "ARRAY_CONSTRUCT, or PARSE_JSON",
            ),
        ),
        SqlTestFixtureValidationE2ETestCase(
            description="invalid fixtures are aggregated across selected tests",
            repo_files=build_multiple_invalid_fixtures_project_files(),
            expected_stderr_fragments=(
                "SQL test 'test_orders_a'",
                "tests/unit/test_orders_a.sql:4",
                "missing required columns: customer_id",
                "SQL test 'test_orders_b'",
                "tests/unit/test_orders_b.sql:4",
                "missing required columns: status",
            ),
        ),
        SqlTestFixtureValidationE2ETestCase(
            description="empty source fixture requires an authoritative shape",
            repo_files=build_open_source_empty_fixture_project_files(),
            expected_stderr_fragments=(
                "mock source 'raw_orders' uses __empty_fixture()",
                "column schema is not authoritative",
            ),
        ),
        SqlTestFixtureValidationE2ETestCase(
            description="empty expected fixture requires an authoritative shape",
            repo_files=build_open_expected_empty_fixture_project_files(),
            expected_stderr_fragments=(
                "expected output 'orders' uses __empty_fixture()",
                "column schema is not authoritative",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_fixture_when_testing_then_static_diagnostics_prevent_connection(
    test_case: SqlTestFixtureValidationE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="fixture_validation_project",
        repo_files=test_case.repo_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"),
        project_dir=project_dir,
    )

    assert result.returncode == 1, result.stdout + result.stderr
    for fragment in test_case.expected_stderr_fragments:
        assert fragment in result.stderr, result.stdout + result.stderr
    assert "Connecting to" not in result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        SqlTestPlanInspectionE2ETestCase(
            description="multi model plan shows real chain",
            repo_files=build_chain_test_project_files(sql_analysis_enabled=True),
            expected_stdout_fragments=(
                "Resolved test plan",
                "mocked sources: raw",
                "real models: stg_orders, fact_orders",
                "expected models: stg_orders, fact_orders",
                "Test plan inspection complete: 1 selected, 0 errors.",
            ),
        ),
        SqlTestPlanInspectionE2ETestCase(
            description="ref mock shows replacement boundary",
            repo_files=build_mock_boundary_test_project_files(),
            expected_stdout_fragments=(
                "mocked refs: stg_orders",
                "real models: int_orders, fact_orders",
                "boundary: stg_orders is replaced by __ref__stg_orders",
            ),
        ),
        SqlTestPlanInspectionE2ETestCase(
            description="recursive derived columns are not required from the mocked input",
            repo_files=build_recursive_ref_fixture_project_files(),
            expected_stdout_fragments=(
                "mocked refs: stg_order_links",
                "real models: order_roots",
                "Test plan inspection complete: 1 selected, 0 errors.",
            ),
        ),
        SqlTestPlanInspectionE2ETestCase(
            description="dynamic pivot uses compiler-proven fixture closure",
            repo_files=build_dynamic_pivot_test_project_files(),
            expected_stdout_fragments=(
                "mocked refs: stg_order_amounts",
                "real models: customer_category_amounts",
                "Test plan inspection complete: 1 selected, 0 errors.",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sql_test_when_inspecting_then_fixture_boundaries_and_chain_are_shown(
    test_case: SqlTestPlanInspectionE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="test_plan_inspection_project",
        repo_files=test_case.repo_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test", "--inspect"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout, result.stdout + result.stderr
    for fragment in test_case.unexpected_stdout_fragments:
        assert fragment not in result.stdout, result.stdout + result.stderr
    assert "Connecting to" not in result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        ParameterCaseSelectionE2ETestCase(
            description="named case runs without sibling cases",
            case_name="open_case",
            expected_exit_code=0,
            expected_stdout_fragments=("Test ready  1 selected", "PASS=1", "open_case"),
        ),
        ParameterCaseSelectionE2ETestCase(
            description="unknown case reports available selected cases",
            case_name="missing_case",
            expected_exit_code=1,
            expected_stderr_fragments=(
                "SQL test case 'missing_case' did not match the selected tests",
                "available cases: closed_case, open_case",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_parameterized_test_when_selecting_case_then_only_named_case_runs(
    test_case: ParameterCaseSelectionE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="parameter_case_project",
        repo_files=build_parameterized_test_project_files(),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "test",
            "--select",
            "orders",
            "--case",
            test_case.case_name,
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout, result.stdout + result.stderr
    for fragment in test_case.expected_stderr_fragments:
        assert fragment in result.stderr, result.stdout + result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        SqlTestE2ETestCase(
            description="test runs SQL unit tests and all pass",
            expected_exit_code=0,
            expected_stdout_fragment="PASS=5",
            expected_stdout_fragments=(
                "Execution  sqb test  (concurrency: 5)",
                "Connecting to duckdb (5 connections)...",
                "\u2713 Warehouse connected  duckdb",
                "expect  expected stg_orders",
                "expect  expected fact_orders",
                "expect  expected macro calculates_line_total_cents",
                "expect  expected udf detects_completed_orders",
                "expect  expected table_fn returns_customer_orders",
            ),
            expected_ordered_stdout_fragments=(
                "Compiling project...",
                "Compiled project. (<time>)",
                "Execution  sqb test  (concurrency: 5)",
                "Test ready  5 selected, 5 models",
                "Connecting to duckdb (5 connections)...",
                "\u2713 Warehouse connected  duckdb  (<time>)",
                "Preparing test functions...",
                "Prepared test functions. (<time>)",
                "fact_orders",
                "PASS=<n>  FAIL=<n>  TOTAL=<n>",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_waffle_shop_project_when_running_test_then_all_tests_pass(
    test_case: SqlTestE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)
    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    assert build_result.returncode == 0, build_result.stdout + build_result.stderr

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test", "--concurrency", "5"), project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert test_case.expected_stdout_fragment in result.stdout
    expected_fragment: str
    for expected_fragment in test_case.expected_stdout_fragments:
        assert expected_fragment in result.stdout
    assert_fragments_in_order(result.stdout, test_case.expected_ordered_stdout_fragments)
    assert "Inspecting warehouse state..." not in result.stdout
    assert "Generated plan." not in result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        SqlAnalysisChainSqlTestE2ETestCase(
            description="sql_analysis enabled chain test runs and writes generated ctes",
            sql_analysis_enabled=True,
            expected_artifact_fragments=(
                "__source__raw AS (",
                "__ref__stg_orders AS (",
                "__actual__fact_orders AS (",
                "FROM __ref__stg_orders",
                "'US' AS country",
                "' + x + ' AS literal_text",
                "'active' AS status",
                "'fact_orders' AS model_name",
            ),
            unexpected_artifact_fragments=(
                "__actual_0",
                "__actual__fact_orders AS (\n  SELECT\n    id,\n    amount + 1 AS adjusted\n  FROM (",
            ),
        ),
        SqlAnalysisChainSqlTestE2ETestCase(
            description="sql_analysis disabled chain test renders each upstream model once",
            sql_analysis_enabled=False,
            expected_artifact_fragments=(
                "__actual__fact_orders AS (",
                "__ref__stg_orders AS (",
                "FROM __ref__stg_orders",
                "'US' AS country",
                "' + x + ' AS literal_text",
                "'active' AS status",
                "'fact_orders' AS model_name",
            ),
            unexpected_artifact_fragments=("__actual_0",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_chain_sql_test_when_running_test_then_generated_sql_is_valid(
    test_case: SqlAnalysisChainSqlTestE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="chain_test_project",
        repo_files=build_chain_test_project_files(
            sql_analysis_enabled=test_case.sql_analysis_enabled
        ),
    )

    test_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"), project_dir=project_dir
    )

    assert test_result.returncode == 0, test_result.stdout + test_result.stderr
    assert "PASS=1" in test_result.stdout
    assert "expect  expected stg_orders" in test_result.stdout
    assert "expect  expected fact_orders" in test_result.stdout
    runtime_artifact_sql: str = (
        project_dir
        / "target"
        / "run"
        / "tests"
        / "_chain_"
        / "fact_orders__stg_orders"
        / "test_chain.sql"
    ).read_text(encoding="utf-8")
    expected_runtime_fragment: str
    for expected_runtime_fragment in test_case.expected_artifact_fragments:
        assert expected_runtime_fragment in runtime_artifact_sql

    compile_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    assert compile_result.returncode == 0, compile_result.stdout + compile_result.stderr
    artifact_sql: str = (
        project_dir
        / "target"
        / "compiled"
        / "tests"
        / "_chain_"
        / "fact_orders__stg_orders"
        / "test_chain.sql"
    ).read_text(encoding="utf-8")
    expected_fragment: str
    for expected_fragment in test_case.expected_artifact_fragments:
        assert expected_fragment in artifact_sql
    unexpected_fragment: str
    for unexpected_fragment in test_case.unexpected_artifact_fragments:
        assert unexpected_fragment not in artifact_sql


@pytest.mark.parametrize(
    "test_case",
    [
        SqlTestE2ETestCase(
            description="macro SQL unit test passes and writes direct comparison artifact",
            expected_exit_code=0,
            expected_stdout_fragment="PASS=1",
            expected_stdout_fragments=(
                "normalizes_status",
                "expect  expected macro normalizes_status",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_macro_sql_test_when_running_test_then_actual_and_expected_are_compared(
    test_case: SqlTestE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="macro_test_project",
        repo_files=build_macro_test_project_files(),
    )

    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
    )

    assert build_result.returncode == 0, build_result.stdout + build_result.stderr
    assert "PASS=2" in build_result.stdout

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert test_case.expected_stdout_fragment in result.stdout
    expected_fragment: str
    for expected_fragment in test_case.expected_stdout_fragments:
        assert expected_fragment in result.stdout
    runtime_artifact_sql: str = (
        project_dir
        / "target"
        / "run"
        / "tests"
        / "macro normalizes_status"
        / "normalizes_status.sql"
    ).read_text(encoding="utf-8")
    assert "__actual__macro_normalizes_status" in runtime_artifact_sql
    assert "LOWER(TRIM(raw_status)) AS status" in runtime_artifact_sql
    assert "__expected__macro_normalizes_status" in runtime_artifact_sql


@pytest.mark.parametrize(
    "test_case",
    [
        SqlTestE2ETestCase(
            description="assertion-only SQL unit test passes when assertion returns zero rows",
            expected_exit_code=0,
            expected_stdout_fragment="PASS=1",
            expected_stdout_fragments=("orders_assert", "expect  assertion no_negative_orders"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_assertion_only_sql_test_when_assertion_returns_zero_rows_then_it_passes(
    test_case: SqlTestE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="assertion_test_project",
        repo_files=build_assertion_test_project_files(failing=False),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert test_case.expected_stdout_fragment in result.stdout
    expected_fragment: str
    for expected_fragment in test_case.expected_stdout_fragments:
        assert expected_fragment in result.stdout
    runtime_artifact_sql: str = (
        project_dir / "target" / "run" / "tests" / "orders" / "orders_assert.sql"
    ).read_text(encoding="utf-8")
    assert "__assert__no_negative_orders AS" in runtime_artifact_sql
    assert "'assertion no_negative_orders' AS model_name" in runtime_artifact_sql


@pytest.mark.parametrize(
    "test_case",
    [
        SqlTestE2ETestCase(
            description="assertion-only SQL unit test fails when assertion returns rows",
            expected_exit_code=1,
            expected_stdout_fragment="FAIL=1",
            expected_stdout_fragments=(
                "orders_assert",
                "expect  assertion no_negative_orders",
                "FAIL  1 row",
                "test 'orders_assert' failed for models: assertion no_negative_orders",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_assertion_only_sql_test_when_assertion_returns_rows_then_it_fails(
    test_case: SqlTestE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="failing_assertion_test_project",
        repo_files=build_assertion_test_project_files(failing=True),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert test_case.expected_stdout_fragment in result.stdout
    expected_fragment: str
    for expected_fragment in test_case.expected_stdout_fragments:
        assert expected_fragment in result.stdout


@pytest.mark.parametrize(
    "test_case",
    (
        TableFunctionFixtureE2ETestCase(
            description="model test replaces complete table function invocation",
            expected_stdout_fragment="PASS=1",
            expected_artifact_fragments=(
                "WITH __helper__fixture_rows AS (SELECT 7 AS order_id)",
                "__table_fn__customer_orders AS (SELECT order_id FROM __helper__fixture_rows "
                "AS fixture_rows)",
                "FROM __table_fn__customer_orders",
                "__actual__orders",
            ),
            unexpected_artifact_fragments=("customer_orders(7)", "customer_id * 100"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_table_function_fixture_when_testing_model_then_fixture_replaces_invocation(
    test_case: TableFunctionFixtureE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="table_function_fixture_project",
        repo_files=build_table_function_fixture_project_files(),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert test_case.expected_stdout_fragment in result.stdout
    artifact_paths: tuple[Path, ...] = tuple(
        (project_dir / "target" / "run" / "tests").rglob("*.sql")
    )
    assert len(artifact_paths) == 1
    artifact_sql: str = artifact_paths[0].read_text(encoding="utf-8")
    for expected_fragment in test_case.expected_artifact_fragments:
        assert expected_fragment in artifact_sql
    for unexpected_fragment in test_case.unexpected_artifact_fragments:
        assert unexpected_fragment not in artifact_sql


@pytest.mark.parametrize(
    "test_case",
    (
        UnknownTableFunctionFixtureE2ETestCase(
            description="unknown table function fixture is rejected",
            expected_stderr_fragment="mocks unknown table function 'missing_orders'",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_unknown_table_function_fixture_when_testing_model_then_compile_fails(
    test_case: UnknownTableFunctionFixtureE2ETestCase,
    tmp_path: Path,
) -> None:
    repo_files: dict[str, str] = build_table_function_fixture_project_files()
    repo_files["tests/unit/test_orders.sql"] = repo_files["tests/unit/test_orders.sql"].replace(
        "__table_fn__customer_orders", "__table_fn__missing_orders"
    )
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="unknown_table_function_fixture_project",
        repo_files=repo_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"),
        project_dir=project_dir,
    )

    assert result.returncode == 1, result.stdout + result.stderr
    assert test_case.expected_stderr_fragment in result.stderr
    assert "Connecting to" not in result.stdout


@pytest.mark.parametrize(
    "test_case",
    (
        SharedGraphChainE2ETestCase(
            description="shared graph chain with sql analysis",
            sql_analysis_enabled=True,
            expected_stdout_fragments=_DIAMOND_STDOUT_FRAGMENTS,
            once_rendered_fragments=_DIAMOND_ONCE_RENDERED_FRAGMENTS,
        ),
        SharedGraphChainE2ETestCase(
            description="shared graph chain without sql analysis",
            sql_analysis_enabled=False,
            expected_stdout_fragments=_DIAMOND_STDOUT_FRAGMENTS,
            once_rendered_fragments=_DIAMOND_ONCE_RENDERED_FRAGMENTS,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_shared_upstream_graph_when_running_test_then_outcomes_and_sql_match_compile(
    test_case: SharedGraphChainE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="diamond_chain_project",
        repo_files=build_diamond_chain_test_project_files(
            sql_analysis_enabled=test_case.sql_analysis_enabled
        ),
    )

    test_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"), project_dir=project_dir
    )
    compile_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    assert test_result.returncode == 1, test_result.stdout + test_result.stderr
    assert compile_result.returncode == 0, compile_result.stdout + compile_result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in test_result.stdout, test_result.stdout
    runtime_sql: str = next(
        (project_dir / "target" / "run" / "tests").rglob("customer_order_mix_matches.sql")
    ).read_text(encoding="utf-8")
    compiled_sql: str = next(
        (project_dir / "target" / "compiled" / "tests").rglob("customer_order_mix_matches.sql")
    ).read_text(encoding="utf-8")
    assert runtime_sql == compiled_sql
    for fragment in test_case.once_rendered_fragments:
        assert runtime_sql.count(fragment) == 1, fragment


@pytest.mark.parametrize(
    "test_case",
    (
        SharedGraphMissingMockE2ETestCase(
            description="deep shared graph reports its missing source mock once",
            layers=12,
            expected_error=(
                "test 'deep_orders_missing_mock': model 'orders_00_left' references "
                '__source("raw_orders") which has no mock'
            ),
            expected_stdout_fragments=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_deep_shared_graph_missing_mock_when_testing_then_error_is_reported_once(
    test_case: SharedGraphMissingMockE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="deep_missing_mock_project",
        repo_files=build_deep_shared_missing_mock_project_files(layers=test_case.layers),
    )

    inspect_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test", "--inspect"), project_dir=project_dir
    )
    test_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"), project_dir=project_dir
    )

    for result in (inspect_result, test_result):
        output: str = result.stdout + result.stderr
        assert result.returncode == 1, output
        assert output.count("which has no mock") == 1, output
        assert test_case.expected_error in result.stderr, output
        assert "Connecting to" not in result.stdout, output
        for fragment in test_case.expected_stdout_fragments:
            assert fragment in result.stdout, output


@pytest.mark.parametrize(
    "test_case",
    (
        ExpectedColumnSubsetE2ETestCase(
            description="listed columns are compared by name with sql analysis",
            sql_analysis_enabled=True,
            command=("--no-color", "test"),
            expected_tests=_ORDER_ROWS_BY_COLUMN_SUBSET,
            expected_exit_code=1,
            expected_output_fragments=_SUBSET_OUTCOME_FRAGMENTS,
            unexpected_output_fragments=("customer_id=", "amount="),
        ),
        ExpectedColumnSubsetE2ETestCase(
            description="listed columns are compared by name without sql analysis",
            sql_analysis_enabled=False,
            command=("--no-color", "test"),
            expected_tests=_ORDER_ROWS_BY_COLUMN_SUBSET,
            expected_exit_code=1,
            expected_output_fragments=_SUBSET_OUTCOME_FRAGMENTS,
            unexpected_output_fragments=("customer_id=", "amount="),
        ),
        ExpectedColumnSubsetE2ETestCase(
            description="unknown expected column is a static error with sql analysis",
            sql_analysis_enabled=True,
            command=("--no-color", "test"),
            expected_tests=_UNKNOWN_EXPECTED_COLUMN,
            expected_exit_code=1,
            expected_output_fragments=(
                "[B302] tests/unit/unknown_column.sql:9:36:",
                "SQL test '__expected__orders' names unknown column 'region'",
            ),
            unexpected_output_fragments=("Connecting to", "Binder Error"),
        ),
        ExpectedColumnSubsetE2ETestCase(
            description="unknown expected column is explained at run time without sql analysis",
            sql_analysis_enabled=True,
            command=("--no-color", "test", "--no-sql-analysis"),
            expected_tests=_UNKNOWN_EXPECTED_COLUMN,
            expected_exit_code=1,
            expected_output_fragments=("[T005]", _UNKNOWN_COLUMN_MESSAGE),
            unexpected_output_fragments=("Binder Error",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_expected_cte_column_subset_when_testing_then_only_listed_columns_are_compared(
    test_case: ExpectedColumnSubsetE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="expected_column_subset_project",
        repo_files=build_expected_column_subset_project_files(
            expected_tests=test_case.expected_tests,
            sql_analysis_enabled=test_case.sql_analysis_enabled,
        ),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit_code, output
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output
    for fragment in test_case.unexpected_output_fragments:
        assert fragment not in output, output


@pytest.mark.parametrize(
    "test_case",
    (
        MissingMockCliE2ETestCase(
            description="missing source mock fails test and compile until mocked",
            expected_error=(
                "test 'test_chain': model 'stg_orders' references __source(\"raw\") which has no mock"
            ),
            compile_error_line=(
                "error[S000]: test 'test_chain': model 'stg_orders' references __source(\"raw\") "
                "which has no mock"
            ),
            compile_summary_fragment="1 error, 0 warnings",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_missing_mock_when_testing_and_compiling_then_error_is_reported_until_mocked(
    test_case: MissingMockCliE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="missing_mock_cli_project",
        repo_files=build_unsatisfied_leaf_test_project_files(),
    )

    test_results: tuple[subprocess.CompletedProcess[str], ...] = tuple(
        run_sqb(command=command, project_dir=project_dir)
        for command in (("--no-color", "test"), ("--no-color", "test", "--inspect"))
    )
    compile_results: tuple[subprocess.CompletedProcess[str], ...] = tuple(
        run_sqb(command=("--no-color", "compile"), project_dir=project_dir) for _ in range(2)
    )
    json_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile", "--json"), project_dir=project_dir
    )
    (project_dir / "tests" / "unit" / "test_chain.sql").write_text(
        MOCKED_UNSATISFIED_LEAF_TEST_SQL, encoding="utf-8"
    )
    fixed_compile: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )
    fixed_test: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"), project_dir=project_dir
    )

    for test_result in test_results:
        test_output: str = test_result.stdout + test_result.stderr
        assert test_result.returncode == 1, test_output
        assert test_output.count(test_case.expected_error) == 1, test_output
        assert "Connecting to" not in test_result.stdout, test_output
    for compile_result in compile_results:
        compile_output: str = compile_result.stdout + compile_result.stderr
        assert compile_result.returncode == 1, compile_output
        assert compile_result.stdout.count(test_case.compile_error_line) == 1, compile_output
        summary_line: str = compile_result.stdout.strip().splitlines()[-2]
        assert test_case.compile_summary_fragment in summary_line, compile_output
    assert json_result.returncode == 1, json_result.stdout + json_result.stderr
    payload: dict[str, Any] = json.loads(json_result.stdout)
    assert payload["has_errors"] is True
    assert payload["summary"]["errors"] == 1
    assert [
        (diagnostic["phase"], diagnostic["code"], diagnostic["message"])
        for diagnostic in payload["diagnostics"]
    ] == [("test", "S000", test_case.expected_error)]
    fixed_compile_output: str = fixed_compile.stdout + fixed_compile.stderr
    assert fixed_compile.returncode == 0, fixed_compile_output
    assert "which has no mock" not in fixed_compile_output
    assert "0 errors" in fixed_compile.stdout, fixed_compile_output
    fixed_test_output: str = fixed_test.stdout + fixed_test.stderr
    assert fixed_test.returncode == 0, fixed_test_output
    assert "PASS=1  FAIL=0  TOTAL=1" in fixed_test.stdout, fixed_test_output
