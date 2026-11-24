"""A rebuilt snapshot does not report its own validity columns as schema changes."""

from __future__ import annotations

import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    SnapshotValidityColumnsTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

_PROJECT_TOML: str = (
    'name = "customers"\nadapter = "duckdb"\n[connection]\ndatabase = "customers.duckdb"\n'
)
_CUSTOMERS_SQL: str = (
    "MODEL (description 'Customers.', materialized table);\n\n"
    "SELECT 1 AS customer_id, 'basic' AS plan, "
    "CAST('2026-01-01 00:00:00' AS TIMESTAMP) AS updated_at\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotValidityColumnsTestCase(
            description="default validity columns",
            validity_config="",
            expected_columns=("customer_id", "plan", "updated_at", "valid_from", "valid_to"),
        ),
        SnapshotValidityColumnsTestCase(
            description="renamed validity columns",
            validity_config="  valid_from_column effective_from,\n  valid_to_column effective_to,\n",
            expected_columns=(
                "customer_id",
                "plan",
                "updated_at",
                "effective_from",
                "effective_to",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_built_snapshot_when_building_again_then_validity_columns_are_not_schema_changes(
    test_case: SnapshotValidityColumnsTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="customers_project",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML,
            "sql/models/customers.sql": _CUSTOMERS_SQL,
            "sql/models/customer_history.sql": (
                "MODEL (\n  description 'Customer history.',\n  materialized snapshot,\n"
                "  unique_key [customer_id],\n  snapshot_strategy timestamp,\n"
                f"  updated_at updated_at,\n{test_case.validity_config});\n\n"
                'SELECT customer_id, plan, updated_at FROM __ref("customers")\n'
            ),
        },
    )

    first: subprocess.CompletedProcess[str] = run_sqb(project_dir=project_dir, command=("build",))
    second: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "build")
    )
    built_columns: list[tuple[Any, ...]] = query_duckdb(
        db_path=project_dir / "customers.duckdb",
        sql=(
            "SELECT column_name FROM information_schema.columns "
            "WHERE table_name = 'customer_history' ORDER BY ordinal_position"
        ),
    )

    assert first.returncode == 0, f"{test_case.description}: {first.stdout}{first.stderr}"
    assert second.returncode == 0, f"{test_case.description}: {second.stdout}{second.stderr}"
    assert "Schema changed" not in second.stdout, f"{test_case.description}: {second.stdout}"
    assert "(removed)" not in second.stdout, f"{test_case.description}: {second.stdout}"
    assert tuple(row[0] for row in built_columns) == test_case.expected_columns, (
        test_case.description
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
