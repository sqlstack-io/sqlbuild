"""Real-CLI coverage of Python reads of a managed source whose reads defer to another target."""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    PythonSourceDeferralE2ETestCase,
    PythonSourceTaskOnlyE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

_DATABASE: str = "warehouse.duckdb"
_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "deferred_sources"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
        f'[connection]\ndatabase = "{_DATABASE}"\n\n'
        '[targets.dev]\nschema = "dev"\nloader_schema = "raw_dev"\ndefer_sources_to = "prod"\n\n'
        '[targets.prod]\nschema = "prod"\nloader_schema = "raw_prod"\n'
    ),
    "sources/raw.yml": (
        "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n    managed: true\n    write_strategy: table\n"
        "    columns:\n      - name: order_id\n        type: INTEGER\n"
    ),
    "python/loaders/raw.py": (
        "from sqlbuild.loaders import loader\n\n\n"
        "@loader\ndef raw_orders(ctx):\n    '''Test loader raw_orders.'''\n    return [{'order_id': 9}]\n"
    ),
    "models/sales/_sqlbuild/_hooks/python/count.py": (
        "from sqlbuild.hooks import hook\nfrom sqlbuild.refs import source\n\n\n"
        '@hook(reads=source("raw_orders"))\n'
        "def count_raw(ctx):\n"
        '    """Test hook count_raw."""\n    raw = ctx.relation(source("raw_orders"))\n'
        '    ctx.execute_sql(f"CREATE OR REPLACE TABLE main.hook_counts AS '
        'SELECT count(*) AS n FROM {raw}")\n'
    ),
    "models/sales/order_count.sql": (
        'MODEL (description "Test model order_count.", materialized table, post_hooks [python("count_raw")]);\n'
        'SELECT count(*) AS n FROM __source("raw_orders")\n'
    ),
    "models/sales/order_marker.sql": (
        'MODEL (description "Test model order_marker.", materialized table, post_hooks [python("count_raw")]);\nSELECT 1 AS n\n'
    ),
    "python/tasks/count.py": (
        "from sqlbuild.refs import source\nfrom sqlbuild.assets import asset\n\n\n"
        '@asset(depends_on=source("raw_orders"))\n'
        "def count_task(ctx):\n"
        '    """Test task count_task."""\n    raw = ctx.relation(source("raw_orders"))\n'
        '    ctx.execute_sql(f"CREATE OR REPLACE TABLE main.task_counts AS '
        'SELECT count(*) AS n FROM {raw}")\n'
    ),
    "python/checks/count.py": (
        "from sqlbuild.checks import check\nfrom sqlbuild.refs import source\n\n\n"
        '@check(depends_on=source("raw_orders"))\n'
        "def two_raw_orders(ctx):\n"
        '    """Test check two_raw_orders."""\n    raw = ctx.relation(source("raw_orders"))\n'
        '    n = ctx.query(f"SELECT count(*) FROM {raw}").fetchone()[0]\n'
        '    return ctx.pass_() if n == 2 else ctx.fail(message=f"read {n} rows")\n'
    ),
}
_ENVIRONMENT_ROWS: str = (
    "CREATE SCHEMA raw_dev; CREATE SCHEMA raw_prod;"
    "CREATE TABLE raw_dev.raw_orders (order_id INTEGER); INSERT INTO raw_dev.raw_orders VALUES (1);"
    "CREATE TABLE raw_prod.raw_orders (order_id INTEGER);"
    "INSERT INTO raw_prod.raw_orders VALUES (1), (2);"
)
_CHECK_PASSED: str = r"check\s+two_raw_orders\s+PASS"
_TASK_RAN: str = r"count_task\s+OK"


@pytest.mark.parametrize(
    "test_case",
    [
        PythonSourceDeferralE2ETestCase(
            description="hook and task read the deferred source a model reads",
            select=("order_count", "asset:count_task"),
            expected_counts={"dev.order_count": 2, "main.hook_counts": 2, "main.task_counts": 2},
            expected_check_pattern=_CHECK_PASSED,
        ),
        PythonSourceDeferralE2ETestCase(
            description="hook on a model that does not read the source still reads it deferred",
            select=("order_marker",),
            expected_counts={"main.hook_counts": 2},
            expected_check_pattern=_CHECK_PASSED,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_deferred_source_reads_when_python_reads_the_source_then_it_reads_like_sql(
    test_case: PythonSourceDeferralE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="deferred_sources", repo_files=_PROJECT_FILES
    )
    database: Path = project_dir / _DATABASE
    execute_duckdb(db_path=database, sql=_ENVIRONMENT_ROWS)

    build: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--select", *test_case.select), project_dir=project_dir
    )
    checked: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "check"), project_dir=project_dir
    )

    assert build.returncode == 0, build.stdout + build.stderr
    assert {
        table: query_duckdb(db_path=database, sql=f"SELECT n FROM {table}")[0][0]
        for table in test_case.expected_counts
    } == test_case.expected_counts
    assert checked.returncode == 0, checked.stdout + checked.stderr
    assert re.search(test_case.expected_check_pattern, checked.stdout), checked.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        PythonSourceTaskOnlyE2ETestCase(
            description="with source deferral",
            target_config='defer_sources_to = "prod"\n',
            expected_task_count=2,
        ),
        PythonSourceTaskOnlyE2ETestCase(
            description="without source deferral",
            target_config="",
            expected_task_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_only_a_task_selected_when_building_then_it_reads_its_source_like_sql(
    test_case: PythonSourceTaskOnlyE2ETestCase,
    tmp_path: Path,
) -> None:
    files: dict[str, str] = dict(_PROJECT_FILES)
    files["sqlbuild_project.toml"] = files["sqlbuild_project.toml"].replace(
        'defer_sources_to = "prod"\n', test_case.target_config
    )
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="deferred_sources", repo_files=files
    )
    database: Path = project_dir / _DATABASE
    execute_duckdb(db_path=database, sql=_ENVIRONMENT_ROWS)

    build: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--select", "asset:count_task"), project_dir=project_dir
    )

    output: str = build.stdout + build.stderr
    assert build.returncode == 0, output
    assert "G000" not in output
    assert "missing from the resolved plan source map" not in output
    assert re.search(_TASK_RAN, output), output
    assert (
        query_duckdb(db_path=database, sql="SELECT n FROM main.task_counts")[0][0]
        == test_case.expected_task_count
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
