"""A person's preview of a build naming a job applies the job's settings as the CLI and the run do."""

from __future__ import annotations

import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    duckdb_rows,
    run_sqb,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.job_settings._test_types import (
    JobPreviewRunTestCase,
    LoaderWarehouseTestCase,
    TypedWarehouseTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.job_settings.helpers import (
    LOADER_PROJECT_FILES,
    WAREHOUSE_HOOK,
    WAREHOUSE_REFUSAL,
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
            description="a job's changed reads consumption in the preview, as --changed does",
            command=("build", "--select", "job:changed_job"),
            expected_actions={"orders": ("incremental_append", "normal_incremental")},
            expected_job_keys={
                "changed": {
                    "counts": ["definition", "source_data"],
                    "reach": "all",
                    "runs": [],
                    "held_back": [],
                    "unchanged": ["model:orders"],
                }
            },
            expected_error_code=None,
            expected_run_status="success",
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


@pytest.mark.parametrize(
    "test_case",
    [
        TypedWarehouseTestCase(
            description="a typed warehouse replaces the job's in the preview and the run",
            command=("build", "--select", "job:warehouse_job", "--warehouse", "BUILD_XS"),
            expected_preview=({"orders": ("incremental_append", "query_changed")}, {}, None),
            expected_run=("failed", WAREHOUSE_REFUSAL),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_typed_warehouse_and_a_job_warehouse_when_previewed_and_run_then_both_take_the_typed_one(
    test_case: TypedWarehouseTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    command: list[str] = list(test_case.command)
    location: JobSettingsLocation
    with job_settings_location(tmp_path, monkeypatch) as location:
        previewed: dict[str, Any] = preview(location, command)
        run: dict[str, Any] = adhoc_run(location, command)
    assert (
        planned_actions(previewed),
        job_keys(previewed),
        error_code(previewed),
    ) == test_case.expected_preview, previewed
    assert (run["status"], run["status_reason"]) == test_case.expected_run, run


@pytest.mark.parametrize(
    "test_case",
    [
        LoaderWarehouseTestCase(
            description="a job's warehouse reaches its loader in the CLI and the ad-hoc run alike",
            command=("build", "--select", "job:loads_job", "job:customers_job"),
            expected_log="LOAD raw_customers\nUSE LOADS_WH\nLOAD raw_inventory\nUSE HOME_WH\n",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_job_holding_a_loader_when_a_person_runs_it_then_the_loader_runs_on_the_jobs_warehouse(
    test_case: LoaderWarehouseTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    hook: Path = tmp_path / "warehouse-hook"
    hook.mkdir()
    _ = (hook / "sitecustomize.py").write_text(WAREHOUSE_HOOK)
    log: Path = tmp_path / "warehouses.log"
    environment: dict[str, str] = {"PYTHONPATH": str(hook), "WAREHOUSE_LOG": str(log)}
    for name, value in environment.items():
        monkeypatch.setenv(name, value)
    command: list[str] = list(test_case.command)
    location: JobSettingsLocation
    with job_settings_location(
        tmp_path, monkeypatch, LOADER_PROJECT_FILES, environment
    ) as location:
        _ = log.write_text("")
        built: subprocess.CompletedProcess[str] = run_sqb(
            location.project, "build", "--target", "development", "--select", *command[2:]
        )
        cli: str = log.read_text()
        _ = log.write_text("")
        run: dict[str, Any] = adhoc_run(location, command)
        adhoc: str = log.read_text()
    assert (built.returncode, run["status"], cli, adhoc) == (
        0,
        "success",
        test_case.expected_log,
        test_case.expected_log,
    ), (built.stderr, run)
