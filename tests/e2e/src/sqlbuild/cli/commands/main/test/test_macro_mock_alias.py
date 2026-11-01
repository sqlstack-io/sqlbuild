"""E2E coverage of macro mocks that name their value, as SQLCLEAN022 asks."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import MacroMockAliasTestCase
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT_TOML: str = (
    'name = "shop"\nadapter = "duckdb"\n\n[connection]\ndatabase = "shop.duckdb"\n\n'
    '[scopes]\nenforce_placement = false\n\n[rules]\nselect = ["SQLCLEAN022"]\n'
)
_REGION_MACRO: str = (
    "MACRO (\n  description \"The sales region filter\",\n  returns BOOLEAN,\n);\n\nregion = 'NA'\n"
)
_ORDERS_MODEL: str = (
    'MODEL (description "Orders.");\n\n'
    "SELECT 1 AS order_id, 'EU' AS region UNION ALL SELECT 2 AS order_id, 'NA' AS region\n"
)
_REGIONAL_ORDERS_MODEL: str = (
    'MODEL (description "Orders of one region.");\n\n'
    'SELECT o.order_id FROM __ref("orders") AS o WHERE @sql.region_filter()\n'
)
_TEST: str = (
    "TEST ();\n\n"
    "WITH\n"
    "__macro__region_filter AS ({mock_select}),\n"
    "__ref__orders AS (SELECT 1 AS order_id, 'EU' AS region),\n"
    "__expected__regional_orders AS (SELECT 1 AS order_id)\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        MacroMockAliasTestCase(
            description="an aliased mock satisfies SQLCLEAN022",
            mock_select="SELECT 'region = ''EU''' AS region_filter",
            command=("--no-color", "compile"),
            expected_exit_code=0,
            expected_fragment="0 errors",
        ),
        MacroMockAliasTestCase(
            description="an aliased mock still replaces the macro",
            mock_select="SELECT 'region = ''EU''' AS region_filter",
            command=("--no-color", "test"),
            expected_exit_code=0,
            expected_fragment="PASS=1  FAIL=0",
        ),
        MacroMockAliasTestCase(
            description="an unaliased mock is reported by SQLCLEAN022",
            mock_select="SELECT 'region = ''EU'''",
            command=("--no-color", "compile"),
            expected_exit_code=1,
            expected_fragment="SQLCLEAN022",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_macro_mock_with_alias_when_testing_then_the_mock_value_is_used(
    test_case: MacroMockAliasTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML,
            "macros/sql/region_filter.sql": _REGION_MACRO,
            "models/orders.sql": _ORDERS_MODEL,
            "models/regional_orders.sql": _REGIONAL_ORDERS_MODEL,
            "tests/unit/test_regional_orders.sql": _TEST.format(mock_select=test_case.mock_select),
        },
    )

    ran: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=test_case.command
    )
    report: str = ran.stdout + ran.stderr

    assert ran.returncode == test_case.expected_exit_code, report
    assert test_case.expected_fragment in report
