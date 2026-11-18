"""A build that names jobs takes their settings: build-wide ones once, node-level ones per node."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    JobFullRefreshTestCase,
    JobSettingsBuildTestCase,
    JobSettingsConflictTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import (
    query_duckdb_rows,
    write_project_files,
)

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "demo"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
        '[connection]\ndatabase = "demo.duckdb"\n\n[targets.dev]\n\n[vars]\namount = "1"\n'
    ),
    "models/stg_orders.sql": (
        "MODEL (description 'Staged orders.', materialized table);\n"
        "SELECT CAST(@@amount AS INTEGER) AS order_id\n"
    ),
    "models/orders.sql": (
        "MODEL (description 'Orders.', materialized table);\n"
        'SELECT order_id FROM __ref("stg_orders")\n'
    ),
    "models/customers.sql": (
        "MODEL (description 'Customers.', materialized table);\nSELECT 7 AS customer_id\n"
    ),
    "models/order_events.sql": (
        "MODEL (description 'Order events.', materialized incremental, "
        "incremental_strategy append);\nSELECT 1 AS event_id\n"
    ),
    "models/refund_events.sql": (
        "MODEL (description 'Refund events.', materialized incremental, "
        "incremental_strategy append);\nSELECT 1 AS event_id\n"
    ),
    "tests/unit/test_orders.sql": (
        "TEST();\n\nWITH\n__ref__stg_orders AS (SELECT 4 AS order_id),\n"
        "__expected__orders AS (SELECT 4 AS order_id)\nSELECT 1\n"
    ),
    "orchestration/jobs/orders_job.toml": (
        'name = "orders_job"\ndescription = "Orders."\nselect = "+orders"\n\n'
        '[settings]\nconcurrency = 1\nrun_tests = false\nvars = { amount = "5" }\n'
    ),
    "orchestration/jobs/checked_orders_job.toml": (
        'name = "checked_orders_job"\ndescription = "Orders, tested."\nselect = "orders"\n\n'
        '[settings]\nrun_tests = true\nvars = { amount = "5" }\n'
    ),
    "orchestration/jobs/crm_job.toml": (
        'name = "crm_job"\ndescription = "Customers."\nselect = "customers"\n\n'
        '[settings]\nvars = { amount = "9" }\n'
    ),
    "orchestration/jobs/events_job.toml": (
        'name = "events_job"\ndescription = "Events."\nselect = "order_events"\n\n'
        "[settings]\nfull_refresh = true\n"
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        JobSettingsBuildTestCase(
            description="a named job's vars, concurrency and run_tests apply",
            build_args=("--select", "job:orders_job"),
            expected_amount=5,
            expected_fragments=("(concurrency: 1)", "TOTAL=2"),
        ),
        JobSettingsBuildTestCase(
            description="a bare job name names the job",
            build_args=("--select", "orders_job"),
            expected_amount=5,
            expected_fragments=("(concurrency: 1)", "TOTAL=2"),
        ),
        JobSettingsBuildTestCase(
            description="command-line flags beat the job's settings",
            build_args=(
                "--select",
                "job:orders_job",
                "--vars",
                '{"amount": "2"}',
                "--concurrency",
                "3",
            ),
            expected_amount=2,
            expected_fragments=("(concurrency: 3)", "TOTAL=2"),
        ),
        JobSettingsBuildTestCase(
            description="an abbreviated command-line flag still beats the job's setting",
            build_args=("--select", "job:orders_job", "--concur", "2"),
            expected_amount=5,
            expected_fragments=("(concurrency: 2)", "TOTAL=2"),
        ),
        JobSettingsBuildTestCase(
            description="--no-job-settings builds with the project's settings",
            build_args=("--select", "job:orders_job", "--no-job-settings"),
            expected_amount=1,
            expected_fragments=("TOTAL=3",),
        ),
        JobSettingsBuildTestCase(
            description="selecting the job's nodes without naming it takes none of its settings",
            build_args=("--select", "+orders"),
            expected_amount=1,
            expected_fragments=("TOTAL=3",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_named_job_when_building_then_its_settings_apply_below_the_command_line(
    test_case: JobSettingsBuildTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_project_files(project_dir=tmp_path, files=_PROJECT_FILES)

    exit_code: int = main(
        ["--project-dir", str(tmp_path), "--no-color", "build", *test_case.build_args]
    )
    captured: str = "".join(capsys.readouterr())

    assert exit_code == 0, captured
    assert query_duckdb_rows(
        db_path=tmp_path / "demo.duckdb", sql="SELECT order_id FROM orders"
    ) == ((test_case.expected_amount,),)
    assert {fragment: fragment in captured for fragment in test_case.expected_fragments} == (
        dict.fromkeys(test_case.expected_fragments, True)
    ), captured


@pytest.mark.parametrize(
    "test_case",
    [
        JobSettingsConflictTestCase(
            description="two named jobs with different vars cannot build together",
            command_args=("build", "--select", "job:orders_job", "job:crm_job"),
            expected_fragment=(
                'error[S027]: jobs orders_job and crm_job set vars differently ({"amount":"5"} '
                'and {"amount":"9"}); a build applies vars to everything it runs'
            ),
        ),
        JobSettingsConflictTestCase(
            description="a plan naming jobs with different vars fails the same way",
            command_args=("plan", "--select", "job:orders_job", "job:crm_job"),
            expected_fragment=(
                'error[S027]: jobs orders_job and crm_job set vars differently ({"amount":"5"} '
                'and {"amount":"9"}); a build applies vars to everything it runs'
            ),
        ),
        JobSettingsConflictTestCase(
            description="two named jobs giving one node different run_tests conflict",
            command_args=("build", "--select", "job:orders_job", "job:checked_orders_job"),
            expected_fragment=(
                "error[S027]: model:orders is selected through jobs orders_job and "
                "checked_orders_job, which set run_tests differently (false and true)"
            ),
        ),
        JobSettingsConflictTestCase(
            description="a group whose jobs set different vars does not compile",
            command_args=("compile",),
            expected_fragment=(
                "jobs orders_job and crm_job set vars differently; a group runs its jobs as one "
                "build, which has one vars"
            ),
            extra_files={
                "orchestration/job_groups/daily_job_group.toml": (
                    'name = "daily_job_group"\ndescription = "Daily."\n'
                    'jobs = ["orders_job", "crm_job"]\n'
                ),
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_conflicting_job_settings_when_running_then_the_command_fails_naming_them(
    test_case: JobSettingsConflictTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_project_files(project_dir=tmp_path, files={**_PROJECT_FILES, **test_case.extra_files})

    exit_code: int = main(["--project-dir", str(tmp_path), "--no-color", *test_case.command_args])
    captured: str = "".join(capsys.readouterr())

    assert exit_code == 1, captured
    assert test_case.expected_fragment in captured.replace("\n", " ").replace("  ", " "), captured


@pytest.mark.parametrize(
    "test_case",
    [
        JobFullRefreshTestCase(
            description="a named job's full_refresh rebuilds only its own incremental models",
            build_args=("--select", "job:events_job", "refund_events"),
            expected_counts=((1, 3),),
        ),
        JobFullRefreshTestCase(
            description="--full-refresh rebuilds every selected incremental model",
            build_args=("--select", "job:events_job", "refund_events", "--full-refresh"),
            expected_counts=((1, 1),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_named_job_with_full_refresh_when_building_then_only_its_models_start_over(
    test_case: JobFullRefreshTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_project_files(project_dir=tmp_path, files=_PROJECT_FILES)
    arguments: list[str] = ["--project-dir", str(tmp_path), "--no-color", "build", "--no-tests"]
    first: int = main([*arguments, "--select", "order_events", "refund_events"])
    second: int = main([*arguments, "--select", "order_events", "refund_events"])

    exit_code: int = main([*arguments, *test_case.build_args])
    captured: str = "".join(capsys.readouterr())

    assert (first, second, exit_code) == (0, 0, 0), captured
    assert (
        query_duckdb_rows(
            db_path=tmp_path / "demo.duckdb",
            sql="SELECT (SELECT count(*) FROM order_events), (SELECT count(*) FROM refund_events)",
        )
        == test_case.expected_counts
    )
