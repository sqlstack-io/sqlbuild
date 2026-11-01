"""E2E coverage of compile-time SQL checks on audits, SQL tests and SQL hooks."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    ResourceSqlHelpCase,
    ResourceSqlOptOutCase,
    ResourceSqlValidationCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    RESOURCE_SQL_GENERIC_AUDIT,
    RESOURCE_SQL_MODELS,
    RESOURCE_SQL_NAMED_HOOK,
    RESOURCE_SQL_ORDERS,
    RESOURCE_SQL_RECENT_ROWS_AUDIT,
    RESOURCE_SQL_SINGULAR_AUDIT,
    RESOURCE_SQL_TEST,
    resource_sql_orders_model,
    resource_sql_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_REQUIRED_ANALYSIS_TOML: tuple[str, str] = (
    "sqlbuild_project.toml",
    'name = "orders"\nadapter = "duckdb"\n\n[connection]\ndatabase = "warehouse.duckdb"\n\n'
    "[settings]\nrequire_sql_analysis = true\n",
)
_FINDING_AUDIT_BODY: str = (
    'SELECT o.order_id\nFROM __ref("orders") AS o\n'
    'JOIN __ref("customers") AS c ON o.order_id = c.order_id\n'
    "WHERE STARTSWITH(o.status, 'x')\n"
)
_FINDING_TEST_BODY: str = (
    "WITH __ref__customers AS (SELECT 1 AS order_id, 7 AS customer_id),\n"
    "__expected__orders AS (SELECT 1 AS order_id, UPPERCASE('open') AS status)\n"
    "SELECT 1\n"
)


@pytest.mark.parametrize(
    "test_case",
    (
        ResourceSqlValidationCase(
            description="singular audit unknown column and function",
            files=(
                resource_sql_orders_model(),
                (
                    RESOURCE_SQL_SINGULAR_AUDIT,
                    'AUDIT ();\nSELECT o.order_id\nFROM __ref("orders") AS o\n'
                    'JOIN __ref("customers") AS c ON o.order_id = c.order_id\n'
                    "WHERE o.shipped_at IS NULL AND STARTSWITH(o.status, 'x')\n",
                ),
            ),
            expected_diagnostics=(
                (
                    "B002",
                    "audit 'orders_have_customers': "
                    "Unknown column 'shipped_at' in table 'orders' (context: WHERE)",
                    RESOURCE_SQL_SINGULAR_AUDIT,
                    5,
                ),
                (
                    "B101",
                    "audit 'orders_have_customers': "
                    "Unknown function 'STARTSWITH' for dialect DuckDB; did you mean STARTS_WITH?",
                    RESOURCE_SQL_SINGULAR_AUDIT,
                    5,
                ),
            ),
        ),
        ResourceSqlValidationCase(
            description="generic audit instance unknown column",
            files=(
                resource_sql_orders_model(", audits [status_is_known]"),
                (
                    RESOURCE_SQL_GENERIC_AUDIT,
                    "AUDIT ();\nSELECT * FROM @relation AS r\nWHERE r.status_code = 'unknown'\n",
                ),
            ),
            expected_diagnostics=(
                (
                    "B002",
                    "audit 'status_is_known' on model 'orders': "
                    "Unknown column 'status_code' in table 'orders' (context: WHERE)",
                    RESOURCE_SQL_GENERIC_AUDIT,
                    3,
                ),
            ),
        ),
        ResourceSqlValidationCase(
            description="SQL test fixture and expected CTE unknown functions",
            files=(
                resource_sql_orders_model(),
                (
                    RESOURCE_SQL_TEST,
                    "TEST ();\nWITH __ref__customers AS (\n"
                    "  SELECT 1 AS order_id, STRING_TO_INT('7') AS customer_id\n),\n"
                    "__expected__orders AS (SELECT 1 AS order_id, UPPERCASE('open') AS status)\n"
                    "SELECT 1\n",
                ),
            ),
            expected_diagnostics=(
                (
                    "B101",
                    "SQL test 'test_orders': Unknown function 'STRING_TO_INT' for dialect DuckDB",
                    RESOURCE_SQL_TEST,
                    3,
                ),
                (
                    "B101",
                    "SQL test 'test_orders': Unknown function 'UPPERCASE' for dialect DuckDB",
                    RESOURCE_SQL_TEST,
                    5,
                ),
            ),
        ),
        ResourceSqlValidationCase(
            description="SQL test assertion unknown model column",
            files=(
                resource_sql_orders_model(),
                (
                    RESOURCE_SQL_TEST,
                    "TEST ();\nWITH __ref__customers AS (SELECT 1 AS order_id, 7 AS customer_id),\n"
                    '__assert__orders_are_open AS (\n  SELECT * FROM __ref("orders") AS o\n'
                    "  WHERE o.order_status <> 'open'\n)\nSELECT 1\n",
                ),
            ),
            expected_diagnostics=(
                (
                    "B002",
                    "SQL test 'test_orders': "
                    "Unknown column 'order_status' in table 'orders' (context: WHERE)",
                    RESOURCE_SQL_TEST,
                    5,
                ),
            ),
        ),
        ResourceSqlValidationCase(
            description="inline and named SQL hook unknown functions",
            files=(
                resource_sql_orders_model(
                    ', post_hooks [inline_sql("SELECT STRING_TO_INT(\'1\')"), sql("record_orders")]'
                ),
                (
                    RESOURCE_SQL_NAMED_HOOK,
                    'HOOK (description "Count orders");\n'
                    'SELECT COUNT_ROWS(*) FROM __ref("customers")\n',
                ),
            ),
            expected_diagnostics=(
                (
                    "B101",
                    "post_hooks[0] SQL hook: Unknown function 'STRING_TO_INT' for dialect DuckDB",
                    RESOURCE_SQL_ORDERS,
                    1,
                ),
                (
                    "B101",
                    "post_hooks[1] SQL hook: Unknown function 'COUNT_ROWS' for dialect DuckDB",
                    RESOURCE_SQL_NAMED_HOOK,
                    2,
                ),
            ),
        ),
        ResourceSqlValidationCase(
            description="unparseable singular audit is a syntax error",
            files=(
                resource_sql_orders_model(),
                (
                    RESOURCE_SQL_SINGULAR_AUDIT,
                    'AUDIT ();\nSELECT o.order_id\nFROM __ref("orders") AS o\n'
                    'JOIN __ref("customers") AS c ON o.order_id = c.order_id\n'
                    "WHERE o.status ===== 'open'\n",
                ),
            ),
            expected_diagnostics=(
                (
                    "P001",
                    "SQL syntax error in audit 'orders_have_customers': Unexpected token: Eq",
                    RESOURCE_SQL_SINGULAR_AUDIT,
                    5,
                ),
            ),
        ),
        ResourceSqlValidationCase(
            description="built-in column audit on a missing column reports once at the header",
            files=(resource_sql_orders_model(", columns (shipped_at (audits [not_null]))"),),
            expected_diagnostics=(
                (
                    "B002",
                    "audit 'not_null' on model 'orders' column 'shipped_at': "
                    "Unknown column 'shipped_at' in table 'orders'",
                    RESOURCE_SQL_ORDERS,
                    1,
                ),
                (
                    "K001",
                    "declared column 'shipped_at' was not found in statically inferred output "
                    "for model 'orders'",
                    RESOURCE_SQL_ORDERS,
                    1,
                ),
            ),
        ),
        ResourceSqlValidationCase(
            description="explicit column contracts report each built-in audit once at the header",
            files=(
                (
                    "sqlbuild_project.toml",
                    'name = "orders"\nadapter = "duckdb"\n\n[connection]\n'
                    'database = "warehouse.duckdb"\n\n'
                    '[settings]\ncolumn_contract_mode = "explicit"\n',
                ),
                resource_sql_orders_model(", columns (shipped_at (audits [not_null, unique]))"),
            ),
            expected_diagnostics=(
                (
                    "B002",
                    "audit 'not_null' on model 'orders' column 'shipped_at': "
                    "Unknown column 'shipped_at' in table 'orders'",
                    RESOURCE_SQL_ORDERS,
                    1,
                ),
                (
                    "B002",
                    "audit 'unique' on model 'orders' column 'shipped_at': "
                    "Unknown column 'shipped_at' in table 'orders'",
                    RESOURCE_SQL_ORDERS,
                    1,
                ),
            ),
        ),
        ResourceSqlValidationCase(
            description="unparseable SQL test fixture is a syntax error",
            files=(
                resource_sql_orders_model(),
                (
                    RESOURCE_SQL_TEST,
                    "TEST ();\nWITH __ref__customers AS (\n"
                    "  SELECT 1 AS order_id, 7 AS customer_id FROM WHERE\n),\n"
                    "__expected__orders AS (SELECT 1 AS order_id)\nSELECT 1\n",
                ),
            ),
            expected_diagnostics=(
                (
                    "P001",
                    "SQL syntax error in SQL test 'test_orders': "
                    "Expected table name or subquery, got Where",
                    RESOURCE_SQL_TEST,
                    3,
                ),
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_resource_sql_error_when_compiling_and_building_then_command_fails_at_the_resource(
    test_case: ResourceSqlValidationCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files=resource_sql_project_files(test_case.files),
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "compile", "--json", "--no-cache")
    )
    built: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "build")
    )

    payload: dict[str, Any] = json.loads(compiled.stdout)
    assert compiled.returncode == 1, compiled.stdout + compiled.stderr
    assert (
        tuple(
            (item["code"], item["message"], item["path"], item["line"])
            for item in payload["diagnostics"]
        )
        == test_case.expected_diagnostics
    )
    assert built.returncode == 1, built.stdout + built.stderr
    assert not (project_dir / "warehouse.duckdb").exists()


@pytest.mark.parametrize(
    "test_case",
    (
        ResourceSqlValidationCase(
            description="valid audits, SQL tests and hooks",
            files=(
                resource_sql_orders_model(
                    ", audits [status_is_known], "
                    "post_hooks [inline_sql(\"SELECT STARTS_WITH('open', 'o')\")]"
                ),
                (
                    RESOURCE_SQL_GENERIC_AUDIT,
                    "AUDIT ();\nSELECT * FROM @relation AS r\nWHERE r.status <> 'open'\n",
                ),
                (
                    RESOURCE_SQL_SINGULAR_AUDIT,
                    'AUDIT ();\nSELECT o.order_id\nFROM __ref("orders") AS o\n'
                    'LEFT JOIN __ref("customers") AS c ON o.order_id = c.order_id\n'
                    "WHERE c.customer_id IS NULL\n",
                ),
                (
                    RESOURCE_SQL_TEST,
                    "TEST ();\nWITH __ref__customers AS (SELECT 1 AS order_id, 7 AS customer_id),\n"
                    "__expected__orders AS (SELECT 1 AS order_id, LOWER('OPEN') AS status)\n"
                    "SELECT 1\n",
                ),
            ),
            expected_diagnostics=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_valid_audits_tests_and_hooks_when_building_then_checks_pass(
    test_case: ResourceSqlValidationCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files=resource_sql_project_files(test_case.files),
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "compile", "--json", "--no-cache")
    )
    built: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "build")
    )

    assert compiled.returncode == 0, compiled.stdout + compiled.stderr
    assert tuple(json.loads(compiled.stdout)["diagnostics"]) == test_case.expected_diagnostics
    assert built.returncode == 0, built.stdout + built.stderr


@pytest.mark.parametrize(
    "test_case",
    (
        ResourceSqlHelpCase(
            description="generic audit on a model compares a text argument with a date",
            files=(
                (
                    RESOURCE_SQL_ORDERS,
                    'MODEL (description "Orders", audits [recent_rows (date_column "ordered_on")]);\n'
                    "SELECT CAST(c.order_id AS INTEGER) AS order_id,"
                    " CAST('2026-04-01' AS VARCHAR) AS ordered_on\n"
                    'FROM __ref("customers") AS c\n',
                ),
                (
                    RESOURCE_SQL_RECENT_ROWS_AUDIT,
                    "AUDIT ();\nSELECT 1 AS missing_rows FROM @relation AS r\n"
                    "WHERE @date_column = CURRENT_DATE\n",
                ),
            ),
            expected_diagnostics=(
                (
                    "B217",
                    "audit 'recent_rows' on model 'orders': "
                    "Incompatible comparison between STRING and DATE",
                    "Convert the text value explicitly, for example `CAST(@date_column AS DATE)`, "
                    "or attach audit 'recent_rows' to a DATE column",
                ),
            ),
        ),
        ResourceSqlHelpCase(
            description="SQL test reads a column the model no longer outputs",
            files=(
                resource_sql_orders_model(),
                (
                    RESOURCE_SQL_TEST,
                    "TEST ();\nWITH __ref__customers AS (SELECT 1 AS order_id, 7 AS customer_id),\n"
                    '__assert__orders_are_open AS (\n  SELECT * FROM __ref("orders") AS o\n'
                    "  WHERE o.order_status <> 'open'\n)\nSELECT 1\n",
                ),
            ),
            expected_diagnostics=(
                (
                    "B002",
                    "SQL test 'test_orders': "
                    "Unknown column 'order_status' in table 'orders' (context: WHERE)",
                    "'orders' does not output 'order_status' (it outputs order_id, status); "
                    "update the test to its current columns",
                ),
            ),
        ),
        ResourceSqlHelpCase(
            description="SQL test assertion compares text values with model columns",
            files=(
                ("sqlbuild_project.toml", 'name = "orders"\nadapter = "snowflake"\n'),
                (
                    f"{RESOURCE_SQL_MODELS}/customers.sql",
                    'MODEL (description "Customers", database example, schema analytics);\n'
                    "SELECT CAST(1 AS INTEGER) AS order_id\n",
                ),
                (
                    RESOURCE_SQL_ORDERS,
                    'MODEL (description "Orders", database example, schema analytics);\n'
                    "SELECT CAST(c.order_id AS INTEGER) AS order_id,"
                    " CAST('2026-04-01' AS TIMESTAMP) AS ordered_at\n"
                    'FROM __ref("customers") AS c\n',
                ),
                (
                    RESOURCE_SQL_TEST,
                    "TEST ();\nWITH __ref__customers AS (SELECT 1 AS order_id),\n"
                    "__assert__orders_match AS (\n"
                    '  SELECT o.order_id, o.ordered_at FROM __ref("orders") AS o\n'
                    "  EXCEPT\n"
                    "  SELECT CAST('1' AS VARCHAR) AS order_id, '2026-04-01' AS ordered_at\n"
                    ")\nSELECT 1\n",
                ),
            ),
            expected_diagnostics=(
                (
                    "B215",
                    "SQL test 'test_orders': Set-operation column 1 may fail during runtime "
                    "conversion: accumulated type NUMBER, next type VARCHAR",
                    "Cast set-operation column 1 to the tested model's type INT in the "
                    "expected or fixture rows, for example `CAST('1' AS INT)`",
                ),
                (
                    "B215",
                    "SQL test 'test_orders': Set-operation column 2 may fail during runtime "
                    "conversion: accumulated type TIMESTAMP_NTZ, next type VARCHAR",
                    "Cast set-operation column 2 to the tested model's type TIMESTAMP_NTZ in the "
                    "expected or fixture rows, for example `CAST('2026-04-01' AS TIMESTAMP_NTZ)`",
                ),
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_typed_resource_mismatch_when_compiling_then_message_names_owner_and_shows_cast(
    test_case: ResourceSqlHelpCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files=resource_sql_project_files(test_case.files),
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "compile", "--json", "--no-cache")
    )

    payload: dict[str, Any] = json.loads(compiled.stdout)
    assert compiled.returncode == 1, compiled.stdout + compiled.stderr
    assert (
        tuple((item["code"], item["message"], item.get("help")) for item in payload["diagnostics"])
        == test_case.expected_diagnostics
    )


@pytest.mark.parametrize(
    "test_case",
    (
        ResourceSqlOptOutCase(
            description="--no-sql-analysis skips audit and SQL test checks",
            files=(
                resource_sql_orders_model(),
                (RESOURCE_SQL_SINGULAR_AUDIT, "AUDIT ();\n" + _FINDING_AUDIT_BODY),
                (RESOURCE_SQL_TEST, "TEST ();\n" + _FINDING_TEST_BODY),
            ),
            flags=("--no-sql-analysis",),
            expected_diagnostics=(),
        ),
        ResourceSqlOptOutCase(
            description="AUDIT and TEST header opt-outs skip their checks",
            files=(
                resource_sql_orders_model(),
                (
                    RESOURCE_SQL_SINGULAR_AUDIT,
                    "AUDIT (sql_analysis false);\n" + _FINDING_AUDIT_BODY,
                ),
                (RESOURCE_SQL_TEST, "TEST (sql_analysis false);\n" + _FINDING_TEST_BODY),
            ),
            flags=(),
            expected_diagnostics=(),
        ),
        ResourceSqlOptOutCase(
            description="SQL tests of a model with sql_analysis false are not analysed",
            files=(
                resource_sql_orders_model(", sql_analysis false"),
                (RESOURCE_SQL_TEST, "TEST ();\n" + _FINDING_TEST_BODY),
            ),
            flags=(),
            expected_diagnostics=(),
        ),
        ResourceSqlOptOutCase(
            description="required analysis rejects opt-outs on parseable audits and tests",
            files=(
                _REQUIRED_ANALYSIS_TOML,
                resource_sql_orders_model(),
                (
                    RESOURCE_SQL_SINGULAR_AUDIT,
                    "AUDIT (sql_analysis false);\n" + _FINDING_AUDIT_BODY,
                ),
                (RESOURCE_SQL_TEST, "TEST (sql_analysis false);\n" + _FINDING_TEST_BODY),
            ),
            flags=(),
            expected_diagnostics=(
                ("P009", RESOURCE_SQL_SINGULAR_AUDIT, 1),
                ("P009", RESOURCE_SQL_TEST, 1),
            ),
            expected_returncode=1,
        ),
        ResourceSqlOptOutCase(
            description="required analysis accepts an opt-out on an unparseable audit",
            files=(
                _REQUIRED_ANALYSIS_TOML,
                resource_sql_orders_model(),
                (
                    RESOURCE_SQL_SINGULAR_AUDIT,
                    "AUDIT (sql_analysis false);\n"
                    + _FINDING_AUDIT_BODY.replace("STARTSWITH(o.status, 'x')", "o.status ===== 1"),
                ),
            ),
            flags=(),
            expected_diagnostics=(),
        ),
        ResourceSqlOptOutCase(
            description="required analysis accepts a model opt-out for an unparseable hook",
            files=(
                _REQUIRED_ANALYSIS_TOML,
                resource_sql_orders_model(
                    ', sql_analysis false, post_hooks [inline_sql("CHECKPOINT")]'
                ),
            ),
            flags=(),
            expected_diagnostics=(),
        ),
        ResourceSqlOptOutCase(
            description="required analysis rejects a model opt-out when its hooks parse",
            files=(
                _REQUIRED_ANALYSIS_TOML,
                resource_sql_orders_model(
                    ', sql_analysis false, post_hooks [inline_sql("SELECT 1")]'
                ),
            ),
            flags=(),
            expected_diagnostics=(("P009", RESOURCE_SQL_ORDERS, 1),),
            expected_returncode=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_sql_analysis_opt_outs_when_compiling_then_resource_checks_follow_them(
    test_case: ResourceSqlOptOutCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files=resource_sql_project_files(test_case.files),
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir,
        command=("--no-color", "compile", "--json", "--no-cache", *test_case.flags),
    )

    payload: dict[str, Any] = json.loads(compiled.stdout)
    assert (
        tuple((item["code"], item["path"], item["line"]) for item in payload["diagnostics"])
        == test_case.expected_diagnostics
    ), compiled.stdout + compiled.stderr
    assert compiled.returncode == test_case.expected_returncode


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
