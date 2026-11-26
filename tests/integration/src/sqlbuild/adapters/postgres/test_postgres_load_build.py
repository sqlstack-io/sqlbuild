from __future__ import annotations

import base64
import json
from pathlib import Path
from textwrap import dedent
from typing import Any

import duckdb
import pytest

from sqlbuild._native import run_cli
from sqlbuild.adapters.postgres.classes.postgres_adapter import PostgresAdapter
from tests.integration.src.sqlbuild.adapters.postgres._test_types import (
    PostgresLoadBuildTestCase,
)

_SOURCES: str = """load:
  name: shop
  kind: postgres
  connection: shop_db
  include: ["{schema}.*"]
  write_strategy: replace
sources:
  - name: shop__orders
    description: Orders
"""
_ORDER_NAMES: str = """MODEL (
  materialized table,
  description "Order names"
);

select id, name from __source("shop__orders")
"""
_CUSTOMER_NAMES: str = """MODEL (
  materialized table,
  description "Customer names"
);

select id, name from __source("shop__customers")
"""


@pytest.mark.parametrize(
    "test_case",
    [
        PostgresLoadBuildTestCase(
            description="every table the pattern matches loads, and models read their sources",
            tables=(
                ("orders", ((1, "first order"), (2, "second order"))),
                ("customers", ((7, "a customer"),)),
                ("refunds", ((9, "a refund"),)),
            ),
            expected_orders=((1, "first order"), (2, "second order")),
            expected_customers=((7, "a customer"),),
            expected_loaded=("shop__customers", "shop__orders", "shop__refunds"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_postgres_load_with_a_pattern_when_building_then_its_tables_feed_the_models(
    test_case: PostgresLoadBuildTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    adapter: PostgresAdapter,
    connection: Any,
    postgres_schema: str,
    postgres_container_config: dict[str, object],
) -> None:
    for table, rows in test_case.tables:
        adapter.execute(
            connection=connection,
            sql=f"CREATE TABLE {postgres_schema}.{table} (id integer primary key, name text)",
        )
        values: str = ", ".join(f"({row_id}, '{name}')" for row_id, name in rows)
        adapter.execute(
            connection=connection, sql=f"INSERT INTO {postgres_schema}.{table} VALUES {values}"
        )
    monkeypatch.setenv("SHOP_DB_PASSWORD", str(postgres_container_config["password"]))
    project_dir: Path = tmp_path / "project"
    (project_dir / "sql" / "sources").mkdir(parents=True)
    (project_dir / "sql" / "models").mkdir(parents=True)
    (project_dir / "sqlbuild_project.toml").write_text(
        dedent(
            f"""
            name = "shop_project"
            adapter = "duckdb"
            default_target = "dev"

            [connections.local]
            database = "shop.duckdb"

            [connections.shop_db]
            kind = "postgres"
            host = "{postgres_container_config["host"]}"
            port = {postgres_container_config["port"]}
            user = "{postgres_container_config["user"]}"
            database = "{postgres_container_config["dbname"]}"
            password = "${{ENV:SHOP_DB_PASSWORD}}"

            [targets.dev]
            connection = "local"
            schema = "dev"
            loader_schema = "raw"
            """
        ).lstrip()
    )
    (project_dir / "sql" / "sources" / "shop.yml").write_text(
        _SOURCES.format(schema=postgres_schema)
    )
    (project_dir / "sql" / "models" / "order_names.sql").write_text(_ORDER_NAMES)
    (project_dir / "sql" / "models" / "customer_names.sql").write_text(_CUSTOMER_NAMES)

    exit_code: int = run_cli(["--project-dir", str(project_dir), "--no-color", "build"])

    warehouse: duckdb.DuckDBPyConnection = duckdb.connect(str(project_dir / "shop.duckdb"))
    try:
        orders: list[tuple[Any, ...]] = warehouse.sql(
            "SELECT id, name FROM dev.order_names ORDER BY id"
        ).fetchall()
        customers: list[tuple[Any, ...]] = warehouse.sql(
            "SELECT id, name FROM dev.customer_names ORDER BY id"
        ).fetchall()
        reports: list[tuple[Any, ...]] = warehouse.sql(
            "SELECT payload_json_b64 FROM dev._sqlbuild_node_results WHERE node_type = 'load'"
        ).fetchall()
    finally:
        warehouse.close()
    loaded: tuple[str, ...] = tuple(
        sorted(
            table["table"].rsplit(".", 1)[-1]
            for table in json.loads(base64.b64decode(reports[0][0]))
        )
    )

    assert exit_code == 0, test_case.description
    assert tuple(orders) == test_case.expected_orders, test_case.description
    assert tuple(customers) == test_case.expected_customers, test_case.description
    assert loaded == test_case.expected_loaded, test_case.description
