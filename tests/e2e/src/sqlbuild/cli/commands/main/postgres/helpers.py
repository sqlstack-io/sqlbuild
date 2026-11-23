"""Helpers for Postgres CLI e2e tests."""

from __future__ import annotations

import subprocess
import sys
import uuid
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

from sqlbuild.adapters.postgres.classes.postgres_adapter import PostgresAdapter
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    prepare_source_loader_strategies,
    prepare_waffle_shop,
    run_sqb,
    stringify_warehouse_rows,
)


def build_unique_schema_name(*, prefix: str) -> str:
    suffix: str = uuid.uuid4().hex[:10]
    return f"{prefix}_{suffix}"


def postgres_dbt_core_executable() -> str:
    """Return a dbt-core executable for Postgres dbt tests, skipping otherwise.

    dbt Fusion does not support the Postgres adapter, so these tests must pin to
    the dbt-core CLI installed in the project virtual environment rather than
    honoring any DBT_EXECUTABLE override that may point at Fusion.
    """

    candidate: Path = Path(sys.prefix) / "bin" / "dbt"
    candidate_actions: dict[bool, Callable[[Path], str]] = {
        False: _skip_missing_postgres_dbt_core,
        True: _validate_postgres_dbt_core,
    }
    return candidate_actions[candidate.exists()](candidate)


def _skip_missing_postgres_dbt_core(candidate: Path) -> str:
    del candidate
    pytest.skip("dbt-core CLI is not installed in the project virtual environment")


def _validate_postgres_dbt_core(candidate: Path) -> str:
    result: subprocess.CompletedProcess[str] = subprocess.run(
        (str(candidate), "--version"),
        capture_output=True,
        check=False,
        text=True,
    )
    output: str = result.stdout + result.stderr
    validation_actions: dict[tuple[bool, bool], Callable[[Path, str], str]] = {
        (False, False): _return_postgres_dbt_core,
        (False, True): _skip_postgres_dbt_fusion,
        (True, False): _skip_unrunnable_postgres_dbt_core,
        (True, True): _skip_unrunnable_postgres_dbt_core,
    }
    return validation_actions[(result.returncode != 0, "fusion" in output.lower())](
        candidate, output
    )


def _return_postgres_dbt_core(candidate: Path, output: str) -> str:
    del output
    return str(candidate)


def _skip_postgres_dbt_fusion(candidate: Path, output: str) -> str:
    del candidate, output
    pytest.skip("project virtual environment dbt resolves to Fusion, which lacks Postgres")


def _skip_unrunnable_postgres_dbt_core(candidate: Path, output: str) -> str:
    del candidate
    pytest.skip(f"dbt-core CLI is not runnable: {output}")


def postgres_dbt_env(*, password: str) -> dict[str, str]:
    """Return the subprocess env for Postgres dbt tests pinned to dbt-core."""

    return {
        "DBT_POSTGRES_PASSWORD": password,
        "DBT_EXECUTABLE": postgres_dbt_core_executable(),
    }


def build_postgres_project_toml(
    *,
    project_name: str,
    schema_name: str,
    config: dict[str, object],
) -> str:
    return (
        f'name = "{project_name}"\n'
        'adapter = "postgres"\n'
        'default_target = "dev"\n\n'
        "[connection]\n"
        f'host = "{config["host"]}"\n'
        f"port = {config['port']}\n"
        f'dbname = "{config["dbname"]}"\n'
        f'user = "{config["user"]}"\n'
        f'password = "{config["password"]}"\n\n'
        "[targets.dev]\n"
        f'schema = "{schema_name}"\n'
        'defer_sources_to = "dev"\n\n'
        "[defaults]\n"
        'materialized = "table"\n'
    )


def build_postgres_source_deferral_project_toml(
    *, project_name: str, dev_schema_name: str, prod_schema_name: str, config: dict[str, object]
) -> str:
    return (
        f'name = "{project_name}"\n'
        'adapter = "postgres"\n'
        'default_target = "dev"\n\n'
        "[connection]\n"
        f'host = "{config["host"]}"\n'
        f"port = {config['port']}\n"
        f'dbname = "{config["dbname"]}"\n'
        f'user = "{config["user"]}"\n'
        f'password = "{config["password"]}"\n\n'
        "[targets.dev]\n"
        f'schema = "{dev_schema_name}"\n'
        'defer_sources_to = "prod"\n\n'
        "[targets.prod]\n"
        f'schema = "{prod_schema_name}"\n\n'
        "[defaults]\n"
        'materialized = "table"\n'
    )


