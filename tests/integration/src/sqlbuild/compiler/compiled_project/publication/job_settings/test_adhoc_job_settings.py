"""A person's preview of a build naming a job applies the job's settings as the CLI and the run do."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    duckdb_rows,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.job_settings._test_types import (
    JobPreviewRunTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.job_settings.helpers import (
    JobSettingsLocation,
    adhoc_run,
    cli_plan,
    error_code,
    job_keys,
    job_settings_location,
    planned_actions,
    preview,
)


@pytest.mark.parametrize(
    "test_case",
    [
        JobPreviewRunTestCase(
            description="a job's full refresh and vars rebuild its model in the preview and the run",
            command=("build", "--select", "job:refresh_job"),
            expected_actions={"orders": ("create_table", "full_refresh")},
            expected_job_keys={"full_refresh_jobs": ["refresh_job"]},
            expected_error_code=None,
            expected_run_status="success",
            expected_amounts=[(5,)],
        ),
        JobPreviewRunTestCase(
            description="a job's concurrency shows in the preview",
            command=("build", "--select", "job:limited_job"),
            expected_actions={"orders": ("incremental_append", "normal_incremental")},
            expected_job_keys={"job_concurrency": {"limited_job": 1}},
            expected_error_code=None,
            expected_run_status="success",
            expected_amounts=[(1,)],
        ),
        JobPreviewRunTestCase(
            description="a job's warehouse, which DuckDB cannot select, fails the preview as the run",
            command=("build", "--select", "job:warehouse_job"),
            expected_actions={},
            expected_job_keys={},
            expected_error_code="C260",
            expected_run_status="failed",
            expected_amounts=[(1,)],
        ),
        JobPreviewRunTestCase(
            description="a build naming no job takes no job settings",
            command=("build", "--select", "orders"),
            expected_actions={"orders": ("incremental_append", "normal_incremental")},
            expected_job_keys={},
            expected_error_code=None,
            expected_run_status="success",
            expected_amounts=[(1,)],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_build_naming_a_job_when_a_person_previews_and_runs_it_then_both_apply_its_settings(
    test_case: JobPreviewRunTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    command: list[str] = list(test_case.command)
    location: JobSettingsLocation
    with job_settings_location(tmp_path, monkeypatch) as location:
        previewed: dict[str, Any] = preview(location, command)
        planned: dict[str, Any] = cli_plan(location.project, command)
        run: dict[str, Any] = adhoc_run(location, command)
        log: str = location.log.read_text()
    assert (planned_actions(previewed), job_keys(previewed), error_code(previewed)) == (
        planned_actions(planned),
        job_keys(planned),
        error_code(planned),
    ), log
    assert planned_actions(previewed) == test_case.expected_actions, previewed
    assert job_keys(previewed) == test_case.expected_job_keys, previewed
    assert error_code(previewed) == test_case.expected_error_code, previewed
    assert run["status"] == test_case.expected_run_status, run
    assert (
        duckdb_rows(
            location.project / "warehouse.duckdb",
            "SELECT DISTINCT amount FROM orders",
        )
        == test_case.expected_amounts
    )
