"""E2E tests for compile-time rejection of built-in spellings the target dialect lacks."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import cast

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    FunctionNameCompileTestCase,
    UnknownFunctionCoverageTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_STARTS_WITH_FUNCTION: str = (
    "FUNCTION (description 'Test function.',\n"
    "  arguments (value STRING, prefix STRING),\n"
    "  returns BOOLEAN,\n"
    "  database warehouse,\n"
    "  schema analytics,\n"
    ");\n\n"
    "LEFT(value, LENGTH(prefix)) = prefix\n"
)

_SELECT: str = "SELECT\n"
_FROM: str = "\nFROM (SELECT 'anvil' AS product_name) AS catalog_items\n"
_SNOWFLAKE_PROJECT: str = 'name = "products"\nadapter = "snowflake"\n'
_DUCKDB_PROJECT: str = (
    'name = "products"\nadapter = "duckdb"\n\n[connection]\ndatabase = ":memory:"\n'
)


@pytest.mark.parametrize(
    "test_case",
    (
        FunctionNameCompileTestCase(
            description="snowflake STARTS_WITH is rejected with the Snowflake spelling",
            project_toml=_SNOWFLAKE_PROJECT,
            query_sql=_SELECT + "    STARTS_WITH(product_name, 'a') AS is_featured" + _FROM,
            expected_exit_code=1,
            expected_diagnostics=(
                (
                    "B101",
                    "Unknown function 'STARTS_WITH' for dialect Snowflake; did you mean STARTSWITH?",
                    4,
                    5,
                ),
            ),
        ),
        FunctionNameCompileTestCase(
            description="snowflake accepted spellings and an alias-named UDF compile",
            project_toml=_SNOWFLAKE_PROJECT,
            query_sql=(
                _SELECT + "    STARTSWITH(product_name, 'a') AS is_featured,\n"
                "    SUBSTR(product_name, 1, 2) AS short_name,\n"
                "    TIMESTAMPDIFF(day, '2026-01-01'::date, '2026-01-02'::date) AS age_days,\n"
                "    TRY_TO_DECIMAL('1.25', 10, 2) AS unit_price,\n"
                "    __udf(\"starts_with\")(product_name, 'a') AS has_prefix" + _FROM
            ),
            expected_exit_code=0,
            expected_diagnostics=(),
        ),
        FunctionNameCompileTestCase(
            description="snowflake CTE column list named like an alias compiles",
            project_toml=_SNOWFLAKE_PROJECT,
            query_sql="WITH day_of_week (dow) AS (SELECT 1) SELECT dow FROM day_of_week\n",
            expected_exit_code=0,
            expected_diagnostics=(),
        ),
        FunctionNameCompileTestCase(
            description="snowflake derived-table alias named like an alias compiles",
            project_toml=_SNOWFLAKE_PROJECT,
            query_sql="SELECT x FROM (VALUES (1)) day_of_week (x)\n",
            expected_exit_code=0,
            expected_diagnostics=(),
        ),
        FunctionNameCompileTestCase(
            description="duckdb STARTSWITH is rejected with the DuckDB spelling",
            project_toml=_DUCKDB_PROJECT,
            query_sql=_SELECT + "    STARTSWITH(product_name, 'a') AS is_featured" + _FROM,
            expected_exit_code=1,
            expected_diagnostics=(
                (
                    "B101",
                    "Unknown function 'STARTSWITH' for dialect DuckDB; did you mean STARTS_WITH?",
                    4,
                    5,
                ),
            ),
        ),
        FunctionNameCompileTestCase(
            description="duckdb materialized CTE column lists named like aliases compile",
            project_toml=_DUCKDB_PROJECT,
            query_sql=(
                "WITH day_of_week (d) AS MATERIALIZED (SELECT 1), "
                "day_of_month (d) AS NOT MATERIALIZED (SELECT 2) "
                "SELECT day_of_week.d FROM day_of_week CROSS JOIN day_of_month\n"
            ),
            expected_exit_code=0,
            expected_diagnostics=(),
        ),
        FunctionNameCompileTestCase(
            description="duckdb catalogue spellings compile",
            project_toml=_DUCKDB_PROJECT,
            query_sql=(
                _SELECT + "    starts_with(product_name, 'a') AS is_featured,\n"
                "    prefix(product_name, 'a') AS has_prefix,\n"
                "    substr(product_name, 1, 2) AS short_name" + _FROM
            ),
            expected_exit_code=0,
            expected_diagnostics=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_builtin_spelling_when_compiling_then_dialect_support_is_enforced(
    test_case: FunctionNameCompileTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="products",
        repo_files={
            "sqlbuild_project.toml": test_case.project_toml,
            "functions/sql/starts_with.sql": _STARTS_WITH_FUNCTION,
            "models/products.sql": (
                f"MODEL (description 'Test model products.', database warehouse, schema analytics);\n\n{test_case.query_sql}"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("compile", "--json", "--no-cache")
    )

    payload: dict[str, object] = json.loads(result.stdout)
    diagnostics: list[dict[str, object]] = cast(list[dict[str, object]], payload["diagnostics"])
    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert (
        tuple((item["code"], item["message"], item["line"], item["column"]) for item in diagnostics)
        == test_case.expected_diagnostics
    )


@pytest.mark.parametrize(
    "test_case",
    (
        UnknownFunctionCoverageTestCase(
            description="duckdb unknown function in WHERE is a partial-check note, not an error",
            project_toml=_DUCKDB_PROJECT,
            query_sql=_SELECT + "    product_name" + _FROM + "WHERE shop_margin(product_name) > 0\n",
            expected_exit_code=0,
            expected_codes=(),
            expected_reasons=("unknown function: shop_margin",),
            expected_notice="products: calls unknown function shop_margin; declare it as a project "
            "function so its calls are checked, or correct its spelling.",
        ),
        UnknownFunctionCoverageTestCase(
            description="snowflake unknown function is a partial-check note, not an error",
            project_toml=_SNOWFLAKE_PROJECT,
            query_sql=_SELECT + "    shop_margin(product_name) AS margin" + _FROM,
            expected_exit_code=0,
            expected_codes=(),
            expected_reasons=("some output types are unknown", "unknown function: shop_margin"),
            expected_notice="products: calls unknown function shop_margin",
        ),
        UnknownFunctionCoverageTestCase(
            description="a declared project function is known",
            project_toml=_DUCKDB_PROJECT,
            query_sql=_SELECT + "    __udf(\"starts_with\")(product_name, 'a') AS flagged" + _FROM,
            expected_exit_code=0,
            expected_codes=(),
            expected_reasons=(),
            expected_notice="0 errors",
        ),
        UnknownFunctionCoverageTestCase(
            description="a duckdb keyword-only built-in called with parentheses stays an error",
            project_toml=_DUCKDB_PROJECT,
            query_sql=_SELECT + "    CURRENT_TIMESTAMP() AS seen_at" + _FROM,
            expected_exit_code=1,
            expected_codes=("B101",),
            expected_reasons=(),
            expected_notice="error[B101]: Unknown function 'CURRENT_TIMESTAMP' for dialect DuckDB",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_unknown_function_when_compiling_then_checks_are_reported_partial(
    test_case: UnknownFunctionCoverageTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="products",
        repo_files={
            "sqlbuild_project.toml": test_case.project_toml,
            "functions/sql/starts_with.sql": _STARTS_WITH_FUNCTION,
            "models/products.sql": (
                f"MODEL (description 'Test model products.', database warehouse, schema analytics);\n\n{test_case.query_sql}"
            ),
        },
    )

    report: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("compile", "--json", "--no-cache")
    )
    text: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "compile", "--no-cache")
    )

    payload: dict[str, object] = json.loads(report.stdout)
    diagnostics: list[dict[str, object]] = cast(list[dict[str, object]], payload["diagnostics"])
    partial: dict[str, list[str]] = cast(
        dict[str, list[str]], payload.get("semantic_checks_partial", {})
    )
    assert report.returncode == test_case.expected_exit_code, report.stdout + report.stderr
    assert tuple(item["code"] for item in diagnostics) == test_case.expected_codes
    assert tuple(partial.get("products", ())) == test_case.expected_reasons
    assert test_case.expected_notice in text.stdout + text.stderr
