"""Real-CLI coverage of relation references written in model SQL hooks."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import cast

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    SqlHookReadBuildE2ETestCase,
    SqlHookReadCycleE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

_DATABASE: str = "warehouse.duckdb"
_MODELS: str = "sql/models/sales"
_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "sql_hook_reads"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
        f'[connection]\ndatabase = "{_DATABASE}"\n\n'
        '[targets.dev]\nschema = "dev"\nloader_schema = "raw_dev"\ndefer_sources_to = "prod"\n\n'
        '[targets.prod]\nschema = "prod"\nloader_schema = "raw_prod"\n'
    ),
    "sql/sources/raw.yml": (
        "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n    managed: true\n    write_strategy: table\n"
        "    columns:\n      - name: order_id\n        type: INTEGER\n"
    ),
    "python/nodes/loaders/raw/loader.py": (
        "from sqlbuild.loaders import loader\n\n\n"
        "@loader\ndef raw_orders(ctx):\n    '''Test loader raw_orders.'''\n    return [{'order_id': 9}]\n"
    ),
    f"{_MODELS}/z_orders.sql": (
        "MODEL (description 'Test model z_orders.', materialized table);\nSELECT 1 AS order_id UNION ALL SELECT 2\n"
    ),
    f"{_MODELS}/_sqlbuild/_macros/python/counts.py": (
        "def count_orders(relation):\n"
        '    return f"CREATE OR REPLACE TABLE main.macro_counts AS '
        'SELECT count(*) AS n FROM {relation}"\n'
    ),
    f"{_MODELS}/_sqlbuild/_hooks/sql/record_counts.sql": (
        'HOOK (description "Test hook record_counts.");\n@py.count_orders(__ref("z_orders"))\n'
    ),
    f"{_MODELS}/a_report.sql": (
        "MODEL (description 'Test model a_report.',\n  materialized table,\n"
        "  pre_hooks [inline_sql('CREATE OR REPLACE TABLE main.inline_counts AS "
        'SELECT count(*) AS n FROM __ref("z_orders")\')],\n'
        '  post_hooks [sql("record_counts")],\n);\nSELECT 1 AS n\n'
    ),
    f"{_MODELS}/raw_marker.sql": (
        "MODEL (description 'Test model raw_marker.',\n  materialized table,\n"
        "  post_hooks [inline_sql('CREATE OR REPLACE TABLE main.raw_counts AS "
        'SELECT count(*) AS n FROM __source("raw_orders")\')],\n);\nSELECT 1 AS n\n'
    ),
}
_ENVIRONMENT_ROWS: str = (
    "CREATE SCHEMA raw_dev; CREATE SCHEMA raw_prod;"
    "CREATE TABLE raw_dev.raw_orders (order_id INTEGER); INSERT INTO raw_dev.raw_orders VALUES (1);"
    "CREATE TABLE raw_prod.raw_orders (order_id INTEGER);"
    "INSERT INTO raw_prod.raw_orders VALUES (1), (2);"
)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlHookReadBuildE2ETestCase(
            description="inline and named macro hook reads run after and read the model",
            select=("a_report", "z_orders"),
            expected_counts={"main.inline_counts": 2, "main.macro_counts": 2},
            expected_order_before=("z_orders", "a_report"),
        ),
        SqlHookReadBuildE2ETestCase(
            description="hook source read follows source deferral",
            select=("raw_marker",),
            expected_counts={"main.raw_counts": 2},
            expected_order_before=("raw_orders", "raw_marker"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sql_hook_references_when_building_then_they_resolve_and_order_like_sql(
    test_case: SqlHookReadBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="sql_hook_reads", repo_files=_PROJECT_FILES
    )
    database: Path = project_dir / _DATABASE
    execute_duckdb(db_path=database, sql=_ENVIRONMENT_ROWS)

    plan: subprocess.CompletedProcess[str] = run_sqb(
        command=("plan", "--json", "--select", *test_case.select), project_dir=project_dir
    )
    build: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--select", *test_case.select), project_dir=project_dir
    )

    assert plan.returncode == 0, plan.stdout + plan.stderr
    payload: dict[str, object] = json.loads(plan.stdout)
    planned: list[str] = [
        *(entry["name"] for entry in cast(list[dict[str, str]], payload["source_loads"])),
        *(entry["name"] for entry in cast(list[dict[str, str]], payload["models"])),
    ]
    first, second = test_case.expected_order_before
    assert planned.index(first) < planned.index(second), planned
    assert build.returncode == 0, build.stdout + build.stderr
    assert build.stdout.index(f" {first} ") < build.stdout.index(f" {second} ")
    assert {
        table: query_duckdb(db_path=database, sql=f"SELECT n FROM {table}")[0][0]
        for table in test_case.expected_counts
    } == test_case.expected_counts


@pytest.mark.parametrize(
    "test_case",
    [
        SqlHookReadCycleE2ETestCase(
            description="inline hook reading a model built from its own model",
            overrides={
                f"{_MODELS}/report_rollup.sql": (
                    'MODEL (description "Test model report_rollup.", materialized table);\nSELECT * FROM __ref("a_report")\n'
                ),
                f"{_MODELS}/a_report.sql": (
                    "MODEL (description 'Test model a_report.',\n  materialized table,\n"
                    "  post_hooks [inline_sql('SELECT * FROM __ref(\"report_rollup\")')],\n"
                    ");\nSELECT 1 AS n\n"
                ),
            },
            expected_output_fragments=(
                "error[P007]: inline SQL hook post_hooks[0] on model 'a_report' reads model "
                "'report_rollup', which depends on 'a_report'",
                "use @@CTX:destination.qualified for 'a_report' itself",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sql_hook_reading_its_own_dependant_when_compiling_then_it_reports_p007(
    test_case: SqlHookReadCycleE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="sql_hook_reads",
        repo_files=_PROJECT_FILES | test_case.overrides,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 1, output
    assert all(fragment in output for fragment in test_case.expected_output_fragments), output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
