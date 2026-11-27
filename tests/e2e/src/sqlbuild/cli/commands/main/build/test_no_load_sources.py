"""E2E coverage for builds with loading off: managed sources are audited, never reloaded."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import NoLoadBuildE2ETestCase
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

_AUTO_LOAD_OFF: str = "\n[settings]\nauto_load_sources = false\n"


@pytest.mark.parametrize(
    "test_case",
    [
        NoLoadBuildE2ETestCase(
            description="no-load flag without a selection audits the existing data",
            second_build=("--no-color", "build", "--no-load"),
            project_settings="",
            expected_exit_code=1,
            expected_audit_reported=True,
        ),
        NoLoadBuildE2ETestCase(
            description="auto_load_sources disabled audits the existing data",
            second_build=("--no-color", "build"),
            project_settings=_AUTO_LOAD_OFF,
            expected_exit_code=1,
            expected_audit_reported=True,
        ),
        NoLoadBuildE2ETestCase(
            description="a source selected with the no-load flag is audited, not loaded",
            second_build=("--no-color", "build", "--no-load", "--select", "raw_orders"),
            project_settings="",
            expected_exit_code=1,
            expected_audit_reported=True,
        ),
        NoLoadBuildE2ETestCase(
            description="a source selected with auto_load_sources disabled is audited, not loaded",
            second_build=("--no-color", "build", "--select", "raw_orders"),
            project_settings=_AUTO_LOAD_OFF,
            expected_exit_code=1,
            expected_audit_reported=True,
        ),
        NoLoadBuildE2ETestCase(
            description="a source outside the selection is neither loaded nor audited",
            second_build=("--no-color", "build", "--no-load", "--select", "orders"),
            project_settings="",
            expected_exit_code=0,
            expected_audit_reported=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_loaded_source_when_building_without_loading_then_only_its_audits_run(
    test_case: NoLoadBuildE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="no_load_sources",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "no_load_sources"\nadapter = "duckdb"\n\n'
                '[connection]\ndatabase = "warehouse.duckdb"\n\n'
                '[defaults]\nmaterialized = "table"\n'
            ),
            "sql/sources/raw.yml": (
                "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    managed: true\n    columns:\n"
                "      - name: order_id\n        type: BIGINT\n        audits: [not_null]\n"
            ),
            "python/nodes/loaders/orders/loader.py": (
                "from sqlbuild.loaders import loader\n\n\n"
                "@loader(write_strategy='append')\ndef raw_orders(ctx):\n"
                "    '''Test loader raw_orders.'''\n    return [{'order_id': 1}]\n"
            ),
            "sql/models/orders.sql": (
                "MODEL (description 'Test model orders.');\n"
                'SELECT r.order_id FROM __source("raw_orders") AS r\n'
            ),
        },
    )
    first: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert first.returncode == 0, first.stdout + first.stderr
    execute_duckdb(
        db_path=project_dir / "warehouse.duckdb",
        sql="INSERT INTO raw_orders (order_id) VALUES (NULL)",
    )
    settings: Path = project_dir / "sqlbuild_project.toml"
    _ = settings.write_text(settings.read_text() + test_case.project_settings)

    second: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.second_build, project_dir=project_dir
    )

    assert second.returncode == test_case.expected_exit_code, second.stdout + second.stderr
    assert ("not_null (raw_orders.order_id)" in second.stdout) == test_case.expected_audit_reported
    rows: list[tuple[object, ...]] = query_duckdb(
        db_path=project_dir / "warehouse.duckdb", sql="SELECT COUNT(*) FROM raw_orders"
    )
    assert rows == [(test_case.expected_order_rows,)]


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
