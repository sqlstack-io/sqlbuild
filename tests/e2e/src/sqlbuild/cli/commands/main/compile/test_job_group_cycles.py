"""E2E coverage for job group cycles: compile reports every cycle, then passes once they are gone."""

from __future__ import annotations

from pathlib import Path

from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb
from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import (
    GROUP_CYCLE_MESSAGE,
    GROUP_CYCLE_NODES,
    duckshop_orchestration,
)

_GROUP_PATH: str = "orchestration/job_groups/nightly_job_group.toml"


def test_given_a_job_group_with_cycles_when_compiling_then_every_cycle_fails_until_fixed(
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="group_cycles",
        repo_files={
            "sqlbuild_project.toml": 'name = "group_cycles"\nadapter = "duckdb"\n',
            **GROUP_CYCLE_NODES,
            **duckshop_orchestration("group_cycles"),
        },
    )

    failed = run_sqb(command=("compile",), project_dir=project_dir)

    assert failed.returncode == 1, failed.stdout + failed.stderr
    assert "error[D017]" in failed.stderr
    for line in GROUP_CYCLE_MESSAGE.splitlines():
        assert line in failed.stderr, failed.stderr

    # One job per pair of members that never wait on each other: no cycle is left.
    (project_dir / _GROUP_PATH).write_text(
        'name = "nightly_job_group"\ndescription = "Build everything nightly."\n'
        'jobs = ["orders_job", "shipments_job", "region_report_job"]\n',
        encoding="utf-8",
    )
    fixed = run_sqb(command=("compile",), project_dir=project_dir)

    assert fixed.returncode == 0, fixed.stdout + fixed.stderr
    assert "D017" not in fixed.stderr
