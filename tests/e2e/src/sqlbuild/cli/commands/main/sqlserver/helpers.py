from __future__ import annotations

import os
import runpy
import uuid
from pathlib import Path
from typing import Any

from sqlbuild.adapters.sqlserver.classes.sqlserver_adapter import SqlServerAdapter
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    WAFFLE_SHOP_DIR,
    prepare_waffle_shop,
    stringify_warehouse_rows,
)

_WAFFLE_LOADERS_FILE: str = "python/nodes/loaders/waffle_sources/loaders.py"


def build_unique_schema_name(*, prefix: str) -> str:
    suffix: str = uuid.uuid4().hex[:10]
    return f"{prefix}_{suffix}"


def build_sqlserver_config() -> dict[str, object]:
    return {
        "host": os.environ.get("SQLBUILD_SQLSERVER_HOST", "localhost"),
        "port": int(os.environ.get("SQLBUILD_SQLSERVER_PORT", "1433")),
        "database": os.environ.get("SQLBUILD_SQLSERVER_DATABASE", "tempdb"),
        "user": os.environ.get("SQLBUILD_SQLSERVER_USER", "sa"),
        "password": os.environ.get("SQLBUILD_SQLSERVER_PASSWORD", "Sqlbuild!2026"),
    }


def build_sqlserver_project_toml(
    *, project_name: str, schema_name: str, config: dict[str, object]
) -> str:
    return (
        f'name = "{project_name}"\n'
        'adapter = "sqlserver"\n'
        'default_target = "dev"\n\n'
        "[connection]\n"
        f'host = "{config["host"]}"\n'
        f"port = {config['port']}\n"
        f'database = "{config["database"]}"\n'
        f'user = "{config["user"]}"\n'
        f'password = "{config["password"]}"\n\n'
        "[targets.dev]\n"
        f'schema = "{schema_name}"\n'
        'defer_sources_to = "dev"\n\n'
        "[defaults]\n"
        'materialized = "table"\n'
    )


def relation_name(*, schema_name: str, name: str) -> str:
    return f"[{schema_name}].[{name}]"


def adapt_sqlserver_project_files(repo_files: dict[str, str]) -> dict[str, str]:
    return {path: adapt_sqlserver_sql(contents) for path, contents in repo_files.items()}


def adapt_sqlserver_sql(sql: str) -> str:
    return (
        sql.replace(" AS TIMESTAMP", " AS DATETIME2")
        .replace("type: TIMESTAMP", "type: DATETIME2")
        .replace("plan", "plan_name")
    )


def adapt_sqlserver_text_file(path: Path) -> None:
    path.write_text(adapt_sqlserver_sql(path.read_text(encoding="utf-8")), encoding="utf-8")


def ensure_sqlserver_schema_ready(*, schema_name: str, config: dict[str, object]) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()
    connection: Any = adapter.connect(config)
    try:
        adapter.execute(
            connection=connection, sql=f"CREATE SCHEMA {adapter.render_identifier(schema_name)}"
        )
    finally:
        adapter.close(connection)


def execute_sqlserver_sql(*, sql: str, config: dict[str, object]) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()
    connection: Any = adapter.connect(config)
    try:
        adapter.execute(connection=connection, sql=sql)
    finally:
        adapter.close(connection)


