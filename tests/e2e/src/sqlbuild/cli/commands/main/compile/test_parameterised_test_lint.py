"""E2E coverage of SQL rules over parameterised SQL tests, which they lint like any test."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    ParameterisedTestLintTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT_TOML: str = (
    'name = "shop"\nadapter = "duckdb"\n\n[connection]\ndatabase = "shop.duckdb"\n\n'
    "[scopes]\nenforce_placement = false\n\n[rules]\nselect = [{rules}]\n"
)
_ORDERS_MODEL: str = (
    "MODEL (description \"Orders.\");\n\nSELECT 1 AS order_id, 'completed' AS status\n"
)
_STAGED_ORDERS_MODEL: str = (
    'MODEL (description "Staged orders.");\n\n'
    "SELECT o.order_id, CASE o.status WHEN 'completed' THEN 'completed' ELSE 'open' END AS status\n"
    'FROM __ref("orders") AS o\n'
)
_STATUS_TEST: str = (
    "TEST (\n"
    '  name "order_status_maps",\n'
    "  parameters (source_status string, expected_status string),\n"
    "  cases (\n"
    '    completed (source_status "completed", expected_status "completed"),\n'
    '    pending (source_status "pending", expected_status "open"),\n'
    "  ),\n"
    ");\n\n"
    "WITH\n"
    '__ref__orders AS (SELECT 1 AS order_id, @param("source_status") AS status),\n'
    '__expected__stg_orders AS (SELECT 1 AS order_id, @param("expected_status") AS status)\n'
)


@pytest.mark.parametrize(
    "test_case",
    [
        ParameterisedTestLintTestCase(
            description="compile lints the parameterised test instead of failing with L005",
            selected_rules='"SQLCLEAN004"',
            command=("--no-color", "compile"),
            expected_fragment="0 errors",
        ),
        ParameterisedTestLintTestCase(
            description="the cases still run once the test file is linted",
            selected_rules='"SQLCLEAN004"',
            command=("--no-color", "test"),
            expected_fragment="PASS=2  FAIL=0  TOTAL=2",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_parameterised_test_and_sql_rules_when_compiling_then_the_test_is_linted(
    test_case: ParameterisedTestLintTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML.format(rules=test_case.selected_rules),
            "models/orders.sql": _ORDERS_MODEL,
            "models/stg_orders.sql": _STAGED_ORDERS_MODEL,
            "tests/unit/test_stg_orders_status.sql": _STATUS_TEST,
        },
    )

    ran: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=test_case.command
    )
    report: str = ran.stdout + ran.stderr

    assert ran.returncode == 0, report
    assert "L005" not in report
    assert test_case.expected_fragment in report
