"""DuckDB E2E coverage: snapshot validity column types and where a returning hard-deleted key opens."""

from pathlib import Path
from textwrap import dedent
from typing import Any

import duckdb
import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    SnapshotValidityTypeTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    assert_snapshot_scd2_invariants,
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

_DATABASE: str = "validity_types.duckdb"


def _load(db_path: Path, updated_at_type: str, rows: str) -> None:
    """Replace the source table with `rows`, typing updated_at as `updated_at_type`."""

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute(
        f"CREATE OR REPLACE TABLE main.raw_customers AS SELECT customer_id, plan, "
        f"CAST(updated_at AS {updated_at_type}) AS updated_at "
        f"FROM (VALUES {rows}) AS rows (customer_id, plan, updated_at)"
    )
    connection.close()


def _project(tmp_path: Path, strategy_config: str) -> Path:
    """A project with one hard-delete-invalidating snapshot over raw_customers."""

    model: str = dedent(
        """
        MODEL (
          description "Customer plans over time",
          materialized snapshot,
          unique_key [customer_id],
          {strategy}
          invalidate_hard_deletes true
        );

        SELECT customer_id, plan, updated_at FROM __source("raw_customers")
        """
    ).format(strategy=strategy_config.replace("\n", "\n  "))
    return prepare_inline_project(
        tmp_path=tmp_path,
        project_name="validity_types",
        repo_files={
            "sqlbuild_project.toml": 'name = "validity_types"\nadapter = "duckdb"\n\n'
            f'[connection]\ndatabase = "{_DATABASE}"\n',
            "sql/sources/raw.yml": (
                "sources:\n  - name: raw_customers\n    description: Customer plans.\n"
                "    schema: main\n    table: raw_customers\n"
            ),
            "sql/models/customer_plans.sql": model.strip() + "\n",
        },
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotValidityTypeTestCase(
            description="a timestamp snapshot over TIMESTAMP keeps TIMESTAMP",
            strategy_config="snapshot_strategy timestamp,\n  updated_at updated_at,",
            updated_at_type="TIMESTAMP",
            expected_validity_types=("TIMESTAMP", "TIMESTAMP"),
        ),
        SnapshotValidityTypeTestCase(
            description="a timestamp snapshot over TIMESTAMPTZ takes TIMESTAMPTZ",
            strategy_config="snapshot_strategy timestamp,\n  updated_at updated_at,",
            updated_at_type="TIMESTAMPTZ",
            expected_validity_types=("TIMESTAMP WITH TIME ZONE", "TIMESTAMP WITH TIME ZONE"),
        ),
        SnapshotValidityTypeTestCase(
            description="a timestamp snapshot over DATE takes DATE",
            strategy_config="snapshot_strategy timestamp,\n  updated_at updated_at,",
            updated_at_type="DATE",
            expected_validity_types=("DATE", "DATE"),
        ),
        SnapshotValidityTypeTestCase(
            description="a check snapshot stamps both columns as plain TIMESTAMP",
            strategy_config="snapshot_strategy check,\n  check_columns [plan],",
            updated_at_type="TIMESTAMPTZ",
            expected_validity_types=("TIMESTAMP", "TIMESTAMP"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_snapshot_when_building_then_validity_columns_take_dbts_types(
    test_case: SnapshotValidityTypeTestCase, tmp_path: Path
) -> None:
    project_dir: Path = _project(tmp_path, test_case.strategy_config)
    db_path: Path = project_dir / _DATABASE
    _load(
        db_path,
        test_case.updated_at_type,
        "(1, 'basic', '2026-01-01 10:00:00'), (2, 'basic', '2026-01-01 10:00:00')",
    )
    first = run_sqb(command=("--no-color", "build"), project_dir=project_dir)
    assert first.returncode == 0, first.stdout + first.stderr
    _load(db_path, test_case.updated_at_type, "(1, 'pro', '2026-01-02 10:00:00')")
    second = run_sqb(command=("--no-color", "build"), project_dir=project_dir)
    assert second.returncode == 0, second.stdout + second.stderr

    types: list[tuple[Any, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT data_type FROM information_schema.columns WHERE table_schema = 'main' "
            "AND table_name = 'customer_plans' AND column_name IN ('valid_from', 'valid_to') "
            "ORDER BY column_name"
        ),
    )
    versions: list[tuple[Any, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT customer_id, plan, valid_to IS NULL FROM main.customer_plans "
            "ORDER BY customer_id, valid_from"
        ),
    )
    assert tuple(row[0] for row in types) == test_case.expected_validity_types
    assert versions == [(1, "basic", False), (1, "pro", True), (2, "basic", False)]


def test_given_hard_deleted_keys_when_they_return_then_each_opens_after_its_close(
    tmp_path: Path,
) -> None:
    project_dir: Path = _project(tmp_path, "snapshot_strategy timestamp,\n  updated_at updated_at,")
    db_path: Path = project_dir / _DATABASE
    rounds: tuple[str, ...] = (
        "(1, 'basic', '2026-01-01 10:00:00'), (2, 'basic', '2026-01-01 10:00:00'), "
        "(3, 'basic', '2026-01-01 10:00:00')",
        "(1, 'basic', '2026-01-01 10:00:00')",
        "(1, 'basic', '2026-01-01 10:00:00'), (2, 'pro', '2026-01-03 11:00:00'), "
        "(3, 'pro', '2099-01-05 09:00:00')",
    )
    for rows in rounds:
        _load(db_path, "TIMESTAMP", rows)
        built = run_sqb(command=("--no-color", "build"), project_dir=project_dir)
        assert built.returncode == 0, built.stdout + built.stderr

    versions: list[tuple[Any, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT customer_id, plan, strftime(valid_from, '%Y-%m-%d %H:%M'), "
            "valid_from >= LAG(valid_to) OVER (PARTITION BY customer_id ORDER BY valid_from), "
            "valid_to IS NULL FROM main.customer_plans ORDER BY customer_id, valid_from"
        ),
    )
    returned_early_from: str = versions[2][2]
    assert versions == [
        (1, "basic", "2026-01-01 10:00", None, True),
        (2, "basic", "2026-01-01 10:00", None, False),
        (2, "pro", returned_early_from, True, True),
        (3, "basic", "2026-01-01 10:00", None, False),
        (3, "pro", "2099-01-05 09:00", True, True),
    ]
    assert returned_early_from != "2026-01-03 11:00"
    assert_snapshot_scd2_invariants(
        db_path=db_path, table_name="customer_plans", key_columns=("customer_id",)
    )