def ensure_postgres_schema_ready(*, schema_name: str, config: dict[str, object]) -> None:
    adapter: PostgresAdapter = PostgresAdapter()
    connection: Any = adapter.connect(config)
    try:
        adapter.execute(connection=connection, sql=f"CREATE SCHEMA IF NOT EXISTS {schema_name}")
    finally:
        adapter.close(connection)


def cleanup_postgres_schema(*, schema_name: str, config: dict[str, object]) -> None:
    adapter: PostgresAdapter = PostgresAdapter()
    connection: Any = adapter.connect(config)
    try:
        adapter.execute(connection=connection, sql=f"DROP SCHEMA IF EXISTS {schema_name} CASCADE")
    finally:
        adapter.close(connection)


def fetch_postgres_rows(*, sql: str, config: dict[str, object]) -> tuple[tuple[object, ...], ...]:
    adapter: PostgresAdapter = PostgresAdapter()
    connection: Any = adapter.connect(config)
    try:
        cursor: Any = adapter.execute(connection=connection, sql=sql)
        return tuple(tuple(row) for row in cursor.fetchall())
    finally:
        adapter.close(connection)


def execute_postgres_sql(*, sql: str, config: dict[str, object]) -> None:
    adapter: PostgresAdapter = PostgresAdapter()
    connection: Any = adapter.connect(config)
    try:
        adapter.execute(connection=connection, sql=sql)
    finally:
        adapter.close(connection)


def relation_name(*, schema_name: str, name: str) -> str:
    return f"{schema_name}.{name}"


def assert_current_postgres_snapshot_rows_from_case(
    *,
    schema_name: str,
    config: dict[str, object],
    expected_rows: tuple[tuple[object, ...], ...],
) -> None:
    assert_current_postgres_snapshot_rows(
        schema_name=schema_name,
        config=config,
        expected_rows=expected_rows,
    )


def assert_current_postgres_snapshot_rows(
    *,
    schema_name: str,
    config: dict[str, object],
    expected_rows: tuple[tuple[object, ...], ...],
) -> None:
    rows: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
        sql=(
            "SELECT customer_id, region_id, plan, "
            "CAST(effective_from AS DATE), CAST(effective_to AS DATE) "
            f"FROM {relation_name(schema_name=schema_name, name='current_customer_snapshot')} "
            "ORDER BY customer_id, region_id, effective_from"
        ),
        config=config,
    )
    assert stringify_warehouse_rows(rows) == expected_rows


def assert_postgres_snapshot_matrix_rows(
    *,
    schema_name: str,
    config: dict[str, object],
    expected_current_rows: tuple[tuple[object, ...], ...],
    expected_historical_timestamp_rows: tuple[tuple[object, ...], ...],
    expected_historical_check_rows: tuple[tuple[object, ...], ...],
) -> None:
    assert_current_postgres_snapshot_rows(
        schema_name=schema_name,
        config=config,
        expected_rows=expected_current_rows,
    )
    historical_timestamp_rows: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
        sql=(
            "SELECT customer_id, plan, CAST(valid_from AS DATE), CAST(valid_to AS DATE) "
            f"FROM {relation_name(schema_name=schema_name, name='historical_customer_snapshot')} "
            "ORDER BY customer_id, valid_from"
        ),
        config=config,
    )
    historical_check_rows: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
        sql=(
            "SELECT customer_id, status, CAST(valid_from AS DATE), CAST(valid_to AS DATE) "
            f"FROM {relation_name(schema_name=schema_name, name='historical_membership_snapshot')} "
            "ORDER BY customer_id, valid_from"
        ),
        config=config,
    )
    assert stringify_warehouse_rows(historical_timestamp_rows) == expected_historical_timestamp_rows
    assert stringify_warehouse_rows(historical_check_rows) == expected_historical_check_rows


