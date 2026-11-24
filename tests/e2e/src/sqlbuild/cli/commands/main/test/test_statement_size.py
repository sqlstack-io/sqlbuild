"""Large SQL tests execute through the real DuckDB CLI without an arbitrary cap."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import SqlTestE2ETestCase
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    [SqlTestE2ETestCase("large combined SQL executes", 0, "PASS=1")],
    ids=lambda case: case.description,
)
def test_given_combined_sql_over_256k_when_testing_on_duckdb_then_passes(
    tmp_path: Path, test_case: SqlTestE2ETestCase
) -> None:
    # A literal survives compiler formatting, unlike padding with whitespace/comments.
    payload: str = "orders" * 45_000
    assert len(payload) > 256_000
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="large_sql_test",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "large_sql_test"\nadapter = "duckdb"\n\n'
                '[connection]\ndatabase = "orders.duckdb"\n'
            ),
            "sql/models/raw_orders.sql": "MODEL (description 'Test model raw_orders.');\nSELECT 'orders' AS details\n",
            "sql/models/orders.sql": 'MODEL (description "Test model orders.");\nSELECT details FROM __ref("raw_orders")\n',
            "tests/unit/test_orders.sql": (
                'TEST (name "large_orders");\nWITH __ref__raw_orders AS (\n'
                f"SELECT '{payload}' AS details\n), __expected__orders AS (\n"
                f"SELECT '{payload}' AS details\n) SELECT 1\n"
            ),
        },
    )

    tested: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"), project_dir=project_dir
    )

    output: str = tested.stdout + tested.stderr
    assert tested.returncode == test_case.expected_exit_code, output
    assert test_case.expected_stdout_fragment in output
    assert "FAIL=0" in output
    assert "T001" not in output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
