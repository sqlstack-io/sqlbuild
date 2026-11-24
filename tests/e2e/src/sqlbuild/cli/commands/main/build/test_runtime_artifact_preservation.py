"""E2E tests for runtime artifact preservation across selected reruns."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from sqlbuild._native import compiler_cache_directory
from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    RuntimeArtifactPreservationBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_waffle_shop,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        RuntimeArtifactPreservationBuildE2ETestCase(
            description="full build then selected rerun preserves existing runtime artifacts",
            initial_command=("--no-color", "build"),
            rerun_command=("--no-color", "build", "--select", "/models/marts"),
            expected_runtime_paths=(
                "target/run/sql/models/staging/stg_orders.sql",
                "target/run/sql/models/intermediate/customer_status_snapshot.sql",
                "target/run/sql/models/marts/hourly_order_activity.sql",
                "target/run/sql/models/marts/hourly_activity_with_daily_context.sql",
                "target/run/sql/functions/sql/is_completed_order.sql",
                "target/run/sql/functions/python/is_completed_order_py.sql",
            ),
            expected_exit_code=0,
            expected_compiler_cache_count=1,
            expected_local_history=False,
            expected_compiled_paths=(
                "target/compiled/sql/models/marts/fact_orders.sql",
                "target/compiled/sql/functions/sql/is_completed_order.sql",
                "target/compiled/sql/functions/python/is_completed_order_py.sql",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_full_build_when_running_selected_rerun_then_existing_runtime_artifacts_are_preserved(
    test_case: RuntimeArtifactPreservationBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)

    initial_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.initial_command,
        project_dir=project_dir,
    )
    assert initial_result.returncode == test_case.expected_exit_code, (
        initial_result.stdout + initial_result.stderr
    )

    rerun_result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.rerun_command,
        project_dir=project_dir,
    )
    assert rerun_result.returncode == test_case.expected_exit_code, (
        rerun_result.stdout + rerun_result.stderr
    )

    relative_path: str
    for relative_path in test_case.expected_runtime_paths:
        path: Path = project_dir / relative_path
        assert path.exists(), f"expected runtime artifact to exist: {path}"
    for relative_path in test_case.expected_compiled_paths:
        path = project_dir / relative_path
        assert path.exists(), f"expected compiled artifact to exist: {path}"
    compiler_caches: tuple[Path, ...] = tuple(
        compiler_cache_directory(project_dir).rglob("*.sqlite3")
    )
    assert len(compiler_caches) == test_case.expected_compiler_cache_count
    assert (project_dir / ".sqlbuild").exists() is test_case.expected_local_history