def cleanup_sqlserver_schema(*, schema_name: str, config: dict[str, object]) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()
    connection: Any = adapter.connect(config)
    escaped_schema_name: str = schema_name.replace("'", "''")
    try:
        adapter.execute(
            connection=connection,
            sql="DECLARE @sql NVARCHAR(MAX) = N''; "
            "SELECT @sql += N'DROP FUNCTION ' + QUOTENAME(s.name) "
            "+ N'.' + QUOTENAME(o.name) + N';' "
            "FROM sys.objects o JOIN sys.schemas s ON o.schema_id = s.schema_id "
            f"WHERE s.name = '{escaped_schema_name}' AND o.type IN ('FN', 'IF', 'TF'); "
            "EXEC sp_executesql @sql;",
        )
        adapter.execute(
            connection=connection,
            sql="DECLARE @sql NVARCHAR(MAX) = N''; "
            "SELECT @sql += N'DROP VIEW ' + QUOTENAME(s.name) + N'.' + QUOTENAME(v.name) + N';' "
            "FROM sys.views v JOIN sys.schemas s ON v.schema_id = s.schema_id "
            f"WHERE s.name = '{escaped_schema_name}'; "
            "EXEC sp_executesql @sql;",
        )
        adapter.execute(
            connection=connection,
            sql="DECLARE @sql NVARCHAR(MAX) = N''; "
            "SELECT @sql += N'DROP TABLE ' + QUOTENAME(s.name) + N'.' + QUOTENAME(t.name) + N';' "
            "FROM sys.tables t JOIN sys.schemas s ON t.schema_id = s.schema_id "
            f"WHERE s.name = '{escaped_schema_name}'; "
            "EXEC sp_executesql @sql;",
        )
        adapter.execute(
            connection=connection,
            sql=f"IF SCHEMA_ID(N'{escaped_schema_name}') IS NOT NULL "
            f"DROP SCHEMA {adapter.render_identifier(schema_name)}",
        )
    finally:
        adapter.close(connection)


def fetch_sqlserver_rows(*, sql: str, config: dict[str, object]) -> tuple[tuple[object, ...], ...]:
    adapter: SqlServerAdapter = SqlServerAdapter()
    connection: Any = adapter.connect(config)
    try:
        cursor: Any = adapter.execute(connection=connection, sql=sql)
        return tuple(tuple(row) for row in cursor.fetchall())
    finally:
        adapter.close(connection)


def assert_current_sqlserver_snapshot_rows_from_case(
    *,
    schema_name: str,
    config: dict[str, object],
    expected_rows: tuple[tuple[object, ...], ...],
) -> None:
    assert_current_sqlserver_snapshot_rows(
        schema_name=schema_name,
        config=config,
        expected_rows=expected_rows,
    )


def assert_current_sqlserver_snapshot_rows(
    *,
    schema_name: str,
    config: dict[str, object],
    expected_rows: tuple[tuple[object, ...], ...],
) -> None:
    rows: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
        sql=(
            "SELECT customer_id, region_id, plan_name, "
            "CAST(effective_from AS DATE), CAST(effective_to AS DATE) "
            f"FROM {relation_name(schema_name=schema_name, name='current_customer_snapshot')} "
            "ORDER BY customer_id, region_id, effective_from"
        ),
        config=config,
    )
    assert stringify_warehouse_rows(rows) == expected_rows


def assert_sqlserver_snapshot_matrix_rows(
    *,
    schema_name: str,
    config: dict[str, object],
    expected_current_rows: tuple[tuple[object, ...], ...],
    expected_historical_timestamp_rows: tuple[tuple[object, ...], ...],
    expected_historical_check_rows: tuple[tuple[object, ...], ...],
) -> None:
    assert_current_sqlserver_snapshot_rows(
        schema_name=schema_name,
        config=config,
        expected_rows=expected_current_rows,
    )
    historical_timestamp_rows: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
        sql=(
            "SELECT customer_id, plan_name, CAST(valid_from AS DATE), CAST(valid_to AS DATE) "
            f"FROM {relation_name(schema_name=schema_name, name='historical_customer_snapshot')} "
            "ORDER BY customer_id, valid_from"
        ),
        config=config,
    )
    historical_check_rows: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
        sql=(
            "SELECT customer_id, status, CAST(valid_from AS DATE), CAST(valid_to AS DATE) "
            f"FROM {relation_name(schema_name=schema_name, name='historical_membership_snapshot')} "
            "ORDER BY customer_id, valid_from"
        ),
        config=config,
    )
    assert stringify_warehouse_rows(historical_timestamp_rows) == expected_historical_timestamp_rows
    assert stringify_warehouse_rows(historical_check_rows) == expected_historical_check_rows


