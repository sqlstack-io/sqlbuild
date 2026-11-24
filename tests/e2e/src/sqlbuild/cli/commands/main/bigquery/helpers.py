"""Helpers for BigQuery CLI e2e tests."""

from __future__ import annotations

from pathlib import Path
from shutil import copytree
from typing import Any

from sqlbuild.adapters.bigquery.classes.bigquery_adapter import BigQueryAdapter
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    WAFFLE_SHOP_DIR,
    prepare_source_loader_strategies,
    stringify_warehouse_rows,
)
from tests.integration.src.sqlbuild.adapters.bigquery.helpers import (
    build_bigquery_connection_config,
    build_unique_dataset_name,
    fetch_rows,
    qualified_name,
)


def build_bigquery_local_config(*, environment: str = "dev", location: str) -> str:
    """Build a local config pointing the project at BigQuery."""

    return (
        'adapter = "bigquery"\n'
        f'target = "{environment}"\n\n'
        "[connection]\n"
        'project = "${ENV:SQB_TEST_BIGQUERY_PROJECT}"\n'
        f'location = "{location}"\n'
    )


def build_bigquery_project_toml(*, project_name: str, dataset_name: str) -> str:
    """Build project TOML for an inline BigQuery e2e project."""

    project_id: str = str(build_bigquery_connection_config(schema=dataset_name)["project"])
    location: str = str(build_bigquery_connection_config(schema=dataset_name)["location"])
    return (
        f'name = "{project_name}"\n'
        'adapter = "bigquery"\n'
        'default_target = "dev"\n\n'
        "[connection]\n"
        'project = "${ENV:SQB_TEST_BIGQUERY_PROJECT}"\n'
        f'location = "{location}"\n\n'
        "[targets.dev]\n"
        f'database = "{project_id}"\n'
        f'schema = "{dataset_name}"\n'
        'defer_sources_to = "dev"\n\n'
        "[defaults]\n"
        'materialized = "table"\n'
    )


def build_bigquery_source_deferral_project_toml(
    *, project_name: str, dev_dataset_name: str, prod_dataset_name: str
) -> str:
    project_id: str = str(build_bigquery_connection_config(schema=dev_dataset_name)["project"])
    location: str = str(build_bigquery_connection_config(schema=dev_dataset_name)["location"])
    return (
        f'name = "{project_name}"\n'
        'adapter = "bigquery"\n'
        'default_target = "dev"\n\n'
        "[connection]\n"
        'project = "${ENV:SQB_TEST_BIGQUERY_PROJECT}"\n'
        f'location = "{location}"\n\n'
        "[targets.dev]\n"
        f'database = "{project_id}"\n'
        f'schema = "{dev_dataset_name}"\n'
        'defer_sources_to = "prod"\n\n'
        "[targets.prod]\n"
        f'database = "{project_id}"\n'
        f'schema = "{prod_dataset_name}"\n\n'
        "[defaults]\n"
        'materialized = "table"\n'
    )


def prepare_bigquery_waffle_shop(*, tmp_path: Path) -> tuple[Path, str]:
    """Prepare the e2e waffle shop project wired to a unique BigQuery dataset."""

    project_dir: Path = tmp_path / "waffle_shop"
    copytree(WAFFLE_SHOP_DIR, project_dir)
    dataset_name: str = build_unique_dataset_name(prefix="sqlbuild_e2e")
    project_name: str = str(build_bigquery_connection_config(schema=dataset_name)["project"])
    location: str = str(build_bigquery_connection_config(schema=dataset_name)["location"])
    project_contents: str = (
        'name = "waffle_shop"\n'
        'adapter = "bigquery"\n'
        'default_target = "dev"\n\n'
        "[connection]\n"
        'project = "${ENV:SQB_TEST_BIGQUERY_PROJECT}"\n'
        f'location = "{location}"\n\n'
        "[settings]\n"
        'default_audit_severity = "warn"\n\n'
        "[defaults]\n"
        'materialized = "table"\n\n'
        "[targets.dev]\n"
        f'database = "{project_name}"\n'
        f'schema = "{dataset_name}"\n'
        'defer_sources_to = "dev"\n\n'
        "[targets.prod]\n"
        f'database = "{project_name}"\n'
        f'schema = "{dataset_name}"\n\n'
        "[path_defaults.staging]\n"
        'materialized = "view"\n'
    )
    (project_dir / "sqlbuild_project.toml").write_text(project_contents, encoding="utf-8")
    (project_dir / "sqlbuild_local.toml").write_text(
        build_bigquery_local_config(location=location),
        encoding="utf-8",
    )
    return project_dir, dataset_name


