"""E2E coverage for sqb build plan safety policies on the dbt interop execution path."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.dbt._test_types import DbtPlanSafetyE2ETestCase
from tests.e2e.src.sqlbuild.cli.commands.main.dbt.helpers import (
    declare_sqlbuild_main_target,
    prepare_dbt_interop_project,
    skip_unless_dbt_is_runnable,
    write_sqlbuild_order_status_snapshot,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb, table_exists

pytestmark: pytest.MarkDecorator = pytest.mark.dbt


@pytest.mark.parametrize(
    "test_case",
    [
        DbtPlanSafetyE2ETestCase(
            description="dbt run refuses SQLBuild work above the target model limit",
            command=("--no-color", "dbt", "run", "--select", "local_only", "local_order_ids"),
            expected_fragments=(
                "C413",
                "Selected models: 2",
                "Maximum models:  1",
                "dbt work has already run; no SQLBuild models were built.",
            ),
            unexpected_fragments=("No warehouse changes were made.",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_target_model_limit_when_running_dbt_interop_then_refuses_sqlbuild_work(
    test_case: DbtPlanSafetyE2ETestCase, tmp_path: Path
) -> None:
    skip_unless_dbt_is_runnable()
    project_dir: Path = prepare_dbt_interop_project(tmp_path=tmp_path)
    declare_sqlbuild_main_target(
        project_dir=project_dir,
        target_settings="\n[targets.main.execution_limits]\nmax_models = 1\n",
    )
    project_dir.joinpath("sql", "models", "local_order_ids.sql").write_text(
        "MODEL (description 'Test model local_order_ids.', materialized table);\n\nSELECT 11 AS order_id\n",
        encoding="utf-8",
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir, input_text=""
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 1, output
    assert all(fragment in output for fragment in test_case.expected_fragments), output
    assert not any(fragment in output for fragment in test_case.unexpected_fragments), output
    database: Path = project_dir / "dbt_interop.duckdb"
    assert not table_exists(db_path=database, table_name="local_only")
    assert not table_exists(db_path=database, table_name="local_order_ids")


@pytest.mark.parametrize(
    "test_case",
    [
        DbtPlanSafetyE2ETestCase(
            description="dbt run refuses an unconfirmed snapshot full refresh",
            command=(
                "--no-color",
                "dbt",
                "run",
                "--full-refresh",
                "--select",
                "order_status_history",
            ),
            expected_fragments=(
                "snapshot full refresh requires confirmation",
                "sqb build --allow-snapshot-full-refresh",
            ),
            unexpected_fragments=("Pass --allow-snapshot-full-refresh",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snapshot_full_refresh_requiring_confirmation_when_running_dbt_then_refuses(
    test_case: DbtPlanSafetyE2ETestCase, tmp_path: Path
) -> None:
    skip_unless_dbt_is_runnable()
    project_dir: Path = prepare_dbt_interop_project(tmp_path=tmp_path)
    declare_sqlbuild_main_target(project_dir=project_dir, target_settings="")
    write_sqlbuild_order_status_snapshot(project_dir=project_dir)
    first: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "dbt", "run", "--select", "order_status_history"),
        project_dir=project_dir,
        input_text="",
    )
    assert first.returncode == 0, first.stdout + first.stderr

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir, input_text=""
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 1, output
    assert all(fragment in output for fragment in test_case.expected_fragments), output
    assert not any(fragment in output for fragment in test_case.unexpected_fragments), output
    assert table_exists(
        db_path=project_dir / "dbt_interop.duckdb", table_name="order_status_history"
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
