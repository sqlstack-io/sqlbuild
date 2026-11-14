"""Jobs select their nodes on the command line like any other resource, loading only when named."""

from __future__ import annotations

from pathlib import Path

import duckdb
import pytest

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import JobSelectorBuildTestCase

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "demo"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
        '[connection]\ndatabase = "demo.duckdb"\n\n[targets.dev]\n\n[targets.prod]\n'
    ),
    "models/stg_orders.sql": (
        "MODEL (description 'Staged orders.', materialized table, tags [staging]);\n"
        "SELECT 1 AS order_id\n"
    ),
    "models/orders.sql": (
        "MODEL (description 'Orders.', materialized table, tags [finance]);\n"
        'SELECT order_id FROM __ref("stg_orders")\n'
    ),
    "models/customers.sql": (
        "MODEL (description 'Customers.', materialized table);\nSELECT 7 AS customer_id\n"
    ),
    "orchestration/jobs/nightly_job.toml": (
        'name = "nightly_job"\ndescription = "Orders."\nselect = "+orders"\n'
    ),
    "orchestration/jobs/crm_job.toml": (
        'name = "crm_job"\ndescription = "Customers."\nselect = "customers"\n'
    ),
    "orchestration/jobs/restock_job.toml": (
        'name = "restock_job"\n'
        'description = "Prod only."\n'
        'select = "customers"\n'
        'targets = ["prod"]\n'
    ),
    "orchestration/job_groups/daily_job_group.toml": (
        'name = "daily_job_group"\ndescription = "Everything."\njobs = ["nightly_job", "crm_job"]\n'
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        JobSelectorBuildTestCase(
            description="a job prefix builds the job's nodes",
            build_args=("--select", "job:nightly_job"),
            expected_exit_code=0,
            expected_tables=frozenset({"orders", "stg_orders"}),
        ),
        JobSelectorBuildTestCase(
            description="a bare job name needs no prefix",
            build_args=("--select", "nightly_job"),
            expected_exit_code=0,
            expected_tables=frozenset({"orders", "stg_orders"}),
        ),
        JobSelectorBuildTestCase(
            description="a job group builds its jobs' nodes",
            build_args=("--select", "job:daily_job_group"),
            expected_exit_code=0,
            expected_tables=frozenset({"customers", "orders", "stg_orders"}),
        ),
        JobSelectorBuildTestCase(
            description="a job intersects with a tag",
            build_args=("--select", "job:daily_job_group,tag:staging"),
            expected_exit_code=0,
            expected_tables=frozenset({"stg_orders"}),
        ),
        JobSelectorBuildTestCase(
            description="a job can be excluded from a group",
            build_args=("--select", "job:daily_job_group", "--exclude", "job:crm_job"),
            expected_exit_code=0,
            expected_tables=frozenset({"orders", "stg_orders"}),
        ),
        JobSelectorBuildTestCase(
            description="a job declared for another target is not selectable",
            build_args=("--select", "job:restock_job"),
            expected_exit_code=1,
            expected_tables=frozenset(),
            expected_fragment="unknown job name 'restock_job'",
        ),
        JobSelectorBuildTestCase(
            description="a job declared for the selected target is selectable",
            build_args=("--target", "prod", "--select", "job:restock_job"),
            expected_exit_code=0,
            expected_tables=frozenset({"customers"}),
        ),
        JobSelectorBuildTestCase(
            description="an unknown job suggests a known one",
            build_args=("--select", "job:nightyl"),
            expected_exit_code=1,
            expected_tables=frozenset(),
            expected_fragment="unknown job name 'nightyl'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_declared_jobs_when_building_by_job_then_only_their_nodes_build(
    test_case: JobSelectorBuildTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    for relative_path, contents in _PROJECT_FILES.items():
        destination: Path = tmp_path / relative_path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(contents, encoding="utf-8")

    exit_code: int = main(
        ["--project-dir", str(tmp_path), "--no-color", "build", "--no-tests", *test_case.build_args]
    )
    captured: str = "".join(capsys.readouterr())

    assert exit_code == test_case.expected_exit_code, captured
    assert test_case.expected_fragment in captured
    with duckdb.connect(str(tmp_path / "demo.duckdb")) as warehouse:
        rows: list[tuple[str]] = warehouse.sql(
            "SELECT table_name FROM duckdb_tables() "
            "WHERE NOT internal AND NOT starts_with(table_name, '_sqlbuild')"
        ).fetchall()
    assert {name for (name,) in rows} == test_case.expected_tables