def prepare_bigquery_source_loader_strategies(*, tmp_path: Path) -> tuple[Path, str]:
    """Prepare source-loader strategy fixture wired to a unique BigQuery dataset."""

    dataset_name: str = build_unique_dataset_name(prefix="sqlbuild_e2e_load")
    project_dir: Path = prepare_source_loader_strategies(
        tmp_path=tmp_path,
        project_toml=build_bigquery_project_toml(
            project_name="source_loader_strategies",
            dataset_name=dataset_name,
        ),
    )
    return project_dir, dataset_name


def cleanup_bigquery_dataset(*, dataset_name: str) -> None:
    """Drop the generated BigQuery dataset after a test completes."""

    adapter: BigQueryAdapter = BigQueryAdapter()
    config: dict[str, object] = build_bigquery_connection_config(schema=dataset_name)
    project_name: str = str(config["project"])
    connection: Any = adapter.connect(config)
    try:
        connection.client.delete_dataset(
            f"{project_name}.{dataset_name}",
            delete_contents=True,
            not_found_ok=True,
        )
    finally:
        adapter.close(connection)


def ensure_bigquery_dataset_ready(*, dataset_name: str) -> None:
    """Precreate the dataset for commands that write directly into it."""

    adapter: BigQueryAdapter = BigQueryAdapter()
    config: dict[str, object] = build_bigquery_connection_config(schema=dataset_name)
    project_name: str = str(config["project"])
    connection: Any = adapter.connect(config)
    try:
        from google.cloud import bigquery

        dataset: Any = bigquery.Dataset(f"{project_name}.{dataset_name}")
        dataset.location = connection.location
        connection.client.create_dataset(dataset, exists_ok=True)
    finally:
        adapter.close(connection)


def fetch_bigquery_rows(*, dataset_name: str, sql: str) -> tuple[tuple[object, ...], ...]:
    """Fetch rows from BigQuery using the configured test credentials."""

    adapter: BigQueryAdapter = BigQueryAdapter()
    config: dict[str, object] = build_bigquery_connection_config(schema=dataset_name)
    connection: Any = adapter.connect(config)
    try:
        return fetch_rows(adapter=adapter, connection=connection, sql=sql)
    finally:
        adapter.close(connection)


def list_bigquery_scenario_relation_names(*, dataset_name: str) -> tuple[str, ...]:
    """Return scenario artifact relation names in a BigQuery dataset."""

    project_name: str = str(build_bigquery_connection_config(schema=dataset_name)["project"])
    rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
        dataset_name=dataset_name,
        sql=(
            f"SELECT table_name FROM `{project_name}.{dataset_name}.INFORMATION_SCHEMA.TABLES` "
            "WHERE table_name LIKE '__sqb_%' ORDER BY table_name"
        ),
    )
    return tuple(str(row[0]) for row in rows)


def bigquery_relation_row_count(*, dataset_name: str, relation: str) -> int:
    """Return row count for one BigQuery relation."""

    rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
        dataset_name=dataset_name,
        sql=f"SELECT COUNT(*) FROM {relation_name(dataset_name=dataset_name, name=relation)}",
    )
    return int(str(rows[0][0]))


def relation_name(*, dataset_name: str, name: str) -> str:
    """Return a fully qualified relation name for a BigQuery e2e dataset."""

    project_name: str = str(build_bigquery_connection_config(schema=dataset_name)["project"])
    return qualified_name(project=project_name, dataset=dataset_name, name=name)


def prepare_bigquery_query_source(*, dataset_name: str) -> str:
    """Create a simple source table used by query CLI tests."""

    table_name: str = relation_name(dataset_name=dataset_name, name="query_source")
    execute_bigquery_sql(
        dataset_name=dataset_name,
        sql=(
            f"CREATE OR REPLACE TABLE {table_name} AS "
            "SELECT 1 AS id, 'alice' AS name UNION ALL SELECT 2 AS id, 'bob' AS name"
        ),
    )
    return table_name


