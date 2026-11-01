"""SQL rule findings agree between compile's early lint and a standalone rule run."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.rules._test_types import SqlRulePathParityCase
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        SqlRulePathParityCase(
            description="macro, CRLF model and CRLF SQL test",
            files={
                "sqlbuild_project.toml": (
                    'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQLCLEAN"]\n'
                ),
                "models/staging/orders.sql": (
                    'MODEL (description "Orders");\n'
                    "WITH unused_orders AS (SELECT 1 AS order_id), "
                    "kept AS (SELECT 2 AS order_id)\n"
                    'SELECT kept.order_id FROM kept WHERE @py.order_filter("kept.order_id")\n'
                ),
                "models/staging/customers.sql": (
                    'MODEL (description "Customers");\r\n'
                    "WITH spare AS (SELECT 1 AS customer_id)\r\n"
                    "SELECT 1 AS customer_id\r\n"
                ),
                "models/staging/_sqlbuild/_macros/python/order_filter.py": (
                    'def order_filter(column: str) -> str:\n    return f"{column} > 0"\n'
                ),
                "tests/unit/test_orders.sql": (
                    "TEST ();\r\nWITH\r\n__ref__orders AS (SELECT 2 AS order_id),\r\n"
                    "unused_rows AS (SELECT 3 AS x),\r\n"
                    "__expected__orders AS (SELECT order_id FROM __ref__orders LIMIT 1)\r\n"
                    "SELECT 1\r\n"
                ),
            },
            expected_findings=(
                ("models/staging/customers.sql", 2, 6, "SQLCLEAN005"),
                ("models/staging/customers.sql", 2, 6, "SQLCLEAN041"),
                ("models/staging/customers.sql", 3, 1, "SQLCLEAN035"),
                ("models/staging/orders.sql", 2, 6, "SQLCLEAN005"),
                ("models/staging/orders.sql", 2, 47, "SQLCLEAN041"),
                ("models/staging/orders.sql", 3, 1, "SQLCLEAN034"),
                ("models/staging/orders.sql", 3, 1, "SQLCLEAN035"),
                ("tests/unit/test_orders.sql", 4, 1, "SQLCLEAN005"),
                ("tests/unit/test_orders.sql", 5, 59, "SQLCLEAN004"),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sql_findings_when_compiling_or_running_rules_then_findings_are_identical(
    test_case: SqlRulePathParityCase,
    tmp_path: Path,
) -> None:
    compile_dir: Path = prepare_inline_project(
        tmp_path=tmp_path / "compile", project_name="orders", repo_files=test_case.files
    )
    rules_dir: Path = prepare_inline_project(
        tmp_path=tmp_path / "rules", project_name="orders", repo_files=test_case.files
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile", "--json", "--no-cache"), project_dir=compile_dir
    )
    ruled: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "rules", "--json", "run", "SQLCLEAN"), project_dir=rules_dir
    )

    compile_payload: dict[str, Any] = json.loads(compiled.stdout)
    rules_payload: dict[str, Any] = json.loads(ruled.stdout)
    compile_findings: list[tuple[str, int, int, str, str]] = [
        (item["path"], item["line"], item["column"], item["code"], item["message"])
        for item in compile_payload["diagnostics"]
    ]
    rules_findings: list[tuple[str, int, int, str, str]] = [
        (item["path"], item["line"], item["column"], item["code"], item["message"])
        for item in rules_payload["findings"]
    ]
    assert compiled.returncode == ruled.returncode == test_case.expected_exit
    assert compile_findings == rules_findings
    assert tuple(finding[:4] for finding in compile_findings) == test_case.expected_findings


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-n", "auto", "--dist", "loadfile"]))