def assert_postgres_snapshot_apply_rows(
    *,
    schema_name: str,
    config: dict[str, object],
    expected_current_check_rows: tuple[tuple[object, ...], ...],
    expected_current_delete_rows: tuple[tuple[object, ...], ...],
    expected_historical_timestamp_rows: tuple[tuple[object, ...], ...],
    expected_historical_check_rows: tuple[tuple[object, ...], ...],
) -> None:
    current_check_rows: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
        sql=(
            "SELECT customer_id, status, valid_to IS NULL "
            f"FROM {relation_name(schema_name=schema_name, name='current_check_snapshot')} "
            "ORDER BY customer_id, status, valid_to IS NULL"
        ),
        config=config,
    )
    current_delete_rows: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
        sql=(
            "SELECT customer_id, plan, valid_to IS NULL "
            f"FROM {relation_name(schema_name=schema_name, name='current_delete_snapshot')} "
            "ORDER BY customer_id, plan, valid_to IS NULL"
        ),
        config=config,
    )
    historical_timestamp_rows: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
        sql=(
            "SELECT customer_id, plan, CAST(valid_from AS DATE), CAST(valid_to AS DATE) "
            f"FROM {relation_name(schema_name=schema_name, name='historical_timestamp_snapshot')} "
            "ORDER BY customer_id, valid_from"
        ),
        config=config,
    )
    historical_check_rows: tuple[tuple[object, ...], ...] = fetch_postgres_rows(
        sql=(
            "SELECT customer_id, status, CAST(valid_from AS DATE), CAST(valid_to AS DATE) "
            f"FROM {relation_name(schema_name=schema_name, name='historical_check_snapshot')} "
            "ORDER BY customer_id, valid_from"
        ),
        config=config,
    )
    assert stringify_warehouse_rows(current_check_rows) == expected_current_check_rows
    assert stringify_warehouse_rows(current_delete_rows) == expected_current_delete_rows
    assert stringify_warehouse_rows(historical_timestamp_rows) == expected_historical_timestamp_rows
    assert stringify_warehouse_rows(historical_check_rows) == expected_historical_check_rows


def prepare_postgres_waffle_shop(*, tmp_path: Path, config: dict[str, object]) -> tuple[Path, str]:
    """Copy waffle shop to tmp dir and wire it to a unique Postgres schema."""

    schema_name: str = build_unique_schema_name(prefix="sqb_waffle")
    project_dir: Path = prepare_waffle_shop(tmp_path)

    (project_dir / "sql" / "functions" / "sql" / "customer_orders.sql").unlink(missing_ok=True)
    (project_dir / "sql" / "functions" / "python" / "nodes" / "is_completed_order_py.py").unlink(missing_ok=True)
    (project_dir / "tests" / "unit" / "test_customer_orders_table_fn.sql").unlink(missing_ok=True)
    (project_dir / "sql" / "models" / "marts" / "daily_order_partitioned.sql").unlink(missing_ok=True)
    (project_dir / "tests" / "unit" / "test_is_completed_order_udf.sql").unlink(missing_ok=True)
    is_completed_order_path: Path = project_dir / "sql" / "functions" / "sql" / "is_completed_order.sql"
    is_completed_order_path.write_text(
        is_completed_order_path.read_text(encoding="utf-8")
        .replace("STRING", "TEXT")
        .replace("order_status = 'completed'", "SELECT order_status = 'completed'"),
        encoding="utf-8",
    )

    fact_orders_path: Path = project_dir / "sql" / "models" / "marts" / "fact_orders.sql"
    fact_orders_path.write_text(
        fact_orders_path.read_text(encoding="utf-8").replace(
            '__udf("is_completed_order_py")(o.status) AS is_completed_order_py,',
            '__udf("is_completed_order")(o.status) AS is_completed_order_py,',
        ),
        encoding="utf-8",
    )
    project_file_path: Path = project_dir / "sqlbuild_project.toml"
    project_file_path.write_text(
        'name = "waffle_shop"\n'
        'adapter = "postgres"\n'
        'default_target = "dev"\n\n'
        "[connection]\n"
        f'host = "{config["host"]}"\n'
        f"port = {config['port']}\n"
        f'dbname = "{config["dbname"]}"\n'
        f'user = "{config["user"]}"\n'
        f'password = "{config["password"]}"\n\n'
        "[settings]\n"
        'default_audit_severity = "warn"\n\n'
        "[defaults]\n"
        'materialized = "table"\n\n'
        "[targets.dev]\n"
        f'schema = "{schema_name}"\n'
        'defer_sources_to = "dev"\n\n'
        "[path_defaults.staging]\n"
        'materialized = "view"\n',
        encoding="utf-8",
    )
    return project_dir, schema_name