def prepare_bigquery_diff_project(*, tmp_path: Path) -> tuple[Path, str, str]:
    """Prepare a BigQuery-backed diff project with explicit prod/dev target datasets."""

    project_dir: Path = tmp_path / "bigquery_diff_project"
    prod_dataset: str = build_unique_dataset_name(prefix="sqlbuild_diff_prod")
    dev_dataset: str = build_unique_dataset_name(prefix="sqlbuild_diff_dev")
    project_name: str = str(build_bigquery_connection_config(schema=dev_dataset)["project"])
    location: str = str(build_bigquery_connection_config(schema=dev_dataset)["location"])
    project_contents: str = (
        'name = "bigquery_diff_project"\n'
        'adapter = "bigquery"\n'
        'default_target = "dev"\n\n'
        "[connection]\n"
        'project = "${ENV:SQB_TEST_BIGQUERY_PROJECT}"\n'
        f'location = "{location}"\n\n'
        "[targets.dev]\n"
        f'database = "{project_name}"\n'
        f'schema = "{dev_dataset}"\n\n'
        "[targets.dev.clone]\n"
        "allow_as_clone_origin = true\n"
        "allow_as_clone_destination = true\n\n"
        "[targets.prod]\n"
        f'database = "{project_name}"\n'
        f'schema = "{prod_dataset}"\n\n'
        "[targets.prod.clone]\n"
        "allow_as_clone_origin = true\n"
        "allow_as_clone_destination = false\n\n"
        "[defaults]\n"
        'materialized = "table"\n'
    )
    project_dir.mkdir(parents=True, exist_ok=True)
    (project_dir / "sqlbuild_project.toml").write_text(project_contents, encoding="utf-8")
    (project_dir / "sqlbuild_local.toml").write_text(
        build_bigquery_local_config(location=location),
        encoding="utf-8",
    )
    models_dir: Path = project_dir / "sql" / "models" / "staging"
    models_dir.mkdir(parents=True, exist_ok=True)
    (models_dir / "stg_orders.sql").write_text(
        "MODEL (description 'Test model stg_orders.',\n"
        "  materialized table,\n"
        "  unique_key [order_id],\n"
        "  cursor order_id,\n"
        "  cursor_type integer\n"
        ");\n\n"
        "SELECT * FROM ("
        "SELECT 1 AS order_id, 1 AS customer_id, 100 AS amount_cents UNION ALL "
        "SELECT 2 AS order_id, 2 AS customer_id, 200 AS amount_cents"
        ")",
        encoding="utf-8",
    )
    return project_dir, prod_dataset, dev_dataset


def execute_bigquery_sql(*, dataset_name: str, sql: str) -> None:
    """Execute mutating SQL against a BigQuery dataset."""

    adapter: BigQueryAdapter = BigQueryAdapter()
    config: dict[str, object] = build_bigquery_connection_config(schema=dataset_name)
    connection: Any = adapter.connect(config)
    try:
        ensure_bigquery_dataset_ready(dataset_name=dataset_name)
        adapter.execute(connection=connection, sql=sql)
    finally:
        adapter.close(connection)


def assert_current_bigquery_snapshot_rows(
    *, dataset_name: str, expected_rows: tuple[tuple[object, ...], ...]
) -> None:
    """Assert current snapshot rows for BigQuery real-warehouse e2e tests."""

    rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
        dataset_name=dataset_name,
        sql=(
            "SELECT customer_id, region_id, plan, CAST(effective_from AS DATE), "
            "CAST(effective_to AS DATE) "
            f"FROM {relation_name(dataset_name=dataset_name, name='current_customer_snapshot')} "
            "ORDER BY customer_id, region_id, effective_from"
        ),
    )
    assert stringify_warehouse_rows(rows) == expected_rows