def assert_sqlserver_snapshot_apply_rows(
    *,
    schema_name: str,
    config: dict[str, object],
    expected_current_check_rows: tuple[tuple[object, ...], ...],
    expected_current_delete_rows: tuple[tuple[object, ...], ...],
    expected_historical_timestamp_rows: tuple[tuple[object, ...], ...],
    expected_historical_check_rows: tuple[tuple[object, ...], ...],
) -> None:
    current_check_rows: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
        sql=(
            "SELECT customer_id, status, CASE WHEN valid_to IS NULL THEN 1 ELSE 0 END "
            f"FROM {relation_name(schema_name=schema_name, name='current_check_snapshot')} "
            "ORDER BY customer_id, status, CASE WHEN valid_to IS NULL THEN 1 ELSE 0 END"
        ),
        config=config,
    )
    current_delete_rows: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
        sql=(
            "SELECT customer_id, plan_name, CASE WHEN valid_to IS NULL THEN 1 ELSE 0 END "
            f"FROM {relation_name(schema_name=schema_name, name='current_delete_snapshot')} "
            "ORDER BY customer_id, plan_name, CASE WHEN valid_to IS NULL THEN 1 ELSE 0 END"
        ),
        config=config,
    )
    historical_timestamp_rows: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
        sql=(
            "SELECT customer_id, plan_name, CAST(valid_from AS DATE), CAST(valid_to AS DATE) "
            f"FROM {relation_name(schema_name=schema_name, name='historical_timestamp_snapshot')} "
            "ORDER BY customer_id, valid_from"
        ),
        config=config,
    )
    historical_check_rows: tuple[tuple[object, ...], ...] = fetch_sqlserver_rows(
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


def prepare_sqlserver_waffle_shop(*, tmp_path: Path, config: dict[str, object]) -> tuple[Path, str]:
    schema_name: str = build_unique_schema_name(prefix="sqb_waffle")
    project_dir: Path = prepare_waffle_shop(tmp_path)

    (project_dir / "sql" / "functions" / "sql" / "customer_orders.sql").unlink(missing_ok=True)
    (project_dir / "sql" / "functions" / "python" / "is_completed_order_py.py").unlink(
        missing_ok=True
    )
    (project_dir / "tests" / "unit" / "test_customer_orders_table_fn.sql").unlink(missing_ok=True)
    (project_dir / "tests" / "unit" / "test_fact_orders.sql").unlink(missing_ok=True)
    (project_dir / "sql" / "models" / "marts" / "daily_order_partitioned.sql").unlink(
        missing_ok=True
    )
    (project_dir / "tests" / "unit" / "test_is_completed_order_udf.sql").unlink(missing_ok=True)

    macro_path: Path = project_dir / "sql" / "models" / "marts" / "_macros" / "python" / "time.py"
    _rewrite(
        macro_path,
        (
            '    if ctx.adapter_name == "bigquery":\n'
            '        return f"TIMESTAMP_TRUNC({expression}, {grain.upper()})"\n'
            "    return f\"DATE_TRUNC('{grain}', {expression})\"",
            '    if ctx.adapter_name == "bigquery":\n'
            '        return f"TIMESTAMP_TRUNC({expression}, {grain.upper()})"\n'
            '    if ctx.adapter_name == "sqlserver":\n'
            "        if grain == 'hour':\n"
            '            return f"DATEADD(hour, DATEDIFF(hour, 0, {expression}), 0)"\n'
            "        if grain == 'day':\n"
            '            return f"CAST(CAST({expression} AS DATE) AS DATETIME2)"\n'
            "    return f\"DATE_TRUNC('{grain}', {expression})\"",
        ),
    )
    is_completed_order_path: Path = (
        project_dir / "sql" / "functions" / "sql" / "is_completed_order.sql"
    )
    _rewrite(is_completed_order_path, ("STRING", "NVARCHAR"))
    _rewrite(is_completed_order_path, ("BOOLEAN", "BIT"))
    _rewrite(
        is_completed_order_path,
        (
            "order_status = 'completed'",
            "CAST(CASE WHEN order_status = 'completed' THEN 1 ELSE 0 END AS BIT)",
        ),
    )
    fact_orders_path: Path = project_dir / "sql" / "models" / "marts" / "fact_orders.sql"
    _rewrite(
        fact_orders_path,
        (
            '__udf("is_completed_order_py")(o.status) AS is_completed_order_py,',
            '__udf("is_completed_order")(o.status) AS is_completed_order_py,',
        ),
    )
    project_file_path: Path = project_dir / "sqlbuild_project.toml"
    project_file_path.write_text(
        build_sqlserver_project_toml(
            project_name="waffle_shop",
            schema_name=schema_name,
            config=config,
        )
        + (
            '\n[settings]\ndefault_audit_severity = "warn"\n\n'
            '[path_defaults.staging]\nmaterialized = "view"\n'
        ),
        encoding="utf-8",
    )
    lookups_path: Path = project_dir / "sql" / "seeds" / "lookups.yml"
    _rewrite(lookups_path, ("type: VARCHAR", "type: NVARCHAR(100)"))
    raw_sources_path: Path = project_dir / "sql" / "sources" / "raw.yml"
    adapt_sqlserver_text_file(raw_sources_path)
    _rewrite(
        raw_sources_path,
        (
            "      - name: ordered_at\n        type: DATETIME2",
            "      - name: ordered_at\n        type: DATETIME",
        ),
    )
    raw_sources_text: str = raw_sources_path.read_text(encoding="utf-8")
    raw_sources_path.write_text(
        raw_sources_text.replace("    managed: true\n", f"    schema: {schema_name}\n"),
        encoding="utf-8",
    )
    (project_dir / _WAFFLE_LOADERS_FILE).unlink()
    return project_dir, schema_name


def write_sqlserver_waffle_sources(*, schema_name: str, config: dict[str, object]) -> None:
    """Write the rows the waffle shop's loaders return, since loaders can't write to SQL Server."""

    data: dict[str, Any] = runpy.run_path(str(WAFFLE_SHOP_DIR / _WAFFLE_LOADERS_FILE))
    customers: str = ", ".join(
        f"({row['id']}, '{row['first_name']}', '{row['last_name']}', '{row['email']}', "
        f"'{row['created_at']}')"
        for row in data["CUSTOMERS"]
    )
    orders: str = ", ".join(
        f"({row[0]}, {row[1]}, {row[2]}, {row[3]}, '{row[4]}', '{row[5]}')"
        for row in data["ORDERS"]
    )
    customers_relation: str = relation_name(schema_name=schema_name, name="raw_customers")
    orders_relation: str = relation_name(schema_name=schema_name, name="raw_orders")
    execute_sqlserver_sql(
        sql=(
            f"CREATE TABLE {customers_relation} (id INT, first_name NVARCHAR(100), "
            "last_name NVARCHAR(100), email NVARCHAR(100), created_at DATETIME2); "
            f"INSERT INTO {customers_relation} VALUES {customers}; "
            f"CREATE TABLE {orders_relation} (id INT, customer_id INT, waffle_type_id INT, "
            "quantity INT, ordered_at DATETIME, status NVARCHAR(100)); "
            f"INSERT INTO {orders_relation} VALUES {orders}"
        ),
        config=config,
    )


def _rewrite(path: Path, replacement: tuple[str, str]) -> None:
    """Replace text in a copied fixture file, failing when the fixture no longer contains it."""

    old, new = replacement
    text: str = path.read_text(encoding="utf-8")
    assert old in text, f"{path} no longer contains {old!r}; update the SQL Server fixture"
    path.write_text(text.replace(old, new), encoding="utf-8")
