"""Tests for source reference resolution."""

from __future__ import annotations

import pytest

from sqlbuild._native import (
    CursorBounds,
    Dialect,
    SourceColumnEntry,
    SourceEntry,
    resolve_source_references,
)
from sqlbuild.adapter.contract.models import ColumnInfo
from sqlbuild.adapter.contract.types import CursorKind
from sqlbuild.adapters.bigquery.classes.bigquery_adapter import BigQueryAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.postgres.classes.postgres_adapter import PostgresAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.adapters.sqlserver.classes.sqlserver_adapter import SqlServerAdapter
from tests.unit.src.sqlbuild.compiler.planner._helpers.resolve._test_types import (
    AdapterSourceResolutionTestCase,
    ReservedSourceColumnTestCase,
    SourceResolutionErrorTestCase,
    SourceResolutionTestCase,
)

_ENFORCED_SOURCE: SourceEntry = SourceEntry(
    name="raw_orders",
    database="raw",
    schema="public",
    table="orders",
    type_enforcement=True,
    columns=(
        SourceColumnEntry(name="order_id", type="VARCHAR"),
        SourceColumnEntry(name="status", type="INTEGER"),
    ),
)

_ENFORCED_WAREHOUSE_COLUMNS: dict[str, tuple[ColumnInfo, ...]] = {
    "raw_orders": (
        ColumnInfo(name="order_id", type="VARCHAR"),
        ColumnInfo(name="status", type="VARCHAR"),
        ColumnInfo(name="amount", type="DECIMAL"),
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        SourceResolutionTestCase(
            description="replaces source with qualified name when no enforcement",
            query_sql='SELECT * FROM __source("raw_orders")',
            star_exclude_keyword="EXCLUDE",
            source_map={
                "raw_orders": SourceEntry(
                    name="raw_orders",
                    database="raw",
                    schema="public",
                    table="orders",
                    type_enforcement=False,
                    columns=(
                        SourceColumnEntry(name="order_id", type="VARCHAR"),
                        SourceColumnEntry(name="status", type="INTEGER"),
                    ),
                ),
            },
            source_warehouse_columns=_ENFORCED_WAREHOUSE_COLUMNS,
            expected_sql="SELECT * FROM raw.public.orders",
        ),
        SourceResolutionTestCase(
            description="allows enforced source contract with extra physical columns",
            query_sql='SELECT * FROM __source("raw_orders")',
            star_exclude_keyword="EXCLUDE",
            source_map={
                "raw_orders": SourceEntry(
                    name="raw_orders",
                    database="raw",
                    schema="public",
                    table="orders",
                    contract="enforced",
                    columns=(
                        SourceColumnEntry(name="order_id", type="INTEGER"),
                        SourceColumnEntry(name="status", type="VARCHAR"),
                    ),
                ),
            },
            source_warehouse_columns={
                "raw_orders": (
                    ColumnInfo(name="order_id", type="INT"),
                    ColumnInfo(name="status", type="VARCHAR"),
                    ColumnInfo(name="extra_physical_column", type="BOOLEAN"),
                ),
            },
            expected_sql="SELECT * FROM raw.public.orders",
        ),
        SourceResolutionTestCase(
            description="wraps query expression source as subquery",
            query_sql='SELECT * FROM __source("raw_orders")',
            star_exclude_keyword="EXCLUDE",
            source_map={
                "raw_orders": SourceEntry(
                    name="raw_orders",
                    expression="SELECT 1 AS order_id, 'placed' AS status",
                ),
            },
            source_warehouse_columns={},
            expected_sql="SELECT * FROM (SELECT 1 AS order_id, 'placed' AS status)",
        ),
        SourceResolutionTestCase(
            description="keeps table function expression source unwrapped",
            query_sql='SELECT * FROM __source("raw_orders")',
            star_exclude_keyword="EXCLUDE",
            source_map={"raw_orders": SourceEntry(name="raw_orders", expression="range(3)")},
            source_warehouse_columns={},
            expected_sql="SELECT * FROM range(3)",
        ),
        SourceResolutionTestCase(
            description="casts expression source declared typed columns",
            query_sql='SELECT * FROM __source("raw_orders")',
            star_exclude_keyword="EXCLUDE",
            source_map={
                "raw_orders": SourceEntry(
                    name="raw_orders",
                    expression="SELECT '1' AS order_id, 12 AS amount",
                    type_enforcement=True,
                    columns=(
                        SourceColumnEntry(name="order_id", type="INTEGER"),
                        SourceColumnEntry(name="amount", type="DECIMAL"),
                    ),
                ),
            },
            source_warehouse_columns={
                "raw_orders": (
                    ColumnInfo(name="order_id", type=""),
                    ColumnInfo(name="amount", type=""),
                ),
            },
            expected_sql=(
                'SELECT * FROM (SELECT CAST("order_id" AS INTEGER) AS "order_id", '
                'CAST("amount" AS DECIMAL) AS "amount" '
                "FROM (SELECT '1' AS order_id, 12 AS amount) AS __source_expression)"
            ),
        ),
        SourceResolutionTestCase(
            description="partial expression source enforcement preserves untyped columns",
            query_sql='SELECT * FROM __source("raw_payments")',
            star_exclude_keyword="EXCLUDE",
            source_map={
                "raw_payments": SourceEntry(
                    name="raw_payments",
                    expression="SELECT 1 AS id, '1700' AS amount_cents, 'success' AS status",
                    type_enforcement=True,
                    columns=(SourceColumnEntry(name="amount_cents", type="INTEGER"),),
                ),
            },
            source_warehouse_columns={
                "raw_payments": (
                    ColumnInfo(name="id", type=""),
                    ColumnInfo(name="amount_cents", type=""),
                    ColumnInfo(name="status", type=""),
                ),
            },
            expected_sql=(
                'SELECT * FROM (SELECT "id", '
                'CAST("amount_cents" AS INTEGER) AS "amount_cents", "status" '
                "FROM (SELECT 1 AS id, '1700' AS amount_cents, 'success' AS status) "
                "AS __source_expression)"
            ),
        ),
        SourceResolutionTestCase(
            description="casts expression source columns matched case-insensitively",
            query_sql='SELECT * FROM __source("raw_customers")',
            star_exclude_keyword="EXCLUDE",
            source_map={
                "raw_customers": SourceEntry(
                    name="raw_customers",
                    expression="SELECT 1 AS id, 'Leslie' AS first_name",
                    type_enforcement=True,
                    columns=(
                        SourceColumnEntry(name="id", type="INTEGER"),
                        SourceColumnEntry(name="first_name"),
                    ),
                ),
            },
            source_warehouse_columns={
                "raw_customers": (
                    ColumnInfo(name="ID", type=""),
                    ColumnInfo(name="FIRST_NAME", type=""),
                ),
            },
            expected_sql=(
                'SELECT * FROM (SELECT CAST("ID" AS INTEGER) AS "id", "FIRST_NAME" '
                "FROM (SELECT 1 AS id, 'Leslie' AS first_name) AS __source_expression)"
            ),
        ),
        SourceResolutionTestCase(
            description="replaces source with CAST subquery when enforcement enabled",
            query_sql='SELECT * FROM __source("raw_orders")',
            star_exclude_keyword="EXCLUDE",
            source_map={"raw_orders": _ENFORCED_SOURCE},
            source_warehouse_columns=_ENFORCED_WAREHOUSE_COLUMNS,
            expected_sql=(
                'SELECT * FROM (SELECT * EXCLUDE ("order_id", "status"), '
                'CAST("order_id" AS VARCHAR) AS "order_id", '
                'CAST("status" AS INTEGER) AS "status" '
                "FROM raw.public.orders)"
            ),
        ),
        SourceResolutionTestCase(
            description="uses adapter-owned star exclusion keyword",
            query_sql='SELECT * FROM __source("raw_orders")',
            star_exclude_keyword="EXCEPT",
            source_map={"raw_orders": _ENFORCED_SOURCE},
            source_warehouse_columns=_ENFORCED_WAREHOUSE_COLUMNS,
            expected_sql=(
                'SELECT * FROM (SELECT * EXCLUDE ("order_id", "status"), '
                'CAST("order_id" AS VARCHAR) AS "order_id", '
                'CAST("status" AS INTEGER) AS "status" '
                "FROM raw.public.orders)"
            ),
        ),
        SourceResolutionTestCase(
            description="leaves unknown source references unchanged",
            query_sql='SELECT * FROM __source("unknown_source")',
            star_exclude_keyword="EXCLUDE",
            source_map={},
            source_warehouse_columns={},
            expected_sql='SELECT * FROM __source("unknown_source")',
        ),
        SourceResolutionTestCase(
            description="replaces multiple source references in one query",
            query_sql=(
                'SELECT a.*, b.* FROM __source("raw_orders") a '
                'JOIN __source("raw_customers") b ON a.id = b.id'
            ),
            star_exclude_keyword="EXCLUDE",
            source_map={
                "raw_orders": SourceEntry(
                    name="raw_orders", database="raw", schema="public", table="orders"
                ),
                "raw_customers": SourceEntry(
                    name="raw_customers", database="raw", schema="public", table="customers"
                ),
            },
            source_warehouse_columns={},
            expected_sql=(
                "SELECT a.*, b.* FROM raw.public.orders a JOIN raw.public.customers b ON a.id = b.id"
            ),
        ),
        SourceResolutionTestCase(
            description="builds qualified name with database schema and table",
            query_sql='SELECT * FROM __source("full_source")',
            star_exclude_keyword="EXCLUDE",
            source_map={
                "full_source": SourceEntry(
                    name="full_source", database="mydb", schema="myschema", table="mytable"
                ),
            },
            source_warehouse_columns={},
            expected_sql="SELECT * FROM mydb.myschema.mytable",
        ),
        SourceResolutionTestCase(
            description="falls back to source name when table is not set",
            query_sql='SELECT * FROM __source("name_only")',
            star_exclude_keyword="EXCLUDE",
            source_map={"name_only": SourceEntry(name="name_only")},
            source_warehouse_columns={},
            expected_sql="SELECT * FROM name_only",
        ),
        SourceResolutionTestCase(
            description="all columns enforced uses plain SELECT without EXCLUDE",
            query_sql='SELECT * FROM __source("all_enforced")',
            star_exclude_keyword="EXCLUDE",
            source_map={
                "all_enforced": SourceEntry(
                    name="all_enforced",
                    database="raw",
                    schema="public",
                    table="all_cols",
                    type_enforcement=True,
                    columns=(
                        SourceColumnEntry(name="id", type="INTEGER"),
                        SourceColumnEntry(name="name", type="VARCHAR"),
                    ),
                ),
            },
            source_warehouse_columns={
                "all_enforced": (
                    ColumnInfo(name="id", type="BIGINT"),
                    ColumnInfo(name="name", type="TEXT"),
                ),
            },
            expected_sql=(
                "SELECT * FROM (SELECT "
                'CAST("id" AS INTEGER) AS "id", '
                'CAST("name" AS VARCHAR) AS "name" '
                "FROM raw.public.all_cols)"
            ),
        ),
        SourceResolutionTestCase(
            description="uses exclusive lower bound when append cursor is not inclusive",
            query_sql='SELECT * FROM __source("orders")',
            star_exclude_keyword="EXCLUDE",
            source_map={"orders": SourceEntry(name="orders", schema="raw", table="orders")},
            source_warehouse_columns={},
            cursor_bounds=CursorBounds(start="2024-01-15", end="2024-02-01"),
            cursor_inputs={"orders": "event_time"},
            cursor_type=CursorKind.TIMESTAMP,
            lower_bound_inclusive=False,
            expected_sql=(
                "SELECT * FROM (SELECT * FROM raw.orders"
                " WHERE event_time > TIMESTAMP '2024-01-15'"
                " AND event_time < TIMESTAMP '2024-02-01')"
            ),
        ),
        SourceResolutionTestCase(
            description="uses integer cursor literals without quotes for source subqueries",
            query_sql='SELECT * FROM __source("orders")',
            star_exclude_keyword="EXCLUDE",
            source_map={"orders": SourceEntry(name="orders", schema="raw", table="orders")},
            source_warehouse_columns={},
            cursor_bounds=CursorBounds(start="10", end="20"),
            cursor_inputs={"orders": "event_id"},
            cursor_type=CursorKind.INTEGER,
            expected_sql=(
                "SELECT * FROM (SELECT * FROM raw.orders WHERE event_id >= 10 AND event_id < 20)"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_references_when_resolving_then_returns_expected_sql(
    test_case: SourceResolutionTestCase,
) -> None:
    result: str = resolve_source_references(
        query_sql=test_case.query_sql,
        source_map=test_case.source_map,
        source_warehouse_columns=test_case.source_warehouse_columns,
        star_exclude_keyword=test_case.star_exclude_keyword,
        cursor_bounds=test_case.cursor_bounds,
        cursor_filter_inputs=test_case.cursor_inputs,
        dialect=Dialect(DuckDbAdapter()),
        cursor_type=test_case.cursor_type,
        lower_bound_inclusive=test_case.lower_bound_inclusive,
    )

    assert result == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    [
        AdapterSourceResolutionTestCase(
            description="bigquery expression source casts use adapter-normalized types",
            query_sql='SELECT * FROM __source("raw_payments")',
            source_map={
                "raw_payments": SourceEntry(
                    name="raw_payments",
                    expression="SELECT '1700' AS amount_cents, 'success' AS status",
                    type_enforcement=True,
                    columns=(SourceColumnEntry(name="amount_cents", type="INTEGER"),),
                ),
            },
            source_warehouse_columns={
                "raw_payments": (
                    ColumnInfo(name="amount_cents", type=""),
                    ColumnInfo(name="status", type=""),
                ),
            },
            expected_sql_fragment="CAST(`amount_cents` AS INT64) AS `amount_cents`",
            forbidden_sql_fragment="CAST(`amount_cents` AS INTEGER)",
        ),
        AdapterSourceResolutionTestCase(
            description="bigquery relation source casts use adapter-normalized types",
            query_sql='SELECT * FROM __source("raw_customers")',
            source_map={
                "raw_customers": SourceEntry(
                    name="raw_customers",
                    database="project-with-hyphens",
                    schema="raw_dataset",
                    table="customers",
                    type_enforcement=True,
                    columns=(SourceColumnEntry(name="first_name", type="VARCHAR"),),
                ),
            },
            source_warehouse_columns={
                "raw_customers": (
                    ColumnInfo(name="id", type=""),
                    ColumnInfo(name="first_name", type=""),
                ),
            },
            expected_sql_fragment=(
                "CAST(`first_name` AS STRING) AS `first_name` FROM "
                "`project-with-hyphens.raw_dataset.customers`"
            ),
            forbidden_sql_fragment="CAST(`first_name` AS VARCHAR)",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_adapter_specific_source_references_when_resolving_then_returns_adapter_sql(
    test_case: AdapterSourceResolutionTestCase,
) -> None:
    result: str = resolve_source_references(
        query_sql=test_case.query_sql,
        source_map=test_case.source_map,
        source_warehouse_columns=test_case.source_warehouse_columns,
        star_exclude_keyword="EXCEPT",
        cursor_bounds=None,
        cursor_filter_inputs={},
        dialect=Dialect(BigQueryAdapter()),
        cursor_type=None,
        lower_bound_inclusive=True,
    )

    assert test_case.expected_sql_fragment in result
    assert test_case.forbidden_sql_fragment not in result


@pytest.mark.parametrize(
    "test_case",
    (
        ReservedSourceColumnTestCase(
            description="snowflake contract projection quotes reserved source column",
            expected_sql=(
                'SELECT order_id FROM (SELECT CAST("ORDER_ID" AS INTEGER) AS "ORDER_ID", '
                'CAST("TABLE" AS VARCHAR) AS "TABLE" FROM raw.public.orders)'
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_reserved_source_column_when_resolving_then_quotes_contract_projection(
    test_case: ReservedSourceColumnTestCase,
) -> None:
    result: str = resolve_source_references(
        query_sql='SELECT order_id FROM __source("raw_orders")',
        source_map={
            "raw_orders": SourceEntry(
                name="raw_orders",
                database="raw",
                schema="public",
                table="orders",
                type_enforcement=True,
                columns=(
                    SourceColumnEntry(name="order_id", type="INTEGER"),
                    SourceColumnEntry(name="table", type="VARCHAR"),
                ),
            )
        },
        source_warehouse_columns={
            "raw_orders": (
                ColumnInfo(name="order_id", type="VARCHAR"),
                ColumnInfo(name="table", type="VARCHAR"),
            )
        },
        star_exclude_keyword="EXCLUDE",
        cursor_bounds=None,
        cursor_filter_inputs={},
        dialect=Dialect(SnowflakeAdapter()),
        cursor_type=None,
        lower_bound_inclusive=True,
    )

    assert result == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    [
        SourceResolutionTestCase(
            description="adds internal alias for type-enforced source without user alias",
            query_sql='SELECT * FROM __source("raw_orders") WHERE status = 1',
            star_exclude_keyword="EXCEPT",
            source_map={"raw_orders": _ENFORCED_SOURCE},
            source_warehouse_columns=_ENFORCED_WAREHOUSE_COLUMNS,
            expected_sql=(
                "SELECT * FROM (SELECT [amount], "
                "CAST([order_id] AS VARCHAR) AS [order_id], "
                "CAST([status] AS INTEGER) AS [status] "
                "FROM raw.public.orders) AS __sqb_source_raw_orders WHERE status = 1"
            ),
        ),
        SourceResolutionTestCase(
            description="preserves implicit user alias for type-enforced source",
            query_sql='SELECT o.order_id FROM __source("raw_orders") o WHERE o.status = 1',
            star_exclude_keyword="EXCEPT",
            source_map={"raw_orders": _ENFORCED_SOURCE},
            source_warehouse_columns=_ENFORCED_WAREHOUSE_COLUMNS,
            expected_sql=(
                "SELECT o.order_id FROM (SELECT [amount], "
                "CAST([order_id] AS VARCHAR) AS [order_id], "
                "CAST([status] AS INTEGER) AS [status] "
                "FROM raw.public.orders) AS o WHERE o.status = 1"
            ),
        ),
        SourceResolutionTestCase(
            description="preserves explicit as user alias for type-enforced source",
            query_sql='SELECT o.order_id FROM __source("raw_orders") AS o WHERE o.status = 1',
            star_exclude_keyword="EXCEPT",
            source_map={"raw_orders": _ENFORCED_SOURCE},
            source_warehouse_columns=_ENFORCED_WAREHOUSE_COLUMNS,
            expected_sql=(
                "SELECT o.order_id FROM (SELECT [amount], "
                "CAST([order_id] AS VARCHAR) AS [order_id], "
                "CAST([status] AS INTEGER) AS [status] "
                "FROM raw.public.orders) AS o WHERE o.status = 1"
            ),
        ),
        SourceResolutionTestCase(
            description="preserves multiple user aliases in joins",
            query_sql=(
                'SELECT o.order_id, c.customer_id FROM __source("raw_orders") o '
                'JOIN __source("raw_customers") c ON o.customer_id = c.customer_id'
            ),
            star_exclude_keyword="EXCEPT",
            source_map={
                "raw_orders": _ENFORCED_SOURCE,
                "raw_customers": SourceEntry(
                    name="raw_customers",
                    database="raw",
                    schema="public",
                    table="customers",
                    type_enforcement=True,
                    columns=(SourceColumnEntry(name="customer_id", type="INTEGER"),),
                ),
            },
            source_warehouse_columns={
                **_ENFORCED_WAREHOUSE_COLUMNS,
                "raw_customers": (
                    ColumnInfo(name="customer_id", type="VARCHAR"),
                    ColumnInfo(name="email", type="VARCHAR"),
                ),
            },
            expected_sql=(
                "SELECT o.order_id, c.customer_id FROM (SELECT [amount], "
                "CAST([order_id] AS VARCHAR) AS [order_id], "
                "CAST([status] AS INTEGER) AS [status] FROM raw.public.orders) AS o "
                "JOIN (SELECT [email], CAST([customer_id] AS INTEGER) AS [customer_id] "
                "FROM raw.public.customers) AS c ON o.customer_id = c.customer_id"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sqlserver_alias_required_sources_when_resolving_then_aliases_derived_tables(
    test_case: SourceResolutionTestCase,
) -> None:
    result: str = resolve_source_references(
        query_sql=test_case.query_sql,
        source_map=test_case.source_map,
        source_warehouse_columns=test_case.source_warehouse_columns,
        star_exclude_keyword=test_case.star_exclude_keyword,
        cursor_bounds=test_case.cursor_bounds,
        cursor_filter_inputs=test_case.cursor_inputs,
        dialect=Dialect(SqlServerAdapter()),
        cursor_type=test_case.cursor_type,
        lower_bound_inclusive=test_case.lower_bound_inclusive,
    )

    assert result == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    [
        SourceResolutionTestCase(
            description="casts sqlserver exclusive timestamp bounds to datetime2",
            query_sql='SELECT * FROM __source("orders")',
            star_exclude_keyword="EXCEPT",
            source_map={"orders": SourceEntry(name="orders", schema="raw", table="orders")},
            source_warehouse_columns={},
            cursor_bounds=CursorBounds(
                start="2026-04-04T14:30:00.000001",
                end="2026-04-05T14:30:00.000001",
            ),
            cursor_inputs={"orders": "ordered_at"},
            cursor_type=CursorKind.TIMESTAMP,
            lower_bound_inclusive=False,
            expected_sql=(
                "SELECT * FROM (SELECT * FROM raw.orders WHERE ordered_at > "
                "CAST('2026-04-04T14:30:00.000001' AS DATETIME2(6)) AND ordered_at < "
                "CAST('2026-04-05T14:30:00.000001' AS DATETIME2(6))) "
                "AS __sqb_source_orders"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sqlserver_timestamp_bounds_when_resolving_then_uses_datetime2_literals(
    test_case: SourceResolutionTestCase,
) -> None:
    result: str = resolve_source_references(
        query_sql=test_case.query_sql,
        source_map=test_case.source_map,
        source_warehouse_columns=test_case.source_warehouse_columns,
        star_exclude_keyword=test_case.star_exclude_keyword,
        cursor_bounds=test_case.cursor_bounds,
        cursor_filter_inputs=test_case.cursor_inputs,
        dialect=Dialect(SqlServerAdapter()),
        cursor_type=test_case.cursor_type,
        lower_bound_inclusive=test_case.lower_bound_inclusive,
    )

    assert result == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    [
        SourceResolutionTestCase(
            description="renders partial source casts without unsupported star exclusion",
            query_sql='SELECT order_id, status, amount FROM __source("raw_orders")',
            star_exclude_keyword="EXCLUDE",
            source_map={"raw_orders": _ENFORCED_SOURCE},
            source_warehouse_columns=_ENFORCED_WAREHOUSE_COLUMNS,
            expected_sql=(
                'SELECT order_id, status, amount FROM (SELECT "amount", '
                'CAST("order_id" AS VARCHAR) AS "order_id", '
                'CAST("status" AS INTEGER) AS "status" '
                "FROM raw.public.orders)"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_postgres_sources_when_resolving_then_avoids_unsupported_star_exclusion(
    test_case: SourceResolutionTestCase,
) -> None:
    result: str = resolve_source_references(
        query_sql=test_case.query_sql,
        source_map=test_case.source_map,
        source_warehouse_columns=test_case.source_warehouse_columns,
        star_exclude_keyword=test_case.star_exclude_keyword,
        cursor_bounds=test_case.cursor_bounds,
        cursor_filter_inputs=test_case.cursor_inputs,
        dialect=Dialect(PostgresAdapter()),
        cursor_type=test_case.cursor_type,
        lower_bound_inclusive=test_case.lower_bound_inclusive,
    )

    assert result == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    [
        SourceResolutionErrorTestCase(
            description="raises when enforced source declares missing warehouse columns",
            query_sql='SELECT * FROM __source("partial_source")',
            star_exclude_keyword="EXCLUDE",
            source_map={
                "partial_source": SourceEntry(
                    name="partial_source",
                    database="raw",
                    schema="public",
                    table="partial",
                    type_enforcement=True,
                    columns=(
                        SourceColumnEntry(name="order_id", type="VARCHAR"),
                        SourceColumnEntry(name="missing_col", type="INTEGER"),
                    ),
                ),
            },
            source_warehouse_columns={
                "partial_source": (
                    ColumnInfo(name="order_id", type="VARCHAR"),
                    ColumnInfo(name="amount", type="DECIMAL"),
                ),
            },
            expected_error_fragment=(
                "source raw.public.partial declares columns not found in warehouse: missing_col"
            ),
        ),
        SourceResolutionErrorTestCase(
            description="raises when untyped relation source metadata has missing columns",
            query_sql='SELECT * FROM __source("partial_source")',
            star_exclude_keyword="EXCLUDE",
            source_map={
                "partial_source": SourceEntry(
                    name="partial_source",
                    database="raw",
                    schema="public",
                    table="partial",
                    columns=(
                        SourceColumnEntry(name="order_id"),
                        SourceColumnEntry(name="missing_col"),
                    ),
                ),
            },
            source_warehouse_columns={
                "partial_source": (
                    ColumnInfo(name="order_id", type="VARCHAR"),
                    ColumnInfo(name="amount", type="DECIMAL"),
                ),
            },
            expected_error_fragment=(
                "source raw.public.partial declares columns not found in warehouse: missing_col"
            ),
        ),
        SourceResolutionErrorTestCase(
            description="raises when enforced source contract type mismatches warehouse",
            query_sql='SELECT * FROM __source("raw_orders")',
            star_exclude_keyword="EXCLUDE",
            source_map={
                "raw_orders": SourceEntry(
                    name="raw_orders",
                    database="raw",
                    schema="public",
                    table="orders",
                    contract="enforced",
                    columns=(SourceColumnEntry(name="order_id", type="INTEGER"),),
                ),
            },
            source_warehouse_columns={
                "raw_orders": (ColumnInfo(name="order_id", type="VARCHAR"),),
            },
            expected_error_fragment=(
                "source raw.public.orders column 'order_id' has type VARCHAR "
                "but contract declares INTEGER"
            ),
        ),
        SourceResolutionErrorTestCase(
            description="raises when expression source typed column is missing from query output",
            query_sql='SELECT * FROM __source("raw_payments")',
            star_exclude_keyword="EXCLUDE",
            source_map={
                "raw_payments": SourceEntry(
                    name="raw_payments",
                    expression="SELECT 1 AS id, 'success' AS status",
                    type_enforcement=True,
                    columns=(SourceColumnEntry(name="amount_cents", type="INTEGER"),),
                ),
            },
            source_warehouse_columns={
                "raw_payments": (
                    ColumnInfo(name="id", type=""),
                    ColumnInfo(name="status", type=""),
                ),
            },
            expected_error_fragment=(
                "source expression 'raw_payments' declares columns not found in query output: "
                "amount_cents. Available query output columns: id, status"
            ),
        ),
        SourceResolutionErrorTestCase(
            description="raises when expression source untyped column is missing from query output",
            query_sql='SELECT * FROM __source("raw_payments")',
            star_exclude_keyword="EXCLUDE",
            source_map={
                "raw_payments": SourceEntry(
                    name="raw_payments",
                    expression="SELECT 1 AS id, 1700 AS amount_cents",
                    type_enforcement=True,
                    columns=(
                        SourceColumnEntry(name="amount_cents", type="INTEGER"),
                        SourceColumnEntry(name="status"),
                    ),
                ),
            },
            source_warehouse_columns={
                "raw_payments": (
                    ColumnInfo(name="id", type=""),
                    ColumnInfo(name="amount_cents", type=""),
                ),
            },
            expected_error_fragment=(
                "source expression 'raw_payments' declares columns not found in query output: "
                "status. Available query output columns: id, amount_cents"
            ),
        ),
        SourceResolutionErrorTestCase(
            description="raises when expression source enforcement lacks query output metadata",
            query_sql='SELECT * FROM __source("raw_payments")',
            star_exclude_keyword="EXCLUDE",
            source_map={
                "raw_payments": SourceEntry(
                    name="raw_payments",
                    expression="SELECT 1 AS amount_cents",
                    type_enforcement=True,
                    columns=(SourceColumnEntry(name="amount_cents", type="INTEGER"),),
                ),
            },
            source_warehouse_columns={},
            expected_error_fragment=(
                "source expression 'raw_payments' type enforcement requires "
                "query output column metadata"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_source_references_when_resolving_then_raises_clear_error(
    test_case: SourceResolutionErrorTestCase,
) -> None:
    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        resolve_source_references(
            query_sql=test_case.query_sql,
            source_map=test_case.source_map,
            source_warehouse_columns=test_case.source_warehouse_columns,
            star_exclude_keyword=test_case.star_exclude_keyword,
            cursor_bounds=None,
            cursor_filter_inputs={},
            dialect=Dialect(DuckDbAdapter()),
            cursor_type=None,
            lower_bound_inclusive=True,
        )