def assert_bigquery_snapshot_matrix_rows(
    *,
    dataset_name: str,
    expected_current_rows: tuple[tuple[object, ...], ...],
    expected_historical_timestamp_rows: tuple[tuple[object, ...], ...],
    expected_historical_check_rows: tuple[tuple[object, ...], ...],
) -> None:
    """Assert all compact snapshot matrix rows for BigQuery."""

    assert_current_bigquery_snapshot_rows(
        dataset_name=dataset_name,
        expected_rows=expected_current_rows,
    )
    historical_timestamp_rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
        dataset_name=dataset_name,
        sql=(
            "SELECT customer_id, plan, CAST(valid_from AS DATE), CAST(valid_to AS DATE) "
            f"FROM {relation_name(dataset_name=dataset_name, name='historical_customer_snapshot')} "
            "ORDER BY customer_id, valid_from"
        ),
    )
    historical_check_rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
        dataset_name=dataset_name,
        sql=(
            "SELECT customer_id, status, CAST(valid_from AS DATE), CAST(valid_to AS DATE) "
            "FROM "
            f"{relation_name(dataset_name=dataset_name, name='historical_membership_snapshot')} "
            "ORDER BY customer_id, valid_from"
        ),
    )
    actual_historical_timestamp_rows: tuple[tuple[object, ...], ...] = stringify_warehouse_rows(
        historical_timestamp_rows
    )
    actual_historical_check_rows: tuple[tuple[object, ...], ...] = stringify_warehouse_rows(
        historical_check_rows
    )
    assert actual_historical_timestamp_rows == expected_historical_timestamp_rows, (
        actual_historical_timestamp_rows
    )
    assert actual_historical_check_rows == expected_historical_check_rows, (
        actual_historical_check_rows
    )


def assert_bigquery_snapshot_apply_rows(
    *,
    dataset_name: str,
    expected_current_check_rows: tuple[tuple[object, ...], ...],
    expected_current_delete_rows: tuple[tuple[object, ...], ...],
    expected_historical_timestamp_rows: tuple[tuple[object, ...], ...],
    expected_historical_check_rows: tuple[tuple[object, ...], ...],
) -> None:
    """Assert existing-target snapshot apply rows for BigQuery."""

    current_check_rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
        dataset_name=dataset_name,
        sql=(
            "SELECT customer_id, status, valid_to IS NULL "
            f"FROM {relation_name(dataset_name=dataset_name, name='current_check_snapshot')} "
            "ORDER BY customer_id, status, valid_to IS NULL"
        ),
    )
    current_delete_rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
        dataset_name=dataset_name,
        sql=(
            "SELECT customer_id, plan, valid_to IS NULL "
            f"FROM {relation_name(dataset_name=dataset_name, name='current_delete_snapshot')} "
            "ORDER BY customer_id, plan, valid_to IS NULL"
        ),
    )
    historical_timestamp_rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
        dataset_name=dataset_name,
        sql=(
            "SELECT customer_id, plan, CAST(valid_from AS DATE), CAST(valid_to AS DATE) "
            "FROM "
            f"{relation_name(dataset_name=dataset_name, name='historical_timestamp_snapshot')} "
            "ORDER BY customer_id, valid_from"
        ),
    )
    historical_check_rows: tuple[tuple[object, ...], ...] = fetch_bigquery_rows(
        dataset_name=dataset_name,
        sql=(
            "SELECT customer_id, status, CAST(valid_from AS DATE), CAST(valid_to AS DATE) "
            f"FROM {relation_name(dataset_name=dataset_name, name='historical_check_snapshot')} "
            "ORDER BY customer_id, valid_from"
        ),
    )
    assert stringify_warehouse_rows(current_check_rows) == expected_current_check_rows
    assert stringify_warehouse_rows(current_delete_rows) == expected_current_delete_rows
    assert stringify_warehouse_rows(historical_timestamp_rows) == expected_historical_timestamp_rows
    assert stringify_warehouse_rows(historical_check_rows) == expected_historical_check_rows


def write_local_environment_override(*, project_dir: Path, environment: str) -> None:
    """Write a local environment override for BigQuery CLI e2e commands."""

    location: str = str(build_bigquery_connection_config()["location"])
    (project_dir / "sqlbuild_local.toml").write_text(
        build_bigquery_local_config(environment=environment, location=location),
        encoding="utf-8",
    )
