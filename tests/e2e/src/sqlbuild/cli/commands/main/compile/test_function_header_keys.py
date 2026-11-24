"""E2E coverage for rejecting unknown function header keys at compile time."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    FunctionHeaderKeyCompileCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        FunctionHeaderKeyCompileCase(
            description="sql udf header with a removed replay setting",
            repo_files={
                "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
                "sql/functions/sql/normalize_status.sql": (
                    "FUNCTION (description 'Test function normalize_status.',\n  arguments (status VARCHAR),\n  returns VARCHAR,\n"
                    "  replay_on_change full,\n);\n\nlower(status)\n"
                ),
                "sql/models/order_statuses.sql": (
                    "MODEL (description 'Test model order_statuses.', materialized table);\n\nSELECT __udf(\"normalize_status\")('OPEN') "
                    "AS status\n"
                ),
            },
            expected_fragments=(
                "error[D002]",
                "sql/functions/sql/normalize_status.sql:4' has unsupported keys: replay_on_change",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unknown_function_header_key_when_compiling_then_fails_naming_key_and_location(
    test_case: FunctionHeaderKeyCompileCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="orders", repo_files=test_case.repo_files
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode != 0, output
    assert all(fragment in output for fragment in test_case.expected_fragments), output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
