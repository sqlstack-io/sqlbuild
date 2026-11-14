"""CLI e2e coverage for `sqb rename` and `sqb mv` rewriting TOML job selectors."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.refactor._test_types import (
    JobSelectorRefactorE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.refactor.helpers import sqb, write_orders_project

_JOB: str = "orchestration/jobs/orders/refresh.toml"
_UNRELATED: str = "orchestration/jobs/orders/totals.toml"
_SELECTOR_REWRITE: str = Path(
    "benchmarks/duckshop/edge_cases/orchestration/selector_rewrite/jobs/orders_refresh_job.toml"
).read_text()
_UNRELATED_JOB: str = (
    'name = "orders_totals_job"\ndescription = "Rebuild totals."\nselect = "customer_totals"\n'
)


def _job(select: str, exclude: str = "") -> str:
    return f'name = "orders_refresh_job"\ndescription = "Refresh orders."\nselect = {select}\n' + (
        f"exclude = {exclude}\n" if exclude else ""
    )


@pytest.mark.parametrize(
    "test_case",
    [
        JobSelectorRefactorE2ETestCase(
            description="a rename rewrites exact names and leaves globs and other kinds alone",
            command=("rename", "stg_orders", "stg_orders_clean"),
            job=_SELECTOR_REWRITE,
            expected_job=_SELECTOR_REWRITE.replace(
                "+stg_orders+ fact_orders,+stg_orders stg_orders~customer_totals",
                "+stg_orders_clean+ fact_orders,+stg_orders_clean stg_orders_clean~customer_totals",
            ).replace("'stg_orders+,customer_totals'", "'stg_orders_clean+,customer_totals'"),
            expected_output=("selector", _JOB),
        ),
        JobSelectorRefactorE2ETestCase(
            description="moving every model out of a folder moves its folder selectors",
            command=("mv", "stg_orders", "models/core/"),
            job=_job('"models/staging +path:models/staging/ stg_orders"'),
            expected_job=_job('"models/core +path:models/core/ stg_orders"'),
            expected_output=("selector", _JOB),
        ),
        JobSelectorRefactorE2ETestCase(
            description="moving some models out of a folder leaves it and asks for a check",
            command=("mv", "fact_orders", "models/finance/"),
            job=_job('"+models/marts"'),
            expected_job=_job('"+models/marts"'),
            expected_output=(
                "Notes",
                f"{_JOB}:3:10",
                "this job selects folder 'models/marts', and fact_orders moved out of it",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_job_selectors_when_refactoring_then_jobs_follow_and_still_build(
    tmp_path: Path, test_case: JobSelectorRefactorE2ETestCase
) -> None:
    project_dir: Path = write_orders_project(
        tmp_path=tmp_path, files={_JOB: test_case.job, _UNRELATED: _UNRELATED_JOB}
    )

    refactored: subprocess.CompletedProcess[str] = sqb(project_dir, *test_case.command)
    built: subprocess.CompletedProcess[str] = sqb(
        project_dir, "build", "--select", "job:orders_refresh_job"
    )

    output: str = refactored.stdout + refactored.stderr
    assert refactored.returncode == 0, output
    for fragment in test_case.expected_output:
        assert fragment in output, output
    assert (project_dir / _JOB).read_text(encoding="utf-8") == test_case.expected_job
    assert (project_dir / _UNRELATED).read_text(encoding="utf-8") == _UNRELATED_JOB
    assert built.returncode == 0, built.stdout + built.stderr
