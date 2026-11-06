"""E2E coverage that SQL macro calls after E-strings and backslash-ended strings compile."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    SqlMacroStringEscapeTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_DUCKDB_CONFIG: str = (
    'name = "order_notes"\nadapter = "duckdb"\n\n[connection]\ndatabase = "orders.duckdb"\n'
)
_POSTGRES_CONFIG: str = (
    'name = "order_notes"\nadapter = "postgres"\n\n[connection]\n'
    'database = "analytics"\nschema = "public"\n'
)
_SHOUT_MACRO: str = (
    'MACRO (description "Upper case.", arguments (v VARCHAR), returns VARCHAR);\n\nUPPER(@v)\n'
)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroStringEscapeTestCase(
            description="an E-string escaped quote on DuckDB",
            adapter_config=_DUCKDB_CONFIG,
            string_sql="E'it\\'s'",
            expected_compiled_fragment="UPPER('web')",
        ),
        SqlMacroStringEscapeTestCase(
            description="an E-string escaped quote on Postgres",
            adapter_config=_POSTGRES_CONFIG,
            string_sql="E'it\\'s'",
            expected_compiled_fragment="UPPER('web')",
        ),
        SqlMacroStringEscapeTestCase(
            description="a plain string ending in a backslash on DuckDB",
            adapter_config=_DUCKDB_CONFIG,
            string_sql="'C:\\'",
            expected_compiled_fragment="UPPER('web')",
        ),
        SqlMacroStringEscapeTestCase(
            description="a plain string ending in a backslash on Postgres",
            adapter_config=_POSTGRES_CONFIG,
            string_sql="'C:\\'",
            expected_compiled_fragment="UPPER('web')",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_string_with_backslash_when_compiling_sql_macro_call_then_call_expands(
    test_case: SqlMacroStringEscapeTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="order_notes",
        repo_files={
            "sqlbuild_project.toml": test_case.adapter_config,
            "models/_macros/sql/shout.sql": _SHOUT_MACRO,
            "models/order_notes.sql": (
                'MODEL (description "Order notes after an escaped string.");\n'
                f"SELECT {test_case.string_sql} AS note, @sql.shout('web') AS channel\n"
            ),
        },
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    assert compiled.returncode == 0, compiled.stdout + compiled.stderr
    assert "0 errors" in compiled.stdout, compiled.stdout
    compiled_sql: str = (
        project_dir / "target" / "compiled" / "models" / "order_notes.sql"
    ).read_text(encoding="utf-8")
    assert test_case.expected_compiled_fragment in compiled_sql, compiled_sql
    assert test_case.string_sql in compiled_sql, compiled_sql


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
