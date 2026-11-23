"""E2E coverage of SQL-test mocks for references read through a parenthesised join."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import (
    ParenthesisedJoinMockTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT_TOML: str = (
    'name = "shop"\nadapter = "duckdb"\n\n[connection]\ndatabase = "shop.duckdb"\n\n'
    "[scopes]\nenforce_placement = false\n"
)
_TEST: str = (
    "TEST ();\n\n"
    "WITH\n"
    "__ref__orders AS (SELECT 7 AS order_id),\n"
    "__ref__customers AS (SELECT 7 AS order_id),\n"
    "__expected__order_customers AS (SELECT 7 AS order_id)\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        ParenthesisedJoinMockTestCase(
            description="both sides of a parenthesised join use their mocks",
            model_from=(
                '(__ref("orders") AS o JOIN __ref("customers") AS c ON o.order_id = c.order_id)'
            ),
            expected_exit_code=0,
            expected_fragment="PASS=1  FAIL=0",
        ),
        ParenthesisedJoinMockTestCase(
            description="an unparenthesised join uses its mocks",
            model_from='__ref("orders") AS o JOIN __ref("customers") AS c ON o.order_id = c.order_id',
            expected_exit_code=0,
            expected_fragment="PASS=1  FAIL=0",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_parenthesised_join_when_testing_then_both_references_use_their_mocks(
    test_case: ParenthesisedJoinMockTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML,
            "sql/models/orders.sql": 'MODEL (description "Orders.");\n\nSELECT 1 AS order_id\n',
            "sql/models/customers.sql": 'MODEL (description "Customers.");\n\nSELECT 1 AS order_id\n',
            "sql/models/order_customers.sql": (
                'MODEL (description "Orders with customers.");\n\n'
                f"SELECT o.order_id FROM {test_case.model_from}\n"
            ),
            "tests/unit/test_order_customers.sql": _TEST,
        },
    )

    tested: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"), project_dir=project_dir
    )

    assert tested.returncode == test_case.expected_exit_code, tested.stdout + tested.stderr
    assert test_case.expected_fragment in tested.stdout