def prepare_postgres_source_loader_strategies(
    *, tmp_path: Path, config: dict[str, object]
) -> tuple[Path, str]:
    """Prepare source-loader strategy fixture wired to a unique Postgres schema."""

    schema_name: str = build_unique_schema_name(prefix="sqb_load")
    project_dir: Path = prepare_source_loader_strategies(
        tmp_path=tmp_path,
        project_toml=build_postgres_project_toml(
            project_name="source_loader_strategies",
            schema_name=schema_name,
            config=config,
        ),
    )
    return project_dir, schema_name


def prepare_postgres_dbt_seed_change_workspace(
    *, tmp_path: Path, schema_name: str, config: dict[str, object]
) -> Path:
    """Write a pure dbt seed-backed model chain on a Postgres dbt profile.

    Chain: seed raw_orders -> stg_orders -> fct_orders. Returns the SQLBuild twin dir.
    """

    workspace: Path = tmp_path / "pg_seed_change"
    dbt_project_dir: Path = workspace / "dbt_project"
    profiles_dir: Path = workspace / "profiles"
    sqlbuild_project_dir: Path = workspace / "sqlbuild_project"
    dbt_models_dir: Path = dbt_project_dir / "sql" / "models"
    dbt_seeds_dir: Path = dbt_project_dir / "sql" / "seeds"
    dbt_models_dir.mkdir(parents=True)
    dbt_seeds_dir.mkdir(parents=True)
    profiles_dir.mkdir(parents=True)
    sqlbuild_project_dir.mkdir(parents=True)
    (profiles_dir / "profiles.yml").write_text(
        "analytics:\n"
        "  target: dev\n"
        "  outputs:\n"
        "    dev:\n"
        "      type: postgres\n"
        f"      host: {config['host']}\n"
        f"      port: {config['port']}\n"
        f"      dbname: {config['dbname']}\n"
        f"      user: {config['user']}\n"
        "      pass: \"{{ env_var('DBT_POSTGRES_PASSWORD') }}\"\n"
        f"      schema: {schema_name}\n",
        encoding="utf-8",
    )
    (dbt_project_dir / "dbt_project.yml").write_text(
        "name: analytics\n"
        "version: '1.0'\n"
        "profile: analytics\n"
        "model-paths: ['models']\n"
        "seed-paths: ['seeds']\n"
        "models:\n"
        "  analytics:\n"
        "    +materialized: table\n",
        encoding="utf-8",
    )
    (dbt_seeds_dir / "raw_orders.csv").write_text(
        "order_id,amount\n1,25\n2,20\n3,30\n", encoding="utf-8"
    )
    (dbt_models_dir / "stg_orders.sql").write_text(
        "select order_id, amount from {{ ref('raw_orders') }}\n", encoding="utf-8"
    )
    (dbt_models_dir / "fct_orders.sql").write_text(
        "select count(*) as order_count, sum(amount) as total_amount "
        "from {{ ref('stg_orders') }}\n",
        encoding="utf-8",
    )
    (sqlbuild_project_dir / "sqlbuild_project.toml").write_text(
        'name = "pg_seed_change"\n'
        'adapter = "postgres"\n'
        'default_target = "dev"\n'
        "[connection]\n"
        f'host = "{config["host"]}"\n'
        f"port = {config['port']}\n"
        f'dbname = "{config["dbname"]}"\n'
        f'user = "{config["user"]}"\n'
        f'password = "{config["password"]}"\n'
        "[targets.dev]\n"
        f'schema = "{schema_name}"\n'
        "[dbt]\n"
        'project_dir = "../dbt_project"\n'
        'profiles_dir = "../profiles"\n'
        'target_path = "../dbt_project/target"\n',
        encoding="utf-8",
    )
    return sqlbuild_project_dir


def append_postgres_dbt_seed_change_order(*, project_dir: Path, order_id: int, amount: int) -> None:
    """Append one row to the Postgres seed-change raw_orders seed."""

    seed_path: Path = project_dir.parent / "dbt_project" / "sql" / "seeds" / "raw_orders.csv"
    seed_path.write_text(
        seed_path.read_text(encoding="utf-8") + f"{order_id},{amount}\n",
        encoding="utf-8",
    )


