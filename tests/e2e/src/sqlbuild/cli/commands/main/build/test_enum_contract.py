"""E2E tests for enum-backed contract enforcement."""

from __future__ import annotations

import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    EnumContractBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        EnumContractBuildE2ETestCase(
            description="enum value outside declared members blocks promotion",
            selected_member="UNKNOWN",
            expected_exit_code=1,
            expected_stdout_fragment="failed before replacing target table",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_enum_typed_contract_when_value_is_invalid_then_build_fails_audit(
    test_case: EnumContractBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="enum_contract_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "enum_contract_project"
                adapter = "duckdb"

                [connection]
                database = "enum_contract.duckdb"
                """
            ).strip()
            + "\n",
            "sql/models/_enums/fulfillment_method.sql": "ENUM (name fulfillment_method, members [DELIVERY, PICKUP]);\n",
            "sql/models/orders.sql": dedent(
                f"""
                MODEL (description "Test model orders.",
                  materialized table,
                  contract enforced,
                  columns (
                    fulfillment_method (type fulfillment_method),
                  ),
                );

                SELECT '{test_case.selected_member}' AS fulfillment_method
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--select", "orders"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code
    assert test_case.expected_stdout_fragment in result.stdout


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
