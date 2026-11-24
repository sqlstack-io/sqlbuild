"""E2E tests for compile-time rejection of truth-value tests the target warehouse lacks."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    TruthValueTestBuildTestCase,
    TruthValueTestCompileTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_SNOWFLAKE_PROJECT: str = 'name = "orders"\nadapter = "snowflake"\n'
_DUCKDB_PROJECT: str = (
    'name = "orders"\nadapter = "duckdb"\n\n[connection]\ndatabase = "warehouse.duckdb"\n'
)
_SOURCES: str = (
    "sources:\n"
    "  - name: raw__orders\n"
    "    description: Raw order records\n"
    "    database: example_db\n"
    "    schema: raw\n"
    "    table: orders\n"
    "    contract: enforced\n"
    "    columns:\n"
    "      - name: order_id\n        type: VARCHAR\n"
    "      - name: is_test_order\n        type: BOOLEAN\n"
)
_DUCKDB_SOURCES: str = _SOURCES.replace(
    "    database: example_db\n    schema: raw\n    table: orders\n",
    "    expression: SELECT 'o1' AS order_id, NULL::BOOLEAN AS is_test_order\n",
)
_MODEL_PATH: str = "sql/models/real_orders.sql"
_AUDIT_PATH: str = "sql/audits/generic/test_orders_are_flagged.sql"
_TEST_PATH: str = "tests/unit/test_real_orders.sql"
_MODEL: str = (
    "MODEL (\n"
    '  description "Orders that are not test orders",\n'
    "  materialized table,\n"
    "  database example_db,\n"
    "  schema analytics,\n"
    "  audits [test_orders_are_flagged],\n"
    ");\n\n"
    "WITH raw__orders AS (\n"
    "  SELECT *\n"
    '  FROM __source("raw__orders")\n'
    ")\n\n"
    "SELECT\n"
    "  order_id,\n"
    "  is_test_order\n"
    "FROM raw__orders\n"
    "WHERE is_test_order IS NOT TRUE\n"
)
_AUDIT: str = "AUDIT ();\n\nSELECT order_id\nFROM @relation\nWHERE is_test_order IS TRUE\n"
_DUCKDB_MODEL: str = _MODEL.replace("  database example_db,\n", "")
_SQL_TEST: str = (
    "TEST ();\n\n"
    "WITH __source__raw__orders AS (\n"
    "  SELECT 'o1' AS order_id, CAST(NULL AS BOOLEAN) AS is_test_order\n"
    "),\n\n"
    "__expected__real_orders AS (\n"
    "  SELECT 'o1' AS order_id, CAST(NULL AS BOOLEAN) AS is_test_order\n"
    "  WHERE (1 = 1) IS NOT FALSE\n"
    ")\n\n"
    "SELECT 1\n"
)
_SNOWFLAKE_HELP: str = (
    "rewrite the truth-value test null-safely, for example COALESCE(x, FALSE) for x IS TRUE, "
    "and NOT COALESCE(x, FALSE) or x IS DISTINCT FROM TRUE for x IS NOT TRUE"
)


@pytest.mark.parametrize(
    "test_case",
    (
        TruthValueTestCompileTestCase(
            description="snowflake rejects truth-value tests in models, audits and SQL tests",
            repo_files={
                "sqlbuild_project.toml": _SNOWFLAKE_PROJECT,
                "sql/sources/raw.yml": _SOURCES,
                _MODEL_PATH: _MODEL,
                _AUDIT_PATH: _AUDIT,
                _TEST_PATH: _SQL_TEST,
            },
            expected_exit_code=1,
            expected_diagnostics=(
                (
                    "B103",
                    "'IS NOT TRUE' is not supported for dialect Snowflake; "
                    "write NOT COALESCE(<expr>, FALSE) or <expr> IS DISTINCT FROM TRUE",
                    _MODEL_PATH,
                    18,
                    21,
                ),
                (
                    "B103",
                    "audit 'test_orders_are_flagged' on model 'real_orders': 'IS TRUE' is not "
                    "supported for dialect Snowflake; write COALESCE(<expr>, FALSE)",
                    _AUDIT_PATH,
                    5,
                    21,
                ),
                (
                    "B103",
                    "SQL test 'test_real_orders': 'IS NOT FALSE' is not supported for dialect "
                    "Snowflake; write COALESCE(<expr>, TRUE) or <expr> IS DISTINCT FROM FALSE",
                    _TEST_PATH,
                    9,
                    17,
                ),
            ),
            expected_help=_SNOWFLAKE_HELP,
        ),
        TruthValueTestCompileTestCase(
            description="duckdb accepts truth-value tests",
            repo_files={
                "sqlbuild_project.toml": _DUCKDB_PROJECT,
                "sql/sources/raw.yml": _DUCKDB_SOURCES,
                _MODEL_PATH: _DUCKDB_MODEL,
                _AUDIT_PATH: _AUDIT,
                _TEST_PATH: _SQL_TEST,
            },
            expected_exit_code=0,
            expected_diagnostics=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_truth_value_test_when_compiling_then_dialect_support_is_enforced(
    test_case: TruthValueTestCompileTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="orders", repo_files=test_case.repo_files
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "compile", "--json", "--no-cache")
    )

    payload: dict[str, Any] = json.loads(result.stdout)
    diagnostics: list[dict[str, Any]] = payload["diagnostics"]
    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert (
        tuple(
            (item["code"], item["message"], item["path"], item["line"], item["column"])
            for item in diagnostics
        )
        == test_case.expected_diagnostics
    )
    assert all(item["help"] == test_case.expected_help for item in diagnostics)


@pytest.mark.parametrize(
    "test_case",
    (
        TruthValueTestBuildTestCase(
            description="duckdb builds, audits and tests truth-value tests",
            repo_files={
                "sqlbuild_project.toml": _DUCKDB_PROJECT,
                "sql/sources/raw.yml": _DUCKDB_SOURCES,
                _MODEL_PATH: _DUCKDB_MODEL,
                _AUDIT_PATH: _AUDIT,
                _TEST_PATH: _SQL_TEST,
            },
            expected_exit_code=0,
            expected_stdout_fragment="FAIL=0",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_duckdb_truth_value_tests_when_building_then_warehouse_accepts_them(
    test_case: TruthValueTestBuildTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="orders", repo_files=test_case.repo_files
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "build")
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert test_case.expected_stdout_fragment in result.stdout, result.stdout
