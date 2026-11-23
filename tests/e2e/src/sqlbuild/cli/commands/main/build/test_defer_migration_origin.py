"""Real-CLI coverage of deferred references to a model renamed with migrate_from."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    DeferredMigrationOriginE2ETestCase,
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
        'name = "deferred_renames"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
        f'[connection]\ndatabase = "{_DATABASE}"\n\n'
        '[targets.dev]\nschema = "dev"\n\n'
        '[targets.prod]\nschema = "preserve"\n'
    ),
    "sql/models/order_ledger.sql": (
        'MODEL (description "Test model order_ledger.", materialized table, '
        "schema prod, migrate_from order_history);\n\nSELECT 1 AS order_id\n"
    ),
    "sql/models/order_report.sql": (
        'MODEL (description "Test model order_report.", materialized table, schema prod);\n\n'
        'SELECT order_id FROM __ref("order_ledger")\n'
    ),
    "sql/models/order_archive.sql": (
        'MODEL (description "Test model order_archive.", materialized table, schema sales);\n\n'
        "SELECT 1 AS order_id\n"
    ),
}
_PRE_MIGRATION_NOTE: str = "deferred to pre-migration name prod.order_history"


@pytest.mark.parametrize(
    "test_case",
    [
        DeferredMigrationOriginE2ETestCase(
            description="rename not yet deployed reads the pre-migration relation",
            prod_sql="CREATE TABLE prod.order_history AS SELECT 7 AS order_id;",
            expected_order_ids=(7,),
            expect_note=True,
        ),
        DeferredMigrationOriginE2ETestCase(
            description="deployed rename reads the current relation",
            prod_sql=(
                "CREATE TABLE prod.order_history AS SELECT 7 AS order_id;"
                "CREATE TABLE prod.order_ledger AS SELECT 8 AS order_id;"
            ),
            expected_order_ids=(8,),
            expect_note=False,
        ),
        DeferredMigrationOriginE2ETestCase(
            description="bare pre-migration name resolves in the model's own deferred schema",
            prod_sql=(
                "CREATE SCHEMA sales;"
                "CREATE TABLE prod.order_history AS SELECT 7 AS order_id;"
                "CREATE TABLE sales.order_history AS SELECT 5 AS order_id;"
            ),
            expected_order_ids=(7,),
            expect_note=True,
        ),
        DeferredMigrationOriginE2ETestCase(
            description="current name built only in another deferred schema is not the model",
            prod_sql=(
                "CREATE SCHEMA sales;"
                "CREATE TABLE prod.order_history AS SELECT 7 AS order_id;"
                "CREATE TABLE sales.order_ledger AS SELECT 9 AS order_id;"
            ),
            expected_order_ids=(7,),
            expect_note=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_renamed_upstream_when_building_with_defer_to_then_reads_the_relation_prod_has(
    test_case: DeferredMigrationOriginE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="deferred_renames", repo_files=_PROJECT_FILES
    )
    database: Path = project_dir / _DATABASE
    execute_duckdb(db_path=database, sql="CREATE SCHEMA prod;" + test_case.prod_sql)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--select", "order_report", "--defer-to", "prod"),
        project_dir=project_dir,
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 0, output
    assert (_PRE_MIGRATION_NOTE in output) is test_case.expect_note, output
    assert (
        tuple(
            row[0]
            for row in query_duckdb(db_path=database, sql="SELECT order_id FROM dev.order_report")
        )
        == test_case.expected_order_ids
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
