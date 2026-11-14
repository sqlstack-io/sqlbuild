from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.contract.models import (
    ColumnInfo,
    RelationInfo,
    RelationRowCountEstimate,
    RowDiffResult,
    RowDiffSampleRow,
)
from sqlbuild.adapters.sqlite.classes.sqlite_adapter import SqliteAdapter
from tests.integration.src.sqlbuild.adapters.sqlite._test_types import (
    SqliteAtomicityTestCase,
    SqliteCastTestCase,
    SqliteConnectErrorTestCase,
    SqliteCursorRowCountTestCase,
    SqliteDiffTestCase,
    SqliteNamespaceListingTestCase,
    SqlitePragmaTestCase,
    SqliteQualifiedNameTestCase,
    SqliteRejectedNameTestCase,
    SqliteSchemaEvolutionTestCase,
    SqliteSeedTestCase,
    SqliteStateTableTestCase,
    SqliteStatisticsTestCase,
    SqliteStrategyTestCase,
    SqliteSwapTestCase,
    SqliteTypedTableTestCase,
    SqliteVersionTestCase,
    SqliteViewRenameTestCase,
)
from tests.integration.src.sqlbuild.adapters.sqlite.helpers import (
    changed_column_names,
    create_orders,
    evolve_orders,
    fetch_rows,
    pragma_values,
    recorder,
    render_relations,
    run_sql,
    write_incrementally,
)


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteQualifiedNameTestCase(
            description="main schema relations keep their bare name",
            schema="main",
            name="orders",
            expected_name="orders",
        ),
        SqliteQualifiedNameTestCase(
            description="other schemas prefix the relation with schema and two underscores",
            schema="analytics",
            name="orders",
            expected_name="analytics__orders",
        ),
        SqliteQualifiedNameTestCase(
            description="no schema leaves the relation unqualified",
            schema=None,
            name="orders",
            expected_name=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_schema_when_rendering_qualified_name_then_sqlite_maps_to_one_file(
    test_case: SqliteQualifiedNameTestCase, adapter: SqliteAdapter
) -> None:
    rendered: str | None = adapter.render_qualified_name(
        database=None, schema=test_case.schema, name=test_case.name
    )

    assert rendered == test_case.expected_name


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteRejectedNameTestCase(
            description="a database is rejected because one connection is one file",
            database="warehouse",
            schema="analytics",
            expected_code="A703",
        ),
        SqliteRejectedNameTestCase(
            description="a schema containing two underscores is ambiguous",
            database=None,
            schema="sales__eu",
            expected_code="A704",
        ),
        SqliteRejectedNameTestCase(
            description="a schema ending in an underscore is ambiguous",
            database=None,
            schema="sales_",
            expected_code="A704",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unsupported_namespace_when_rendering_name_then_sqlite_raises_coded_error(
    test_case: SqliteRejectedNameTestCase, adapter: SqliteAdapter
) -> None:
    with pytest.raises(AdapterUserError) as raised:
        adapter.render_qualified_name(
            database=test_case.database, schema=test_case.schema, name="orders"
        )

    assert raised.value.code == test_case.expected_code


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteConnectErrorTestCase(
            description="unknown connection keys are rejected",
            config={"path": "shop.sqlite", "database": "shop"},
            expected_code="A703",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_config_when_connecting_then_sqlite_raises_coded_error(
    test_case: SqliteConnectErrorTestCase, adapter: SqliteAdapter
) -> None:
    with pytest.raises(AdapterUserError) as raised:
        adapter.connect(test_case.config)

    assert raised.value.code == test_case.expected_code


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteVersionTestCase(
            description="SQLite older than 3.44 is rejected with a coded error",
            linked_version=(3, 43, 2),
            expected_code="A701",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_old_sqlite_when_connecting_then_sqlite_raises_version_error(
    test_case: SqliteVersionTestCase, adapter: SqliteAdapter, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr("sqlite3.sqlite_version_info", test_case.linked_version)

    with pytest.raises(AdapterUserError) as raised:
        adapter.connect({"path": ":memory:"})

    assert raised.value.code == test_case.expected_code


@pytest.mark.parametrize(
    "test_case",
    [
        SqlitePragmaTestCase(
            description="file connections use WAL, a busy timeout and legacy renames",
            pragmas=("journal_mode", "busy_timeout", "foreign_keys", "legacy_alter_table"),
            expected_values=("wal", 5000, 1, 1),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_file_connection_when_connecting_then_sqlite_enables_wal_and_pragmas(
    test_case: SqlitePragmaTestCase, adapter: SqliteAdapter, connection: Any
) -> None:
    values: tuple[object, ...] = pragma_values(
        adapter=adapter, connection=connection, pragmas=test_case.pragmas
    )

    assert values == test_case.expected_values


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteNamespaceListingTestCase(
            description="lists every schema; tables SQLBuild did not create stay in main",
            schemas=None,
            expected_relations=(
                RelationInfo(None, "main", "daily__totals", "BASE TABLE"),
                RelationInfo(None, "main", "analytics__legacy_orders", "BASE TABLE"),
                RelationInfo(None, "analytics", "orders", "BASE TABLE"),
                RelationInfo(None, "analytics", "orders_view", "VIEW"),
            ),
        ),
        SqliteNamespaceListingTestCase(
            description="filters to one schema",
            schemas=("analytics",),
            expected_relations=(
                RelationInfo(None, "analytics", "orders", "BASE TABLE"),
                RelationInfo(None, "analytics", "orders_view", "VIEW"),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_schemas_when_listing_relations_then_sqlite_reports_logical_names(
    test_case: SqliteNamespaceListingTestCase, adapter: SqliteAdapter, connection: Any
) -> None:
    adapter.ensure_schema(
        connection=connection, database=None, schema="analytics", statement_recorder=recorder()
    )
    render_relations(
        adapter=adapter, relations=(("analytics", "orders"), ("analytics", "orders_view"))
    )
    create_orders(adapter=adapter, connection=connection, relation="analytics__orders")
    run_sql(
        adapter=adapter,
        connection=connection,
        statements=(
            "CREATE VIEW analytics__orders_view AS SELECT * FROM analytics__orders",
            "CREATE TABLE daily__totals (total INTEGER)",
            "CREATE TABLE analytics__legacy_orders (order_id INTEGER)",
        ),
    )

    relations: tuple[RelationInfo, ...] = adapter.list_relations(
        connection=connection, database=None, schemas=test_case.schemas
    )

    assert sorted(relations, key=lambda relation: relation.identity) == sorted(
        test_case.expected_relations, key=lambda relation: relation.identity
    )
    assert adapter.schema_exists(connection=connection, database=None, schema="analytics")
    assert not adapter.schema_exists(connection=connection, database=None, schema="daily")


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteTypedTableTestCase(
            description="passes types through and reports unenforced decimals as numbers",
            select_sql=(
                "SELECT order_id, amount, ordered_at, amount * 2 AS doubled, 1 AS one FROM orders"
            ),
            config={"inferred_column_types": {"doubled": "DECIMAL(12, 2)"}},
            expected_columns=(
                ColumnInfo("order_id", "INTEGER"),
                ColumnInfo("amount", "NUMERIC"),
                ColumnInfo("ordered_at", "TIMESTAMP"),
                ColumnInfo("doubled", "NUMERIC"),
                ColumnInfo("one", "INTEGER"),
            ),
            expected_rows=(
                (1, 12.5, "2026-01-01 10:00:00", 25.0, 1),
                (2, 7.25, "2026-01-02 11:00:00", 14.5, 1),
                (3, 30, "2026-01-03 12:00:00", 60, 1),
            ),
        ),
        SqliteTypedTableTestCase(
            description="enforced decimals are stored as canonical fixed-scale text",
            select_sql=(
                "SELECT CAST(order_id AS BIGINT) AS order_id, "
                "CASE WHEN amount IS NULL THEN NULL ELSE printf('%.2f', amount) END AS amount "
                "FROM orders"
            ),
            config={"column_types": {"order_id": "BIGINT", "amount": "DECIMAL(10,2)"}},
            expected_columns=(
                ColumnInfo("order_id", "BIGINT"),
                ColumnInfo("amount", "DECIMAL(10,2)"),
            ),
            expected_rows=((1, "12.50"), (2, "7.25"), (3, "30.00")),
        ),
        SqliteTypedTableTestCase(
            description="untyped columns stay untyped instead of becoming BLOB",
            setup_statements=(
                "CREATE TABLE order_notes (order_id INTEGER, note)",
                "INSERT INTO order_notes VALUES (1, 'gift')",
            ),
            select_sql="SELECT order_id, note FROM order_notes",
            config={},
            expected_columns=(ColumnInfo("order_id", "INTEGER"), ColumnInfo("note", "TEXT")),
            expected_rows=((1, "gift"),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_query_when_creating_table_then_sqlite_keeps_column_types(
    test_case: SqliteTypedTableTestCase, adapter: SqliteAdapter, connection: Any
) -> None:
    create_orders(adapter=adapter, connection=connection, relation="orders")
    run_sql(adapter=adapter, connection=connection, statements=test_case.setup_statements)

    adapter.create_table_as(
        connection=connection,
        destination="order_copy",
        sql=test_case.select_sql,
        config=test_case.config,
        statement_recorder=recorder(),
    )

    assert (
        adapter.describe_relation(connection=connection, relation='"order_copy"')
        == test_case.expected_columns
    )
    assert (
        fetch_rows(adapter=adapter, connection=connection, sql="SELECT * FROM order_copy")
        == test_case.expected_rows
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteCastTestCase(
            description="decimal targets render canonical fixed-scale text",
            target_type="DECIMAL(10,2)",
            expected_sql=(
                'CASE WHEN "amount" IS NULL THEN NULL '
                'ELSE printf(\'%.2f\', "amount") END AS "amount"'
            ),
        ),
        SqliteCastTestCase(
            description="date and time targets keep their ISO text unchanged",
            target_type="TIMESTAMP",
            expected_sql='"amount" AS "amount"',
        ),
        SqliteCastTestCase(
            description="other targets render a plain cast",
            target_type="INTEGER",
            expected_sql='CAST("amount" AS INTEGER) AS "amount"',
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_contract_type_when_casting_then_sqlite_renders_storage_form(
    test_case: SqliteCastTestCase, adapter: SqliteAdapter
) -> None:
    rendered: str = adapter.render_source_expression_cast(
        expression='"amount"', target_type=test_case.target_type, alias='"amount"'
    )

    assert rendered == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteStrategyTestCase(
            description="append inserts every source row",
            strategy="append",
            source_sql="SELECT 4, 30, 1, '2026-01-04 09:00:00', 'placed'",
            select_columns="order_id",
            expected_rows=((1,), (2,), (3,), (4,)),
        ),
        SqliteStrategyTestCase(
            description="merge updates matched keys and inserts new ones",
            strategy="merge",
            select_columns="order_id, status",
            source_sql=(
                "SELECT 2 AS order_id, 10 AS customer_id, 9 AS amount, "
                "'2026-01-02 11:00:00' AS ordered_at, 'returned' AS status "
                "UNION ALL SELECT 5, 30, 4, '2026-01-05 09:00:00', 'placed'"
            ),
            expected_rows=((1, "placed"), (2, "returned"), (3, "placed"), (5, "placed")),
        ),
        SqliteStrategyTestCase(
            description="delete_insert replaces rows that share a key",
            strategy="delete_insert",
            select_columns="order_id, status",
            source_sql=(
                "SELECT 3 AS order_id, 20 AS customer_id, 31 AS amount, "
                "'2026-01-03 12:00:00' AS ordered_at, 'shipped' AS status"
            ),
            expected_rows=((1, "placed"), (2, "shipped"), (3, "shipped")),
        ),
        SqliteStrategyTestCase(
            description="cursor delete_insert replaces exactly the window",
            strategy="cursor",
            select_columns="order_id, status",
            source_sql=(
                "SELECT 9 AS order_id, 10 AS customer_id, 1 AS amount, "
                "'2026-01-02 12:00:00' AS ordered_at, 'placed' AS status"
            ),
            expected_rows=((1, "placed"), (3, "placed"), (9, "placed")),
        ),
        SqliteStrategyTestCase(
            description="keyed cursor delete_insert also replaces a key whose cursor moved",
            strategy="keyed_cursor",
            select_columns="order_id, status",
            source_sql=(
                "SELECT 1 AS order_id, 10 AS customer_id, 13 AS amount, "
                "'2026-01-02 15:00:00' AS ordered_at, 'shipped' AS status"
            ),
            expected_rows=((1, "shipped"), (3, "placed")),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_strategy_when_writing_incrementally_then_sqlite_produces_expected_rows(
    test_case: SqliteStrategyTestCase, adapter: SqliteAdapter, connection: Any
) -> None:
    create_orders(adapter=adapter, connection=connection, relation="orders")

    write_incrementally(
        adapter=adapter,
        connection=connection,
        strategy=test_case.strategy,
        sql=test_case.source_sql,
    )

    assert (
        fetch_rows(
            adapter=adapter,
            connection=connection,
            sql=f"SELECT {test_case.select_columns} FROM orders ORDER BY order_id",
        )
        == test_case.expected_rows
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteAtomicityTestCase(
            description="a failing delete_insert leaves the target unchanged",
            strategy="delete_insert",
            expected_rows=((1,), (2,), (3,)),
        ),
        SqliteAtomicityTestCase(
            description="a failing cursor window leaves the target unchanged",
            strategy="cursor",
            expected_rows=((1,), (2,), (3,)),
        ),
        SqliteAtomicityTestCase(
            description="a failing keyed cursor window leaves the target unchanged",
            strategy="keyed_cursor",
            expected_rows=((1,), (2,), (3,)),
        ),
        SqliteAtomicityTestCase(
            description="a failing merge leaves the target unchanged",
            strategy="merge",
            expected_rows=((1,), (2,), (3,)),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failing_statement_when_writing_then_sqlite_rolls_back_the_transaction(
    test_case: SqliteAtomicityTestCase, adapter: SqliteAdapter, connection: Any
) -> None:
    create_orders(adapter=adapter, connection=connection, relation="orders")
    failing_sql: str = (
        "SELECT 2 AS order_id, 10 AS customer_id, 1 AS amount, "
        "'2026-01-02 11:00:00' AS ordered_at, 'x' AS status, 'extra' AS unexpected"
    )

    with pytest.raises(Exception, match="column|values"):
        write_incrementally(
            adapter=adapter, connection=connection, strategy=test_case.strategy, sql=failing_sql
        )

    assert not connection.in_transaction()
    assert (
        fetch_rows(
            adapter=adapter, connection=connection, sql="SELECT order_id FROM orders ORDER BY 1"
        )
        == test_case.expected_rows
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteSwapTestCase(
            description="swap keeps dependent views bound by name to the promoted table",
            expected_view_rows=((99,),),
            expected_dependent_views=("analytics.order_ids",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dependent_view_when_swapping_then_sqlite_view_reads_new_table(
    test_case: SqliteSwapTestCase, adapter: SqliteAdapter, connection: Any
) -> None:
    adapter.ensure_schema(
        connection=connection, database=None, schema="analytics", statement_recorder=recorder()
    )
    render_relations(
        adapter=adapter,
        relations=(
            ("analytics", "orders"),
            ("analytics", "orders__staging"),
            ("analytics", "order_ids"),
        ),
    )
    run_sql(
        adapter=adapter,
        connection=connection,
        statements=(
            "CREATE TABLE analytics__orders (order_id INTEGER)",
            "INSERT INTO analytics__orders VALUES (1)",
            "CREATE TABLE analytics__orders__staging (order_id INTEGER)",
            "INSERT INTO analytics__orders__staging VALUES (99)",
            "CREATE VIEW analytics__order_ids AS SELECT order_id FROM analytics__orders",
        ),
    )

    adapter.swap(
        connection=connection,
        left="analytics__orders",
        right="analytics__orders__staging",
        statement_recorder=recorder(),
    )

    assert (
        fetch_rows(adapter=adapter, connection=connection, sql="SELECT * FROM analytics__order_ids")
        == test_case.expected_view_rows
    )
    assert (
        adapter.list_dependent_view_names(
            connection=connection, database=None, schema="analytics", name="orders"
        )
        == test_case.expected_dependent_views
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteSchemaEvolutionTestCase(
            description="adds a nullable column",
            operation="add",
            expected_columns=(
                ColumnInfo("order_id", "INTEGER"),
                ColumnInfo("status", "VARCHAR(16)"),
                ColumnInfo("channel", "TEXT"),
            ),
            expected_rows=((1, "placed", None),),
        ),
        SqliteSchemaEvolutionTestCase(
            description="drops a column a view still references",
            operation="drop",
            expected_columns=(ColumnInfo("order_id", "INTEGER"),),
            expected_rows=((1,),),
        ),
        SqliteSchemaEvolutionTestCase(
            description="renames a column in place",
            operation="rename",
            expected_columns=(
                ColumnInfo("order_id", "INTEGER"),
                ColumnInfo("order_status", "VARCHAR(16)"),
            ),
            expected_rows=((1, "placed"),),
        ),
        SqliteSchemaEvolutionTestCase(
            description="retypes by rebuilding the table with its column order",
            operation="retype",
            expected_columns=(ColumnInfo("order_id", "TEXT"), ColumnInfo("status", "VARCHAR(16)")),
            expected_rows=(("1", "placed"),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_schema_change_when_evolving_table_then_sqlite_keeps_data(
    test_case: SqliteSchemaEvolutionTestCase, adapter: SqliteAdapter, connection: Any
) -> None:
    run_sql(
        adapter=adapter,
        connection=connection,
        statements=(
            "CREATE TABLE orders (order_id INTEGER, status VARCHAR(16))",
            "INSERT INTO orders VALUES (1, 'placed')",
            "CREATE VIEW order_status AS SELECT order_id, status FROM orders",
        ),
    )

    evolve_orders(adapter=adapter, connection=connection, operation=test_case.operation)

    assert (
        adapter.get_columns(connection=connection, database=None, schema="main", name="orders")
        == test_case.expected_columns
    )
    assert (
        fetch_rows(adapter=adapter, connection=connection, sql="SELECT * FROM orders")
        == test_case.expected_rows
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteSeedTestCase(
            description="loads typed rows, booleans as integers and empty cells as null",
            csv_contents=(
                "customer_id,customer_name,is_active,signed_up_at\n"
                "1,Ada,true,2026-01-01 08:00:00\n"
                "2,,false,\n"
            ),
            columns=(
                ColumnInfo("customer_id", "INTEGER"),
                ColumnInfo("customer_name", "VARCHAR"),
                ColumnInfo("is_active", "BOOLEAN"),
                ColumnInfo("signed_up_at", "TIMESTAMP"),
            ),
            expected_rows=((1, "Ada", 1, "2026-01-01 08:00:00"), (2, None, 0, None)),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_seed_csv_when_loading_twice_then_sqlite_replaces_rows(
    test_case: SqliteSeedTestCase, adapter: SqliteAdapter, connection: Any, tmp_path: Path
) -> None:
    seed_path: Path = tmp_path / "customers.csv"
    seed_path.write_text(test_case.csv_contents, encoding="utf-8")

    for _ in range(2):
        adapter.load_seed(
            connection=connection,
            destination="customers",
            file_path=seed_path,
            columns=test_case.columns,
            statement_recorder=recorder(),
        )

    assert (
        fetch_rows(adapter=adapter, connection=connection, sql="SELECT * FROM customers")
        == test_case.expected_rows
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteStateTableTestCase(
            description="state tables and file-unique indexes coexist in two schemas",
            schema="analytics",
            expected_tables=(
                "_sqlbuild_audit_results",
                "_sqlbuild_fingerprints",
                "_sqlbuild_node_results",
                "_sqlbuild_source_freshness",
                "analytics___sqlbuild_audit_results",
                "analytics___sqlbuild_fingerprints",
                "analytics___sqlbuild_node_results",
                "analytics___sqlbuild_source_freshness",
            ),
            expected_index_count=12,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_two_schemas_when_creating_state_tables_then_sqlite_names_indexes_uniquely(
    test_case: SqliteStateTableTestCase, adapter: SqliteAdapter, connection: Any
) -> None:
    schema: str
    for schema in ("main", test_case.schema):
        adapter.ensure_schema(
            connection=connection, database=None, schema=schema, statement_recorder=recorder()
        )
        run_sql(
            adapter=adapter,
            connection=connection,
            statements=(
                adapter.render_create_fingerprint_table_sql(database=None, schema=schema),
                *adapter.render_create_fingerprint_index_sqls(database=None, schema=schema),
                adapter.render_create_source_freshness_table_sql(database=None, schema=schema),
                *adapter.render_create_source_freshness_index_sqls(database=None, schema=schema),
                adapter.render_create_node_result_table_sql(database=None, schema=schema),
                *adapter.render_create_node_result_index_sqls(database=None, schema=schema),
                adapter.render_create_audit_result_table_sql(database=None, schema=schema),
                *adapter.render_create_audit_result_index_sqls(database=None, schema=schema),
                adapter.render_prune_fingerprint_history_sql(
                    database=None, schema=schema, retain_versions=3
                ),
            ),
        )

    tables: tuple[tuple[object, ...], ...] = fetch_rows(
        adapter=adapter,
        connection=connection,
        sql=(
            "SELECT name FROM sqlite_schema WHERE type = 'table' "
            "AND name LIKE '%sqlbuild%' AND name NOT IN ('_sqlbuild_schemas', '_sqlbuild_relations') "
            "ORDER BY name"
        ),
    )
    indexes: tuple[tuple[object, ...], ...] = fetch_rows(
        adapter=adapter,
        connection=connection,
        sql="SELECT COUNT(*) FROM sqlite_schema WHERE type = 'index' AND sql IS NOT NULL",
    )
    assert tuple(row[0] for row in tables) == test_case.expected_tables
    assert indexes == ((test_case.expected_index_count,),)


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteDiffTestCase(
            description="counts equal, changed and side-only rows with full outer join",
            right_sql=(
                "SELECT order_id, customer_id, amount, ordered_at, "
                "CASE WHEN order_id = 2 THEN 'returned' ELSE status END AS status FROM orders "
                "WHERE order_id <> 3 UNION ALL SELECT 4, 40, 1, '2026-01-04 00:00:00', 'placed'"
            ),
            expected_counts=(3, 3, 1, 1, 1),
            expected_changed_columns=("status",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_two_relations_when_diffing_then_sqlite_reports_row_differences(
    test_case: SqliteDiffTestCase, adapter: SqliteAdapter, connection: Any
) -> None:
    create_orders(adapter=adapter, connection=connection, relation="orders")
    adapter.create_table_as(
        connection=connection,
        destination="orders_next",
        sql=test_case.right_sql,
        statement_recorder=recorder(),
    )

    result: RowDiffResult = adapter.diff_rows(
        connection=connection, left="orders", right="orders_next", unique_key="order_id"
    )
    samples: tuple[RowDiffSampleRow, ...] = adapter.sample_unequal_rows(
        connection=connection, left="orders", right="orders_next", unique_key="order_id"
    )
    unkeyed: RowDiffResult = adapter.diff_unkeyed_rows(
        connection=connection, left="orders", right="orders_next"
    )

    assert (
        result.left_count,
        result.right_count,
        result.equal_count,
        result.unequal_count,
        result.left_only_count,
    ) == test_case.expected_counts
    assert changed_column_names(samples) == test_case.expected_changed_columns
    assert unkeyed.equal_count == 1


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteViewRenameTestCase(
            description="recreates the view under its new name from its stored body",
            expected_definition="SELECT order_id FROM orders",
            expected_names=("orders", "orders_archive"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_view_when_renaming_then_sqlite_recreates_it(
    test_case: SqliteViewRenameTestCase, adapter: SqliteAdapter, connection: Any
) -> None:
    create_orders(adapter=adapter, connection=connection, relation="orders")
    adapter.create_view_as(
        connection=connection,
        destination="orders_view",
        sql="SELECT order_id FROM orders",
        statement_recorder=recorder(),
    )

    adapter.rename_view(
        connection=connection,
        origin="orders_view",
        destination="orders_archive",
        statement_recorder=recorder(),
    )

    assert (
        adapter.read_view_definition(
            connection=connection, database=None, schema="main", name="orders_archive"
        )
        == test_case.expected_definition
    )
    assert (
        tuple(
            relation.name
            for relation in adapter.list_relations(
                connection=connection, database=None, schemas=None
            )
        )
        == test_case.expected_names
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteStatisticsTestCase(
            description="background metadata avoids scans while interactive counts remain exact",
            fixture_name="sqlite",
            expected_interactive_row_count=3,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sqlite_rows_when_sampling_metadata_then_unknown_without_scanning(
    test_case: SqliteStatisticsTestCase, adapter: SqliteAdapter, connection: Any
) -> None:
    fixture: Any = json.loads(
        (
            Path(__file__).resolve().parents[6]
            / "benchmarks/duckshop/edge_cases/orchestration/warehouse_statistics.json"
        ).read_text()
    )[test_case.fixture_name]
    run_sql(adapter=adapter, connection=connection, statements=tuple(fixture["setup_sql"]))
    statements: list[str] = []
    connection.raw_connection.set_trace_callback(statements.append)

    metadata: RelationRowCountEstimate = adapter.estimate_relation_row_count(
        connection=connection,
        database=None,
        schema="main",
        name=fixture["name"],
        metadata_only=True,
    )

    assert metadata.row_count is None
    assert metadata.detail == fixture["metadata_detail"]
    assert metadata.byte_count is None
    assert metadata.byte_count_basis is None
    assert statements == []

    interactive: RelationRowCountEstimate = adapter.estimate_relation_row_count(
        connection=connection, database=None, schema="main", name=fixture["name"]
    )

    assert interactive.row_count == test_case.expected_interactive_row_count
    assert interactive.detail is None


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteCursorRowCountTestCase(
            description="a keyed window reports the insert's rows, not its staging drop",
            unique_key=("order_id",),
            expected_inserted=1,
        ),
        SqliteCursorRowCountTestCase(
            description="an unkeyed window reports the insert's rows",
            unique_key=(),
            expected_inserted=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cursor_window_when_writing_then_sqlite_reports_inserted_rows(
    test_case: SqliteCursorRowCountTestCase, adapter: SqliteAdapter, connection: Any
) -> None:
    create_orders(adapter=adapter, connection=connection, relation="orders")

    inserted: int | None = adapter.delete_insert_cursor(
        connection=connection,
        destination="orders",
        sql=(
            "SELECT 1 AS order_id, 10 AS customer_id, 13 AS amount, "
            "'2026-01-02 15:00:00' AS ordered_at, 'shipped' AS status"
        ),
        cursor_column="ordered_at",
        cursor_start="2026-01-02 00:00:00",
        cursor_end="2026-01-03 00:00:00",
        cursor_type="timestamp",
        statement_recorder=StatementRecorder(),
        unique_key=test_case.unique_key,
    )

    assert inserted == test_case.expected_inserted
