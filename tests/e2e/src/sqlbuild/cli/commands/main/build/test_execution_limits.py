"""E2E coverage for target-scoped build execution limits."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    ExecutionLimitBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    run_sqb,
    table_exists,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ExecutionLimitBuildE2ETestCase(
            description="project limit blocks expanded build before mutation",
            expected_exit_code=1,
            expected_tables=False,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_target_model_limit_when_building_then_effective_policy_controls_execution(
    tmp_path: Path,
    test_case: ExecutionLimitBuildE2ETestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="execution_limit_build",
        repo_files={
            "sqlbuild_project.toml": """
name = "execution_limit_build"
adapter = "duckdb"
default_target = "dev"

[connection]
database = "warehouse.duckdb"

[targets.dev]
schema = "main"

[targets.dev.execution_limits]
max_models = 1
remediation = "Automated tools must request approval before changing this policy."
""".strip(),
            "sql/models/orders.sql": "MODEL (description 'Test model orders.', materialized table);\n\nSELECT 1 AS id\n",
            "sql/models/order_items.sql": "MODEL (description 'Test model order_items.', materialized table);\n\nSELECT 1 AS id\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
    )
    output: str = result.stdout + result.stderr

    assert result.returncode == test_case.expected_exit_code, output
    assert "C413" in output
    assert "Selected models: 2" in output
    assert "Maximum models:  1" in output
    assert "No warehouse changes were made." in output
    assert "Automated tools must request approval before changing this policy." in output
    database_path: Path = project_dir / "warehouse.duckdb"
    assert table_exists(db_path=database_path, table_name="orders") is test_case.expected_tables
    assert (
        table_exists(db_path=database_path, table_name="order_items") is test_case.expected_tables
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ExecutionLimitBuildE2ETestCase(
            description="local limit override permits intentional expanded build",
            expected_exit_code=0,
            expected_tables=True,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_local_model_limit_override_when_building_then_local_policy_is_applied(
    tmp_path: Path,
    test_case: ExecutionLimitBuildE2ETestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="local_execution_limit_build",
        repo_files={
            "sqlbuild_project.toml": """
name = "local_execution_limit_build"
adapter = "duckdb"
default_target = "dev"

[connection]
database = "warehouse.duckdb"

[targets.dev]
schema = "main"

[targets.dev.execution_limits]
max_models = 1
""".strip(),
            "sqlbuild_local.toml": """
[targets.dev.execution_limits]
max_models = 2
""".strip(),
            "sql/models/orders.sql": "MODEL (description 'Test model orders.', materialized table);\n\nSELECT 1 AS id\n",
            "sql/models/order_items.sql": "MODEL (description 'Test model order_items.', materialized table);\n\nSELECT 1 AS id\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    database_path: Path = project_dir / "warehouse.duckdb"
    assert table_exists(db_path=database_path, table_name="orders") is test_case.expected_tables
    assert (
        table_exists(db_path=database_path, table_name="order_items") is test_case.expected_tables
    )
