"""E2E coverage of DuckDB `attach` entries across connections, run from the repository root."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import DuckDbAttachE2ETestCase
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    REPO_ROOT,
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

_PROJECT_TOML: str = (
    'name = "shop"\nadapter = "duckdb"\n\n[connection]\ndatabase = "shop.duckdb"\n\n'
    '[[connection.attach]]\npath = "{attach_path}"\nalias = "archive"\n\n'
    "[scopes]\nenforce_placement = false\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        DuckDbAttachE2ETestCase(
            description="one connection attaches the file from the project directory",
            concurrency=1,
            expected_archived_rows=[(1,)],
        ),
        DuckDbAttachE2ETestCase(
            description="several connections attach the same file once",
            concurrency=2,
            expected_archived_rows=[(1,)],
        ),
        DuckDbAttachE2ETestCase(
            description="a home-relative path attaches the file in the home directory",
            concurrency=1,
            expected_archived_rows=[(1,)],
            attach_path="~/archive.duckdb",
            expected_archive_root="home",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_attached_duckdb_file_when_building_then_models_land_in_it(
    test_case: DuckDbAttachE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML.format(attach_path=test_case.attach_path),
            "sql/models/archived_orders.sql": (
                "MODEL (description 'Test model archived_orders.', materialized table, "
                "database archive, schema main);\nSELECT 1 AS order_id\n"
            ),
            "sql/models/customers.sql": (
                "MODEL (description 'Test model customers.', materialized table);\n"
                "SELECT 1 AS customer_id\n"
            ),
            "sql/models/order_ids.sql": (
                "MODEL (description 'Test model order_ids.', materialized table);\n"
                'SELECT o.order_id FROM __ref("archived_orders") AS o\n'
            ),
        },
    )
    home: Path = tmp_path / "home"
    home.mkdir()
    built: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--concurrency", str(test_case.concurrency)),
        project_dir=project_dir,
        env={"HOME": str(home)},
    )
    archive_root: Path = {"project": project_dir, "home": home}[test_case.expected_archive_root]

    assert built.returncode == 0, built.stdout + built.stderr
    assert (
        query_duckdb(
            db_path=archive_root / "archive.duckdb", sql="SELECT order_id FROM archived_orders"
        )
        == test_case.expected_archived_rows
    ), test_case.description
    assert not (REPO_ROOT / "archive.duckdb").exists()
    assert not (project_dir / "~").exists()
