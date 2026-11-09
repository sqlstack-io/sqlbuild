"""E2E coverage for SQL analysis complexity budgets."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    DeepSqlAnalysisCompileTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    nested_coalesce_expression,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    (
        DeepSqlAnalysisCompileTestCase(
            description="trusted model exceeds the defensive default",
            function_depth=65,
            expected_stdout_fragment="Project compiled  1 model",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_deep_trusted_model_when_compiling_then_sql_analysis_completes(
    test_case: DeepSqlAnalysisCompileTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="deep_sql_analysis",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "deep_sql_analysis"\n'
                'adapter = "duckdb"\n'
                'default_target = "dev"\n\n'
                "[connection]\n"
                'database = ":memory:"\n\n'
                "[targets.dev]\n"
                'schema = "dev"\n'
            ),
            "models/deep_expression.sql": (
                "MODEL (description 'Test model deep_expression.', columns (resolved_value (type INTEGER)));\n\n"
                f"SELECT {nested_coalesce_expression(function_depth=test_case.function_depth)} "
                "AS resolved_value\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"),
        project_dir=project_dir,
        working_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert test_case.expected_stdout_fragment in result.stdout


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
