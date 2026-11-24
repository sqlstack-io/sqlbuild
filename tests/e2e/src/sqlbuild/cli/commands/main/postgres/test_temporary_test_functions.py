"""E2E coverage of the PostgreSQL option that turns off temporary function copies in `sqb test`."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.postgres._test_types import (
    PostgresTemporaryTestFunctionsE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.postgres.helpers import (
    build_postgres_project_toml,
    build_unique_schema_name,
    cleanup_postgres_schema,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_FUNCTION_PATH: str = "sql/functions/sql/is_completed_order.sql"
_FUNCTION_HEADER: str = (
    "FUNCTION (description 'Whether an order status counts as completed.', "
    "arguments (order_status VARCHAR), returns BOOLEAN);\n"
)
_TEST: str = (
    'TEST (name "orders_case");\nWITH\n'
    "__ref__order_statuses AS (SELECT 'completed' AS order_status),\n"
    "__expected__orders AS (SELECT 'completed' AS order_status, TRUE AS is_completed)\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        PostgresTemporaryTestFunctionsE2ETestCase(
            description="by default the test runs the edited definition",
            connection_option="",
            expected_exit_code=1,
            expected_stdout_fragment="FAIL=1",
        ),
        PostgresTemporaryTestFunctionsE2ETestCase(
            description="with copies off the test runs the built function",
            connection_option="temporary_test_functions = false\n",
            expected_exit_code=0,
            expected_stdout_fragment="PASS=1",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_edited_function_when_testing_then_option_chooses_the_definition(
    test_case: PostgresTemporaryTestFunctionsE2ETestCase,
    tmp_path: Path,
    postgres_e2e_config: dict[str, object],
) -> None:
    schema_name: str = build_unique_schema_name(prefix="sqb_temp_functions")
    project_toml: str = build_postgres_project_toml(
        project_name="orders_temporary_functions",
        schema_name=schema_name,
        config=postgres_e2e_config,
    ).replace("[connection]\n", f"[connection]\n{test_case.connection_option}", 1)
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders_temporary_functions",
        repo_files={
            "sqlbuild_project.toml": project_toml,
            "sql/models/order_statuses.sql": (
                "MODEL (description 'Test model order_statuses.', materialized table);\n"
                "SELECT 'completed' AS order_status\n"
            ),
            "sql/models/orders.sql": (
                "MODEL (description 'Test model orders.', materialized table);\n"
                'SELECT order_status, __udf("is_completed_order")(order_status) AS is_completed '
                'FROM __ref("order_statuses")\n'
            ),
            "tests/unit/orders_case.sql": _TEST,
            _FUNCTION_PATH: _FUNCTION_HEADER + "SELECT order_status = 'completed'",
        },
    )
    try:
        built: subprocess.CompletedProcess[str] = run_sqb(
            command=("build", "--no-tests", "--no-audits"), project_dir=project_dir
        )
        _ = (project_dir / _FUNCTION_PATH).write_text(
            _FUNCTION_HEADER + "SELECT order_status = 'shipped'", encoding="utf-8"
        )

        tested: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "test"), project_dir=project_dir
        )
    finally:
        cleanup_postgres_schema(schema_name=schema_name, config=postgres_e2e_config)

    assert built.returncode == 0, built.stdout + built.stderr
    assert tested.returncode == test_case.expected_exit_code, tested.stdout + tested.stderr
    assert test_case.expected_stdout_fragment in tested.stdout, test_case.description
