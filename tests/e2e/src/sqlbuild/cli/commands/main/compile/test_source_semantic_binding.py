"""Warehouse schemas close uncontracted sources before model execution."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import SourceSemanticBindingCase
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    run_sqb,
    table_exists,
)


@pytest.mark.parametrize(
    "test_case",
    [
        SourceSemanticBindingCase(
            description="plan checks inspected source columns", command="plan"
        ),
        SourceSemanticBindingCase(
            description="build checks before executing models", command="build"
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_open_source_when_planning_then_live_columns_reject_missing_reference(
    test_case: SourceSemanticBindingCase,
    tmp_path: Path,
) -> None:
    project: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="source_binding",
        repo_files={
            "sqlbuild_project.toml": 'name = "source_binding"\nadapter = "duckdb"\n[connection]\ndatabase = "orders.duckdb"\n',
            "sql/sources/orders.yml": "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n    table: raw_orders\n",
            "sql/models/orders.sql": 'MODEL (description "Test model orders.", materialized table);\nSELECT missing FROM __source("raw_orders")\n',
        },
    )
    database: Path = project / "orders.duckdb"
    execute_duckdb(db_path=database, sql="CREATE TABLE raw_orders AS SELECT 1 AS id")
    compiled: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project, command=("compile", "--no-cache")
    )
    assert compiled.returncode == 0, compiled.stdout + compiled.stderr
    planned: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project, command=(test_case.command,)
    )
    assert planned.returncode != 0
    assert f"error[{test_case.expected_code}]" in planned.stdout + planned.stderr
    assert "sql/models/orders.sql:2:" in planned.stdout + planned.stderr
    assert not table_exists(db_path=database, table_name="orders")


@pytest.mark.parametrize(
    "test_case",
    [
        SourceSemanticBindingCase(
            description="plan stops before connection", command="plan", expected_code="B300"
        ),
        SourceSemanticBindingCase(
            description="build stops before connection", command="build", expected_code="B300"
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_metadata_when_planning_then_no_warehouse_connection_is_opened(
    test_case: SourceSemanticBindingCase,
    tmp_path: Path,
) -> None:
    project: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="metadata_binding",
        repo_files={
            "sqlbuild_project.toml": 'name = "metadata_binding"\nadapter = "duckdb"\n[connection]\ndatabase = "orders.duckdb"\n',
            "sql/models/orders.sql": "MODEL (description 'Test model orders.', materialized table, unique_key [missing]);\nSELECT 1 AS id\n",
        },
    )
    result: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project, command=(test_case.command,)
    )
    assert result.returncode != 0
    assert f"error[{test_case.expected_code}]" in result.stdout + result.stderr
    assert not (project / "orders.duckdb").exists()


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