ORIGIN_MODEL: str = "stg_orders"
DESTINATION_MODEL: str = "stg_customer_orders"
_JANITOR_CONFIG: str = "\n[janitor]\nenabled = true\narchive_retention_days = 0\n"
_MIGRATION_TEXT_COLUMNS: tuple[str, ...] = (
    "event_id",
    "target_name",
    "origin_model",
    "origin_database",
    "origin_schema",
    "origin_name",
    "destination_model",
    "destination_database",
    "destination_schema",
    "destination_name",
    "origin_version_hash",
    "discovery",
    "decision",
    "run_id",
)


def orders_sql(*, migrate_from: str = "", where: str = "") -> str:
    """Return an incremental orders model with an optional forced migration header."""

    migration: str = {"": ""}.get(
        migrate_from, f'  migrate_from "{migrate_from}",\n  migrate_force true,\n'
    )
    return (
        "MODEL (description 'Test model.',\n"
        "  materialized incremental,\n"
        "  incremental_strategy delete_insert,\n"
        "  unique_key order_id,\n"
        "  cursor order_date,\n"
        "  cursor_type timestamp,\n"
        "  cursor_grain day,\n"
        '  cursor_start "2026-01-01",\n'
        f"{migration}"
        ");\n\n"
        f'SELECT order_id, order_date, amount_cents FROM __source("raw_orders"){where}\n'
    )


def report_view_sql() -> str:
    """Return a project view that reads the destination by name."""

    return (
        'MODEL (description "Test model.", '
        f'materialized view);\n\nSELECT order_id FROM __ref("{DESTINATION_MODEL}")\n'
    )


