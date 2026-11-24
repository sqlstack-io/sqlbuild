"""E2E coverage that a DuckDB storage version in user config is not applied to the warehouse."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    ConnectionStorageKeyBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ConnectionStorageKeyBuildE2ETestCase(
            description="a storage version key is ignored like any unknown connection key",
            connection_key="storage_compatibility_version",
            expected_storage_version="v2.0.0+",
        ),
        ConnectionStorageKeyBuildE2ETestCase(
            description="an unknown connection key is ignored",
            connection_key="unknown_option",
            expected_storage_version="v2.0.0+",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_storage_key_in_connection_when_building_then_warehouse_keeps_default_storage(
    test_case: ConnectionStorageKeyBuildE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="storage_key",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "storage_key"\nadapter = "duckdb"\n\n'
                f'[connection]\ndatabase = "warehouse.duckdb"\n{test_case.connection_key} = "latest"\n'
            ),
            "sql/models/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1 AS order_id\n",
        },
    )

    built: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    assert built.returncode == 0, built.stdout + built.stderr
    tags: list[tuple[object, ...]] = query_duckdb(
        db_path=project_dir / "warehouse.duckdb",
        sql="SELECT tags FROM duckdb_databases() WHERE database_name = 'warehouse'",
    )
    assert tags == [({"storage_version": test_case.expected_storage_version},)]
