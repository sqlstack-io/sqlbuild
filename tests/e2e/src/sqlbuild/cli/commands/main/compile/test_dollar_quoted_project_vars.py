"""E2E coverage for project variables around dollar-quoted text."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    DollarQuotedProjectVarCompileCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT_TOML: str = 'name = "orders"\nadapter = "duckdb"\n\n[vars]\nregion = "north"\n'
_MODEL_HEADER: str = "MODEL (description 'Order region labels.', materialized view);\n\n"
_COMPILED_MODEL: Path = Path("target/compiled/models/order_labels.sql")


@pytest.mark.parametrize(
    "test_case",
    [
        DollarQuotedProjectVarCompileCase(
            description="comment_markers_inside_dollar_quotes_are_text",
            model_sql=(
                "SELECT $$--@@region$$ AS label, $tag$ it's /* $tag$ AS note, "
                "$$ /* $$ AS a, '@@region' AS b -- */\n"
            ),
            expected_fragment=(
                "SELECT $$--north$$ AS label, $tag$ it's /* $tag$ AS note, "
                "$$ /* $$ AS a, 'north' AS b -- */"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_dollar_quoted_model_sql_when_compiling_then_vars_substitute_inside_quotes(
    test_case: DollarQuotedProjectVarCompileCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML,
            "sql/models/order_labels.sql": _MODEL_HEADER + test_case.model_sql,
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    compiled_sql: str = (project_dir / _COMPILED_MODEL).read_text(encoding="utf-8")
    assert test_case.expected_fragment in compiled_sql


@pytest.mark.parametrize(
    "test_case",
    [
        DollarQuotedProjectVarCompileCase(
            description="unclosed_dollar_quote_reports_interpolation_error",
            model_sql="SELECT $tag$ @@region AS label\n",
            expected_fragment="error[P001]: SQL interpolation contains an unclosed quoted string",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unclosed_dollar_quote_with_var_when_compiling_then_interpolation_reports_it(
    test_case: DollarQuotedProjectVarCompileCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML,
            "sql/models/order_labels.sql": _MODEL_HEADER + test_case.model_sql,
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"),
        project_dir=project_dir,
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 1, output
    assert test_case.expected_fragment in output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
