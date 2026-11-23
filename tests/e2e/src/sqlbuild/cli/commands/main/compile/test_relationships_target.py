"""E2E coverage for the relation a relationships audit reads."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    RelationshipsTargetCompileTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        RelationshipsTargetCompileTestCase(
            description="quoted relation name is rejected",
            target='"customers"',
            expected_exit_code=1,
            expected_output_fragment='write to __ref("customers")',
        ),
        RelationshipsTargetCompileTestCase(
            description="reference call compiles",
            target='__ref("customers")',
            expected_exit_code=0,
            expected_output_fragment="Project compiled",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_relationships_audit_when_compiling_then_target_must_be_a_reference(
    test_case: RelationshipsTargetCompileTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="relationships_target",
        repo_files={
            "sqlbuild_project.toml": 'name = "relationships_target"\nadapter = "duckdb"\n',
            "sql/models/customers.sql": (
                "MODEL (description 'Test model customers.');\nSELECT 1 AS id\n"
            ),
            "sql/models/orders.sql": (
                "MODEL (description 'Test model orders.', columns (customer_id (\n"
                f"  audits [relationships (to {test_case.target}, field id)])));\n"
                "SELECT 1 AS order_id, 1 AS customer_id\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert test_case.expected_output_fragment in result.stdout + result.stderr


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
