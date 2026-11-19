"""A build naming jobs runs each job's nodes on the job's warehouse, switching only on change."""

from __future__ import annotations

from pathlib import Path

import pytest
from _pytest.capture import CaptureResult

from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    DynamicRefreshWarehouseTestCase,
    JobWarehouseOutputTestCase,
    JobWarehouseSwitchTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import (
    dynamic_refresh_warehouses,
    install_writable_offline_snowflake,
    run_cli_exit_code,
    unit_test_sql,
    warehouse_switches,
    write_dynamic_job_project,
    write_job_warehouse_project,
    write_project_files,
)

_BOTH_JOBS: tuple[str, ...] = (
    "--select",
    "stg",
    "job:orders_wh_job",
    "job:refunds_wh_job",
    "summary",
)


@pytest.mark.parametrize(
    "test_case",
    [
        JobWarehouseSwitchTestCase(
            description="two jobs' nodes run on their own warehouses, switching only on change",
            argv=("build", *_BOTH_JOBS),
            expected_connect_warehouses=frozenset({"BUILD_WH"}),
            expected_switches=("ORDERS_WH", "REFUNDS_WH", "BUILD_WH"),
        ),
        JobWarehouseSwitchTestCase(
            description="a single named job's warehouse is the whole build's, its task included",
            argv=("build", "--select", "job:tasks_wh_job"),
            expected_connect_warehouses=frozenset({"TASKS_WH"}),
            expected_switches=(),
        ),
        JobWarehouseSwitchTestCase(
            description="a warehouse every named job shares runs upstream nodes outside them too",
            argv=("build", "--select", "stg", "job:orders_wh_job", "job:orders_copy_job"),
            expected_connect_warehouses=frozenset({"ORDERS_WH"}),
            expected_switches=(),
        ),
        JobWarehouseSwitchTestCase(
            description="--warehouse wins over every named job, even ones that would conflict",
            argv=("build", "--warehouse", "CLI_WH", *_BOTH_JOBS, "job:refunds_alt_job"),
            expected_connect_warehouses=frozenset({"CLI_WH"}),
            expected_switches=(),
        ),
        JobWarehouseSwitchTestCase(
            description="a SQL test covering nodes with and without a job warehouse stays put",
            argv=("build", *_BOTH_JOBS),
            expected_connect_warehouses=frozenset({"BUILD_WH"}),
            expected_switches=("ORDERS_WH", "REFUNDS_WH", "BUILD_WH"),
            extra_files={"tests/unit/test_refunds.sql": unit_test_sql("refunds", "summary")},
        ),
        JobWarehouseSwitchTestCase(
            description="a SQL test covering only one job's nodes runs on its warehouse",
            argv=("build", *_BOTH_JOBS),
            expected_connect_warehouses=frozenset({"BUILD_WH"}),
            expected_switches=("REFUNDS_WH", "ORDERS_WH", "REFUNDS_WH", "BUILD_WH"),
            extra_files={"tests/unit/test_refunds.sql": unit_test_sql("refunds")},
        ),
        JobWarehouseSwitchTestCase(
            description="--no-job-settings keeps every node on the build's warehouse",
            argv=("build", "--no-job-settings", *_BOTH_JOBS),
            expected_connect_warehouses=frozenset({"BUILD_WH"}),
            expected_switches=(),
        ),
        JobWarehouseSwitchTestCase(
            description="nodes selected without naming their job stay on the build's warehouse",
            argv=("build", "--select", "+refunds"),
            expected_connect_warehouses=frozenset({"BUILD_WH"}),
            expected_switches=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_named_jobs_with_warehouses_when_building_then_each_node_runs_on_its_jobs(
    test_case: JobWarehouseSwitchTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_job_warehouse_project(project_dir=tmp_path)
    write_project_files(project_dir=tmp_path, files=test_case.extra_files)
    connect_calls, executed_sql = install_writable_offline_snowflake(monkeypatch)

    exit_code: int = run_cli_exit_code(test_case.argv, tmp_path)

    captured: CaptureResult[str] = capsys.readouterr()
    assert (
        exit_code,
        {call.get("warehouse") for call in connect_calls},
        warehouse_switches(executed_sql),
    ) == (
        0,
        test_case.expected_connect_warehouses,
        test_case.expected_switches,
    ), captured.out + captured.err


@pytest.mark.parametrize(
    "test_case",
    [
        JobWarehouseOutputTestCase(
            description="a plan names each job's warehouse",
            argv=("plan", *_BOTH_JOBS),
            expected_exit_code=0,
            expected_fragment="Nodes of job orders_wh_job run on warehouse ORDERS_WH",
        ),
        JobWarehouseOutputTestCase(
            description="plan JSON maps each job to its warehouse",
            argv=("plan", "--json", *_BOTH_JOBS),
            expected_exit_code=0,
            expected_fragment=(
                '"job_warehouses": {\n    "orders_wh_job": "ORDERS_WH",\n'
                '    "refunds_wh_job": "REFUNDS_WH"\n  }'
            ),
        ),
        JobWarehouseOutputTestCase(
            description="the run context shows the jobs' warehouses beside the build's",
            argv=("build", "--verbose", *_BOTH_JOBS),
            expected_exit_code=0,
            expected_fragment=(
                "warehouse    BUILD_WH (sqlbuild_project.toml [targets.dev.warehouses] build); "
                "nodes of job orders_wh_job on ORDERS_WH; nodes of job refunds_wh_job on "
                "REFUNDS_WH"
            ),
        ),
        JobWarehouseOutputTestCase(
            description="two jobs giving one node different warehouses conflict",
            argv=("build", "--select", "job:refunds_wh_job", "job:refunds_alt_job"),
            expected_exit_code=1,
            expected_fragment=(
                "error[S027]: model:refunds is selected through jobs refunds_wh_job and "
                'refunds_alt_job, which set warehouse differently ("REFUNDS_WH" and "ALT_WH")'
            ),
        ),
        JobWarehouseOutputTestCase(
            description="a job warehouse that is not an identifier fails before building",
            argv=("plan", "--select", "job:bad_wh_job"),
            expected_exit_code=1,
            expected_fragment=(
                "error[C261]: job bad_wh_job sets warehouse 'bad wh', which is not a valid "
                "Snowflake warehouse identifier"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_named_jobs_with_warehouses_when_planning_or_building_then_output_names_them(
    test_case: JobWarehouseOutputTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_job_warehouse_project(project_dir=tmp_path)
    install_writable_offline_snowflake(monkeypatch)

    exit_code: int = run_cli_exit_code(test_case.argv, tmp_path)

    captured: CaptureResult[str] = capsys.readouterr()
    output: str = captured.out + captured.err
    assert (exit_code, test_case.expected_fragment in output) == (
        test_case.expected_exit_code,
        True,
    ), output


@pytest.mark.parametrize(
    "test_case",
    [
        DynamicRefreshWarehouseTestCase(
            description="a job switching its nodes to its warehouse leaves refreshes on the build's",
            argv=("build", "--select", "stg", "job:orders_wh_job", "job:dynamic_wh_job"),
            expected_refresh_warehouses=("BUILD_WH",),
        ),
        DynamicRefreshWarehouseTestCase(
            description="a single job's warehouse, the whole build's, still refreshes on the build's",
            argv=("build", "--select", "job:dynamic_wh_job"),
            expected_refresh_warehouses=("BUILD_WH",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_dynamic_model_built_through_a_job_when_created_then_it_refreshes_on_the_build_s(
    test_case: DynamicRefreshWarehouseTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_dynamic_job_project(project_dir=tmp_path)
    _, executed_sql = install_writable_offline_snowflake(monkeypatch)

    exit_code: int = run_cli_exit_code(test_case.argv, tmp_path)

    captured: CaptureResult[str] = capsys.readouterr()
    assert (exit_code, dynamic_refresh_warehouses(executed_sql)) == (
        0,
        test_case.expected_refresh_warehouses,
    ), captured.out + captured.err
