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
    "orchestration/jobs.py": (
        "from pathlib import Path\n"
        "from sqlbuild.orchestration import job, job_group\n"
        "Path(__file__).parent.parent.joinpath('jobs_imported').write_text('yes')\n"
        "nightly = job('nightly', description='Orders.', select='+orders')\n"
        "crm = job('crm', description='Customers.', select='customers')\n"
        "daily = job_group('daily', description='Everything.', jobs=[nightly, crm])\n"
        "restock = job('restock', description='Prod only.', select='customers', targets=['prod'])\n"
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        JobSelectorBuildTestCase(
            description="a job prefix builds the job's nodes",
            build_args=("--select", "job:nightly"),
            expected_exit_code=0,
            expected_tables=frozenset({"orders", "stg_orders"}),
            expected_jobs_imported=True,
        ),
        JobSelectorBuildTestCase(
            description="a bare job name needs no prefix",
            build_args=("--select", "nightly"),
            expected_exit_code=0,
            expected_tables=frozenset({"orders", "stg_orders"}),
            expected_jobs_imported=True,
        ),
        JobSelectorBuildTestCase(
            description="a job group builds its jobs' nodes",
            build_args=("--select", "job:daily"),
            expected_exit_code=0,
            expected_tables=frozenset({"customers", "orders", "stg_orders"}),
            expected_jobs_imported=True,
        ),
        JobSelectorBuildTestCase(
            description="a job intersects with a tag",
            build_args=("--select", "job:daily,tag:staging"),
            expected_exit_code=0,
            expected_tables=frozenset({"stg_orders"}),
            expected_jobs_imported=True,
        ),
        JobSelectorBuildTestCase(
            description="a job can be excluded from a group",
            build_args=("--select", "job:daily", "--exclude", "job:crm"),
            expected_exit_code=0,
            expected_tables=frozenset({"orders", "stg_orders"}),
            expected_jobs_imported=True,
        ),
        JobSelectorBuildTestCase(
            description="a job declared for another target is not selectable",
            build_args=("--select", "job:restock"),
            expected_exit_code=1,
            expected_tables=frozenset(),
            expected_jobs_imported=True,
            expected_fragment="unknown job name 'restock'",
        ),
        JobSelectorBuildTestCase(
            description="a job declared for the selected target is selectable",
            build_args=("--target", "prod", "--select", "job:restock"),
            expected_exit_code=0,
            expected_tables=frozenset({"customers"}),
            expected_jobs_imported=True,
        ),
        JobSelectorBuildTestCase(
            description="model selectors never import orchestration modules",
            build_args=("--select", "+orders"),
            expected_exit_code=0,
            expected_tables=frozenset({"orders", "stg_orders"}),
            expected_jobs_imported=False,
        ),
        JobSelectorBuildTestCase(
            description="an unknown job suggests a known one",
            build_args=("--select", "job:nightyl"),
            expected_exit_code=1,
            expected_tables=frozenset(),
            expected_jobs_imported=True,
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
    assert (tmp_path / "jobs_imported").exists() == test_case.expected_jobs_imported
    with duckdb.connect(str(tmp_path / "demo.duckdb")) as warehouse:
        rows: list[tuple[str]] = warehouse.sql(
            "SELECT table_name FROM duckdb_tables() "
            "WHERE NOT internal AND NOT starts_with(table_name, '_sqlbuild')"
        ).fetchall()
    assert {name for (name,) in rows} == test_case.expected_tables