def write_migration_project(
    *,
    tmp_path: Path,
    schema_name: str,
    raw_schema_name: str,
    config: dict[str, object],
    models: dict[str, str],
) -> Path:
    """Write a Postgres project whose sources live outside the managed schema."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": build_postgres_project_toml(
            project_name="postgres_model_migrations", schema_name=schema_name, config=config
        )
        + _JANITOR_CONFIG,
        "sql/sources/raw.yml": (
            "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
            f"    schema: {raw_schema_name}\n"
            "    table: raw_orders\n"
        ),
    }
    project_dir: Path = tmp_path / "postgres_model_migrations"
    models_dir: Path = project_dir / "sql" / "models"
    stale: Path
    for stale in models_dir.glob("*.sql"):
        stale.unlink()
    files.update({f"sql/models/{name}.sql": sql for name, sql in models.items()})
    return prepare_inline_project(
        tmp_path=tmp_path, project_name="postgres_model_migrations", repo_files=files
    )


def load_raw_orders(*, raw_schema_name: str, config: dict[str, object]) -> None:
    """Create five raw orders, one per January day, in the source schema."""

    execute_postgres_sql(sql=f"CREATE SCHEMA IF NOT EXISTS {raw_schema_name}", config=config)
    execute_postgres_sql(
        sql=(
            f"CREATE TABLE {raw_schema_name}.raw_orders AS SELECT i AS order_id, "
            "TIMESTAMP '2026-01-01' + (i - 1) * INTERVAL '1 day' AS order_date, "
            "100 + i AS amount_cents FROM generate_series(1, 5) AS t(i)"
        ),
        config=config,
    )


def create_unwritable_migration_table(*, schema_name: str, config: dict[str, object]) -> None:
    """Occupy the migration state name with a read-only view so the event insert fails."""

    columns: str = ", ".join(
        f"CAST(NULL AS TEXT) AS {column}" for column in _MIGRATION_TEXT_COLUMNS
    )
    execute_postgres_sql(
        sql=(
            f"CREATE VIEW {schema_name}._sqlbuild_migrations AS SELECT {columns}, "
            "CAST(NULL AS TIMESTAMP) AS created_at WHERE false"
        ),
        config=config,
    )


def archive_names(
    *, schema_name: str, kind: str, config: dict[str, object]
) -> tuple[tuple[object, ...], ...]:
    """Return every migration archive of one kind in the managed schema."""

    return fetch_postgres_rows(
        sql=(
            "SELECT table_name FROM information_schema.tables "
            f"WHERE table_schema = '{schema_name}' "
            "AND table_name LIKE '\\_sqb\\_archive\\_\\_%' "
            f"AND position('__{kind}__' IN table_name) > 0 ORDER BY 1"
        ),
        config=config,
    )


def ordered_ids(*, relation: str, config: dict[str, object]) -> tuple[tuple[object, ...], ...]:
    """Return the sorted order IDs readable through one relation."""

    return fetch_postgres_rows(sql=f"SELECT order_id FROM {relation} ORDER BY 1", config=config)


def create_external_dependent_views(
    *, raw_schema_name: str, schema_name: str, config: dict[str, object]
) -> None:
    """Create plain, security-option, and check-option views on the destination."""

    destination: str = f"{schema_name}.{DESTINATION_MODEL}"
    statements: tuple[str, ...] = (
        f"CREATE VIEW {raw_schema_name}.orders_dashboard AS SELECT order_id FROM {destination}",
        f"GRANT SELECT ON {raw_schema_name}.orders_dashboard TO PUBLIC",
        (
            f"CREATE VIEW {raw_schema_name}.orders_invoker "
            "WITH (security_barrier=true, security_invoker=true) AS "
            f"SELECT order_id FROM {destination}"
        ),
        (
            f"CREATE VIEW {raw_schema_name}.orders_checked AS "
            f"SELECT order_id, order_date, amount_cents FROM {destination} "
            "WHERE order_id > 0 WITH CASCADED CHECK OPTION"
        ),
    )
    statement: str
    for statement in statements:
        execute_postgres_sql(sql=statement, config=config)


def view_catalog(
    *, raw_schema_name: str, config: dict[str, object]
) -> tuple[tuple[object, ...], ...]:
    """Return (view, options, owner, privileges) for every view in the source schema."""

    return fetch_postgres_rows(
        sql=(
            "SELECT view.relname, coalesce(array_to_string(view.reloptions, ','), ''), "
            "pg_get_userbyid(view.relowner), coalesce(view.relacl::text, '') "
            "FROM pg_class AS view JOIN pg_namespace AS namespace "
            "ON namespace.oid = view.relnamespace "
            f"WHERE namespace.nspname = '{raw_schema_name}' AND view.relkind = 'v' ORDER BY 1"
        ),
        config=config,
    )


FACT_ORDERS_MODEL: str = "fct_orders"
_COLUMN_MIGRATION_TEXT_COLUMNS: tuple[str, ...] = (
    "event_id",
    "target_name",
    "model_name",
    "relation_database",
    "relation_schema",
    "relation_name",
    "origin_column",
    "destination_column",
    "discovery",
    "decision",
    "run_id",
)


def fact_orders_sql(*, amount: str = "amount_cents", columns: str = "") -> str:
    """Return an incremental orders fact model with an optional column migrations block."""

    declarations: str = {"": ""}.get(columns, f"  columns ({columns}),\n")
    return (
        "MODEL (description 'Test model.',\n"
        "  materialized incremental,\n"
        "  incremental_strategy delete_insert,\n"
        "  unique_key order_id,\n"
        "  cursor order_date,\n"
        "  cursor_type timestamp,\n"
        "  cursor_grain day,\n"
        '  cursor_start "2026-01-01",\n'
        "  replay_on_change full,\n"
        f"{declarations}"
        ");\n\n"
        f'SELECT order_id, order_date, {amount} FROM __source("raw_orders")\n'
    )


def fact_order_columns(
    *, schema_name: str, config: dict[str, object]
) -> tuple[tuple[object, ...], ...]:
    """Return the physical column names of the orders fact table in order."""

    return fetch_postgres_rows(
        sql=(
            "SELECT column_name FROM information_schema.columns "
            f"WHERE table_schema = '{schema_name}' AND table_name = '{FACT_ORDERS_MODEL}' "
            "ORDER BY ordinal_position"
        ),
        config=config,
    )


def create_unwritable_column_migration_table(
    *, schema_name: str, config: dict[str, object]
) -> None:
    """Occupy the column migration state name with a read-only view so the insert fails."""

    columns: str = ", ".join(
        f"CAST(NULL AS TEXT) AS {column}" for column in _COLUMN_MIGRATION_TEXT_COLUMNS
    )
    execute_postgres_sql(
        sql=(
            f"CREATE VIEW {schema_name}._sqlbuild_column_migrations AS SELECT {columns}, "
            "CAST(NULL AS TIMESTAMP) AS created_at WHERE false"
        ),
        config=config,
    )


_OLD_NAME_HEADERS: dict[str, str] = {
    "table": "  materialized table,\n",
    "incremental": (
        "  materialized incremental,\n"
        "  incremental_strategy delete_insert,\n"
        "  unique_key order_id,\n"
        "  cursor order_date,\n"
        "  cursor_type timestamp,\n"
        "  cursor_grain day,\n"
        '  cursor_start "2026-01-01",\n'
    ),
    "incremental_sync": (
        "  materialized incremental,\n"
        "  incremental_strategy delete_insert,\n"
        "  unique_key order_id,\n"
        "  cursor order_date,\n"
        "  cursor_type timestamp,\n"
        "  cursor_grain day,\n"
        '  cursor_start "2026-01-01",\n'
        "  on_schema_change sync_all_columns,\n"
    ),
}


def old_name_model_sql(
    *,
    materialized: str,
    migrate_from: str = "",
    columns: str = "order_id, order_date, amount_cents",
) -> str:
    """Return a table or incremental orders model with an optional migrate_from header."""

    migration: str = {"": ""}.get(migrate_from, f"  migrate_from {migrate_from},\n")
    return (
        f"MODEL (description 'Test model.',\n{_OLD_NAME_HEADERS[materialized]}{migration});\n\n"
        f'SELECT {columns} FROM __source("raw_orders")\n'
    )


def build_ok(project_dir: Path) -> subprocess.CompletedProcess[str]:
    """Run sqb build and require success."""

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert result.returncode == 0, result.stdout + result.stderr
    return result


def _crash(connection: Any) -> None:
    """Die like a killed process: the session ends, so nothing after this point runs or commits."""

    connection.close()
    raise RuntimeError("simulated crash")


def _crashing_execute(
    monkeypatch: pytest.MonkeyPatch, *, crashes_at: Callable[[str, list[str]], bool]
) -> None:
    original: Callable[..., Any] = PostgresAdapter.execute
    executed: list[str] = []

    def crash(self: PostgresAdapter, *, connection: Any, sql: str) -> Any:
        del self, sql
        _crash(connection)

    outcomes: dict[bool, Callable[..., Any]] = {True: crash, False: original}

    def execute(self: PostgresAdapter, *, connection: Any, sql: str) -> Any:
        executed.append(sql)
        return outcomes[crashes_at(sql, executed)](self, connection=connection, sql=sql)

    monkeypatch.setattr(PostgresAdapter, "execute", execute)


def _is_second_swap_rename(sql: str, executed: list[str]) -> bool:
    swap_renames: list[str] = [*filter(lambda statement: "__swap_staging" in statement, executed)]
    return swap_renames[1:2] == [sql]


def _is_compatibility_view_rebind(sql: str, executed: list[str]) -> bool:
    del executed
    target: str = sql.split(" AS ")[0].replace('"', "")
    return target.startswith("CREATE OR REPLACE VIEW") and target.endswith(".revenue")


def _is_displaced_drop(sql: str, executed: list[str]) -> bool:
    del executed
    return sql.startswith("DROP TABLE") and "__staging" in sql


def _is_column_drop(sql: str, executed: list[str]) -> bool:
    del executed
    return "DROP COLUMN" in sql


def fail_mid_swap(monkeypatch: pytest.MonkeyPatch) -> None:
    """Crash on the second rename of a swap, after the new relation already holds the name."""

    _crashing_execute(monkeypatch, crashes_at=_is_second_swap_rename)


def fail_bound_view_rebind(monkeypatch: pytest.MonkeyPatch) -> None:
    """Crash while the compatibility view is being pointed at the new relation."""

    _crashing_execute(monkeypatch, crashes_at=_is_compatibility_view_rebind)


def fail_displaced_drop(monkeypatch: pytest.MonkeyPatch) -> None:
    """Crash after compatibility views follow the new relation, before the old one is dropped."""

    _crashing_execute(monkeypatch, crashes_at=_is_displaced_drop)


def fail_column_drop(monkeypatch: pytest.MonkeyPatch) -> None:
    """Crash on the column drop of an incremental schema change."""

    _crashing_execute(monkeypatch, crashes_at=_is_column_drop)


def no_postgres_failure(monkeypatch: pytest.MonkeyPatch) -> None:
    """Install nothing."""

    del monkeypatch


def build_in_process(*, project_dir: Path, capsys: pytest.CaptureFixture[str]) -> int:
    """Run sqb build in-process so faults can be injected, returning its exit code."""

    from sqlbuild._native import run_cli as main

    _ = capsys.readouterr()
    exit_code: int = main(["--project-dir", str(project_dir), "--no-color", "build"])
    output: str = "".join(capsys.readouterr())
    assert exit_code in (0, 1), output
    return exit_code


def reader_rows(
    *, config: dict[str, object], role: str, sql: str
) -> tuple[tuple[object, ...], ...]:
    """Run one query as a login role that holds only the privileges granted to it."""

    reader: dict[str, object] = {**config, "user": role, "password": role}
    return fetch_postgres_rows(sql=sql, config=reader)


def reader_error(*, config: dict[str, object], role: str, sql: str) -> str:
    """Return the error a login role gets for one query, or an empty string."""

    try:
        _ = reader_rows(config=config, role=role, sql=sql)
    except Exception as error:
        return str(error).splitlines()[0]
    return ""


def create_login_role(*, role: str, config: dict[str, object]) -> None:
    """Create a login role whose password is its name, unless it exists."""

    execute_postgres_sql(
        sql=(
            f"DO $$ BEGIN CREATE ROLE {role} LOGIN PASSWORD '{role}'; "
            "EXCEPTION WHEN duplicate_object THEN NULL; END $$"
        ),
        config=config,
    )


def replace_view(*, view: str, sql: str, config: dict[str, object]) -> None:
    """Replace a view with one defined by ``sql``; an empty ``sql`` leaves it alone."""

    statements: dict[bool, tuple[str, ...]] = {
        True: (),
        False: (f"DROP VIEW {view}", f"CREATE VIEW {view} AS {sql}"),
    }
    statement: str
    for statement in statements[not sql]:
        execute_postgres_sql(sql=statement, config=config)


def revoke_view_select(*, view: str, role: str, revoke: bool, config: dict[str, object]) -> None:
    """Revoke a role's SELECT on a view when ``revoke`` is set."""

    statements: dict[bool, tuple[str, ...]] = {
        True: (f"REVOKE SELECT ON {view} FROM {role}",),
        False: (),
    }
    statement: str
    for statement in statements[revoke]:
        execute_postgres_sql(sql=statement, config=config)


