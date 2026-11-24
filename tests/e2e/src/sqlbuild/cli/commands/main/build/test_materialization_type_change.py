"""A built model switching between table and view replaces the existing relation."""

from __future__ import annotations

import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    MaterializationTypeChangeTestCase,
    ViewToFullRefreshTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import order_totals_model_sql
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

_PROJECT_TOML: str = (
    'name = "orders"\nadapter = "duckdb"\n[connection]\ndatabase = "orders.duckdb"\n'
)
_MODEL_PATH: str = "sql/models/order_totals.sql"
_SNAPSHOT_PROJECT_TOML: str = (
    'name = "customers"\nadapter = "duckdb"\n[connection]\ndatabase = "customers.duckdb"\n'
    '[snapshots]\ncurrent_state_full_refresh = "allow"\n'
)
_RAW_CUSTOMERS_YML: str = (
    "sources:\n  - name: raw_customers\n    description: Raw customers.\n"
    "    schema: main\n    table: raw_customers\n"
)
_CUSTOMER_SNAPSHOT_SQL: str = (
    'SELECT customer_id, plan, updated_at FROM __source("raw_customers")\n'
)


@pytest.mark.parametrize(
    "test_case",
    [
        MaterializationTypeChangeTestCase(
            description="table then view then table",
            materializations=("table", "view", "table"),
            expected_table_types=("BASE TABLE", "VIEW", "BASE TABLE"),
        ),
        MaterializationTypeChangeTestCase(
            description="view then table then view",
            materializations=("view", "table", "view"),
            expected_table_types=("VIEW", "BASE TABLE", "VIEW"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_built_model_when_materialization_changes_then_build_replaces_relation(
    test_case: MaterializationTypeChangeTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders_project",
        repo_files={"sqlbuild_project.toml": _PROJECT_TOML},
    )
    model_path: Path = project_dir / _MODEL_PATH
    model_path.parent.mkdir(parents=True, exist_ok=True)
    outcomes: list[tuple[int, str, list[tuple[Any, ...]]]] = []
    for materialized in test_case.materializations:
        _ = model_path.write_text(
            order_totals_model_sql(materialized=materialized), encoding="utf-8"
        )
        built: subprocess.CompletedProcess[str] = run_sqb(
            project_dir=project_dir, command=("--no-color", "build")
        )
        table_types: list[tuple[Any, ...]] = query_duckdb(
            db_path=project_dir / "orders.duckdb",
            sql="SELECT table_type FROM information_schema.tables WHERE table_name = 'order_totals'",
        )
        outcomes.append((built.returncode, built.stdout + built.stderr, table_types))
    rows: list[tuple[Any, ...]] = query_duckdb(
        db_path=project_dir / "orders.duckdb", sql="SELECT order_id, total_cents FROM order_totals"
    )

    assert [outcome[0] for outcome in outcomes] == [0] * len(test_case.materializations), (
        f"{test_case.description}: {[outcome[1] for outcome in outcomes]}"
    )
    assert tuple(outcome[2][0][0] for outcome in outcomes) == test_case.expected_table_types, (
        test_case.description
    )
    assert rows == [(1, 250)], test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        ViewToFullRefreshTestCase(
            description="a view switched to a snapshot fully refreshes three times",
            model_header=(
                "MODEL (description 'Customer snapshot.', materialized snapshot, "
                "unique_key [customer_id], snapshot_strategy timestamp, updated_at updated_at);"
            ),
            full_refresh_count=3,
            expected_relations=[
                ("__sqb_prev__customer_snapshot", "BASE TABLE"),
                ("customer_snapshot", "BASE TABLE"),
            ],
            expected_rows=[(1, "basic")],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_view_model_when_switched_and_fully_refreshed_then_each_refresh_replaces_it(
    test_case: ViewToFullRefreshTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="customers_project",
        repo_files={
            "sqlbuild_project.toml": _SNAPSHOT_PROJECT_TOML,
            "sql/sources/raw.yml": _RAW_CUSTOMERS_YML,
        },
    )
    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(project_dir / "customers.duckdb"))
    connection.execute(
        "CREATE TABLE main.raw_customers AS "
        "SELECT 1 AS customer_id, 'basic' AS plan, TIMESTAMP '2024-01-01' AS updated_at"
    )
    connection.close()
    model_path: Path = project_dir / "sql/models/customer_snapshot.sql"
    model_path.parent.mkdir(parents=True, exist_ok=True)
    _ = model_path.write_text(
        "MODEL (description 'Customer snapshot.', materialized view);\n\n" + _CUSTOMER_SNAPSHOT_SQL,
        encoding="utf-8",
    )
    builds: list[subprocess.CompletedProcess[str]] = [
        run_sqb(project_dir=project_dir, command=("--no-color", "build"))
    ]
    _ = model_path.write_text(
        test_case.model_header + "\n\n" + _CUSTOMER_SNAPSHOT_SQL, encoding="utf-8"
    )
    for _refresh in range(test_case.full_refresh_count):
        builds.append(
            run_sqb(project_dir=project_dir, command=("--no-color", "build", "--full-refresh"))
        )
    relations: list[tuple[Any, ...]] = query_duckdb(
        db_path=project_dir / "customers.duckdb",
        sql=(
            "SELECT table_name, table_type FROM information_schema.tables "
            "WHERE table_name LIKE '%customer_snapshot%' ORDER BY table_name"
        ),
    )
    rows: list[tuple[Any, ...]] = query_duckdb(
        db_path=project_dir / "customers.duckdb",
        sql="SELECT customer_id, plan FROM main.customer_snapshot",
    )

    assert [build.returncode for build in builds] == [0] * len(builds), [
        build.stdout + build.stderr for build in builds
    ]
    assert relations == test_case.expected_relations
    assert rows == test_case.expected_rows


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