def build_long_model_name_project_files(
    *, project_toml: str, schema_name: str, model_name: str, model_columns: str
) -> dict[str, str]:
    """Build a contracted table model whose name nearly fills the identifier limit."""

    return {
        "sqlbuild_project.toml": project_toml + 'contract = "enforced"\n',
        "sql/sources/raw.yml": (
            "sources:\n"
            "  - name: raw_orders\n"
            "    description: Raw orders.\n"
            f"    schema: {schema_name}\n"
            "    table: raw_orders\n"
            "    columns:\n"
            "      - name: id\n"
            "        type: INTEGER\n"
            "      - name: amount\n"
            "        type: INTEGER\n"
        ),
        f"sql/models/{model_name}.sql": (
            "MODEL (\n"
            "  description 'Order totals for a long-named model.',\n"
            "  materialized table,\n"
            "  sql_analysis false,\n"
            f"  columns (\n{model_columns}  ),\n"
            ");\n\n"
            "SELECT CAST(SUM(amount) AS BIGINT) AS total_amount\n"
            'FROM __source("raw_orders")\n'
        ),
        "tests/scenarios/long_name_totals.sql": (
            "SCENARIO (description 'Totals for a long-named model.');\n\n"
            "WITH\n"
            "__source__raw_orders AS (\n"
            "  SELECT 1 AS id, 10 AS amount\n"
            "),\n"
            f"__expected__{model_name} AS (\n"
            "  SELECT 10 AS total_amount\n"
            ")\n"
            "SELECT 1\n"
        ),
    }


def postgres_relation_names(
    *, schema_name: str, name_prefix: str, config: dict[str, object]
) -> tuple[str, ...]:
    """Return the relation names in one Postgres schema that start with a literal prefix."""

    return tuple(
        str(row[0])
        for row in fetch_postgres_rows(
            sql=(
                "SELECT table_name FROM information_schema.tables "
                f"WHERE table_schema = '{schema_name}' "
                f"AND starts_with(table_name, '{name_prefix}') ORDER BY table_name"
            ),
            config=config,
        )
    )
