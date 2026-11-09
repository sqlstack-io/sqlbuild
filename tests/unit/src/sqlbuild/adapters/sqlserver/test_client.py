from __future__ import annotations

from datetime import datetime
from decimal import Decimal

import pytest

from sqlbuild._native import FunctionArgument
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.contract.models import (
    ColumnInfo,
    CursorValue,
    RowDiffTolerance,
    RowDiffTolerances,
)
from sqlbuild.adapter.contract.types import CursorKind
from sqlbuild.adapters.sqlserver.classes.sqlserver_adapter import SqlServerAdapter
from tests.unit.src.sqlbuild.adapters.sqlserver._test_types import (
    SqlServerAdapterDefaultsTestCase,
    SqlServerCursorBoundLiteralTestCase,
    SqlServerDeleteInsertCursorSqlTestCase,
    SqlServerDiffCursorFilterTestCase,
    SqlServerIndexSqlTestCase,
    SqlServerLatestReadSqlTestCase,
    SqlServerMergeExclusionTestCase,
    SqlServerMoveOrCopyRelationTestCase,
    SqlServerPruneSqlTestCase,
    SqlServerRenderCreateFunctionTestCase,
    SqlServerRenderCreateSchemaTestCase,
    SqlServerRenderCreateTableAsTestCase,
    SqlServerRenderIdentifierTestCase,
    SqlServerRenderQualifiedNameTestCase,
    SqlServerRenderRenameTestCase,
    SqlServerRollbackTestCase,
    SqlServerRowDiffEqualitySqlTestCase,
    SqlServerSourceExpressionRelationTestCase,
)
from tests.unit.src.sqlbuild.adapters.sqlserver.helpers import FakeSqlServerConnection


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerAdapterDefaultsTestCase(
            description="returns expected SQL Server adapter defaults and dialect settings",
            expected_default_schema="dbo",
            expected_default_database=None,
            expected_sql_analysis_dialect="tsql",
            expected_identifier_length=128,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sqlserver_adapter_when_checking_defaults_then_returns_expected_values(
    test_case: SqlServerAdapterDefaultsTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    assert adapter.default_schema() == test_case.expected_default_schema
    assert adapter.default_database() == test_case.expected_default_database
    assert adapter.sql_analysis_dialect() == test_case.expected_sql_analysis_dialect
    assert adapter.maximum_identifier_length() == test_case.expected_identifier_length


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerRenderIdentifierTestCase(
            description="quotes identifiers with brackets",
            name="event_id",
            expected_identifier="[event_id]",
        ),
        SqlServerRenderIdentifierTestCase(
            description="escapes embedded closing bracket",
            name="event]id",
            expected_identifier="[event]]id]",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_identifier_when_rendering_then_sqlserver_bracket_quotes_identifier(
    test_case: SqlServerRenderIdentifierTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    identifier: str = adapter.render_identifier(test_case.name)

    assert identifier == test_case.expected_identifier


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerCursorBoundLiteralTestCase(
            description="casts fractional timestamp cursor bounds to datetime2",
            value="2026-04-04T14:30:00.000001",
            cursor_type=CursorKind.TIMESTAMP,
            expected_literal="CAST('2026-04-04T14:30:00.000001' AS DATETIME2(6))",
        ),
        SqlServerCursorBoundLiteralTestCase(
            description="leaves integer cursor bounds unquoted",
            value="42",
            cursor_type=CursorKind.INTEGER,
            expected_literal="42",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cursor_bound_when_rendering_then_sqlserver_uses_compatible_literal(
    test_case: SqlServerCursorBoundLiteralTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    literal: str = adapter.render_cursor_bound_literal(
        value=test_case.value,
        cursor_type=test_case.cursor_type,
    )

    assert literal == test_case.expected_literal


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerDeleteInsertCursorSqlTestCase(
            description="casts timestamp delete bounds to datetime2",
            cursor_column="ordered_at",
            cursor_start="2026-04-04T13:30:00.000001",
            cursor_end="2026-04-04T14:30:00.000001",
            cursor_type=CursorKind.TIMESTAMP,
            expected_statements=(
                "DELETE FROM dbo.raw_orders WHERE [ordered_at] >= "
                "CAST('2026-04-04T13:30:00.000001' AS DATETIME2(6)) AND [ordered_at] < "
                "CAST('2026-04-04T14:30:00.000001' AS DATETIME2(6))",
                "INSERT INTO dbo.raw_orders SELECT * FROM dbo.raw_orders__staging",
            ),
        ),
        SqlServerDeleteInsertCursorSqlTestCase(
            description="leaves integer delete bounds unquoted",
            cursor_column="order_id",
            cursor_start="41",
            cursor_end="43",
            cursor_type=CursorKind.INTEGER,
            expected_statements=(
                "DELETE FROM dbo.raw_orders WHERE [order_id] >= 41 AND [order_id] < 43",
                "INSERT INTO dbo.raw_orders SELECT * FROM dbo.raw_orders__staging",
            ),
        ),
        SqlServerDeleteInsertCursorSqlTestCase(
            description="quotes and escapes unknown delete bounds",
            cursor_column="order_key",
            cursor_start="a'b",
            cursor_end="c'd",
            cursor_type=None,
            expected_statements=(
                "DELETE FROM dbo.raw_orders WHERE [order_key] >= 'a''b' AND [order_key] < 'c''d'",
                "INSERT INTO dbo.raw_orders SELECT * FROM dbo.raw_orders__staging",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cursor_type_when_rendering_delete_insert_then_sqlserver_uses_typed_bounds(
    test_case: SqlServerDeleteInsertCursorSqlTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    statements: tuple[str, ...] = adapter.render_delete_insert_cursor(
        destination="dbo.raw_orders",
        sql="SELECT * FROM dbo.raw_orders__staging",
        cursor_column=test_case.cursor_column,
        cursor_start=test_case.cursor_start,
        cursor_end=test_case.cursor_end,
        cursor_type=test_case.cursor_type,
    )

    assert statements == test_case.expected_statements


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerMergeExclusionTestCase(
            description="sqlserver selective merge",
            expected_update_clause="[status] = __source.[status]",
            expected_insert_clause=(
                "([tenant_id], [order_id], [updated_at], [status]) VALUES "
                "(__source.[tenant_id], __source.[order_id], __source.[updated_at], "
                "__source.[status]);"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_composite_keys_and_case_variant_exclusion_when_rendering_merge_then_sqlserver_updates_only_mutable_columns(  # noqa: E501
    test_case: SqlServerMergeExclusionTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    (statement,) = adapter.render_merge(
        destination="dbo.orders",
        sql="SELECT * FROM staging.orders",
        unique_key=("tenant_id", "order_id"),
        source_columns=("tenant_id", "order_id", "updated_at", "status"),
        exclude_columns=("UPDATED_AT",),
    )

    matched_clause, insert_clause = statement.split(" WHEN NOT MATCHED THEN INSERT ", maxsplit=1)
    update_clause: str = matched_clause.split("WHEN MATCHED THEN UPDATE SET ", maxsplit=1)[1]

    assert "[tenant_id]" not in update_clause
    assert "[order_id]" not in update_clause
    assert "[updated_at]" not in update_clause
    assert update_clause == test_case.expected_update_clause
    assert insert_clause == test_case.expected_insert_clause


@pytest.mark.parametrize(
    "test_case",
    (
        SqlServerDiffCursorFilterTestCase(
            description="renders integer diff bounds without string coercion",
            cursor_column="order_id",
            start_cursor=CursorValue(kind=CursorKind.INTEGER, value=2),
            end_cursor=CursorValue(kind=CursorKind.INTEGER, value=4),
            expected_filter="[order_id] >= 2 AND [order_id] < 4",
        ),
        SqlServerDiffCursorFilterTestCase(
            description="casts timestamp diff bounds to datetime2",
            cursor_column="ordered_at",
            start_cursor=CursorValue(
                kind=CursorKind.TIMESTAMP,
                value=datetime.fromisoformat("2026-04-04T13:30:00.000001"),
            ),
            end_cursor=CursorValue(
                kind=CursorKind.TIMESTAMP,
                value=datetime.fromisoformat("2026-04-04T14:30:00.000001"),
            ),
            expected_filter=(
                "[ordered_at] >= CAST('2026-04-04 13:30:00.000001' AS DATETIME2(6)) "
                "AND [ordered_at] < CAST('2026-04-04 14:30:00.000001' AS DATETIME2(6))"
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_typed_cursors_when_building_diff_filter_then_sqlserver_uses_typed_literals(
    test_case: SqlServerDiffCursorFilterTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    cursor_filter: str = adapter.build_cursor_filter(
        cursor_column=test_case.cursor_column,
        start_cursor=test_case.start_cursor,
        end_cursor=test_case.end_cursor,
    )

    assert cursor_filter == test_case.expected_filter


@pytest.mark.parametrize(
    "test_case",
    (
        SqlServerRowDiffEqualitySqlTestCase(
            description="renders portable null-safe equality",
            column="status",
            column_info=ColumnInfo(name="status", type="nvarchar"),
            tolerances=None,
            expected_fragments=(
                "__left.status IS NULL AND __right.status IS NULL",
                "__left.status IS NOT NULL AND __right.status IS NOT NULL",
                "__left.status = __right.status",
            ),
            unexpected_fragments=("IS NOT DISTINCT FROM",),
        ),
        SqlServerRowDiffEqualitySqlTestCase(
            description="renders relative tolerance without greatest",
            column="amount",
            column_info=ColumnInfo(name="amount", type="decimal"),
            tolerances=RowDiffTolerances(
                by_column={"amount": RowDiffTolerance(relative=Decimal("0.01"))}
            ),
            expected_fragments=("CASE WHEN", "0.01 *", "ABS(__left.amount - __right.amount)"),
            unexpected_fragments=("GREATEST",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_diff_column_when_rendering_equality_then_sqlserver_uses_portable_sql(
    test_case: SqlServerRowDiffEqualitySqlTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    expression: str = adapter.build_row_diff_equal_expression(
        column=test_case.column,
        column_info=test_case.column_info,
        tolerances=test_case.tolerances,
    )

    for fragment in test_case.expected_fragments:
        assert fragment in expression
    for fragment in test_case.unexpected_fragments:
        assert fragment not in expression


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerRollbackTestCase(
            description="guards rollback when sqlserver already ended the transaction",
            expected_statement="IF @@TRANCOUNT > 0 ROLLBACK TRANSACTION",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_transaction_when_rolling_back_then_sqlserver_checks_transaction_count(
    test_case: SqlServerRollbackTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()
    connection: FakeSqlServerConnection = FakeSqlServerConnection()

    adapter.rollback(connection)

    assert connection.executed_sql == [test_case.expected_statement]


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerRenderQualifiedNameTestCase(
            description="renders three part qualified name",
            database="tempdb",
            schema="analytics_dev",
            name="fact_orders",
            expected_name="tempdb.analytics_dev.fact_orders",
        ),
        SqlServerRenderQualifiedNameTestCase(
            description="renders schema qualified name",
            database=None,
            schema="analytics_dev",
            name="fact_orders",
            expected_name="analytics_dev.fact_orders",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_relation_parts_when_rendering_qualified_name_then_sqlserver_joins_parts(
    test_case: SqlServerRenderQualifiedNameTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    qualified_name: str | None = adapter.render_qualified_name(
        database=test_case.database,
        schema=test_case.schema,
        name=test_case.name,
    )

    assert qualified_name == test_case.expected_name


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerRenderRenameTestCase(
            description="renders sp_rename without database and without quoted destination name",
            origin="[tempdb].[analytics_dev].[fact_orders__sqlbuild_reuse_stage]",
            destination='"tempdb"."analytics_dev"."fact_orders"',
            expected_statements=(
                "EXEC sp_rename '[analytics_dev].[fact_orders__sqlbuild_reuse_stage]', "
                "'fact_orders'",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_quoted_dbt_relation_when_rendering_rename_then_sqlserver_normalizes_names(
    test_case: SqlServerRenderRenameTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    statements: tuple[str, ...] = adapter.render_rename(
        origin=test_case.origin,
        destination=test_case.destination,
    )

    assert statements == test_case.expected_statements


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerRenderCreateTableAsTestCase(
            description="renders drop and select into for table replacement",
            target="dbo.fact_orders",
            sql="SELECT id FROM dbo.stg_orders",
            expected_statements=(
                "DROP TABLE IF EXISTS dbo.fact_orders",
                "SELECT * INTO dbo.fact_orders FROM "
                "(SELECT id FROM dbo.stg_orders) AS __create_source",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_table_target_when_rendering_create_then_sqlserver_uses_select_into(
    test_case: SqlServerRenderCreateTableAsTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    statements: tuple[str, ...] = adapter.render_create_table_as(
        destination=test_case.target,
        sql=test_case.sql,
    )

    assert statements == test_case.expected_statements


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerRenderCreateFunctionTestCase(
            description="renders scalar SQL function with T-SQL body",
            arguments=(("order_status", "NVARCHAR(MAX)"),),
            returns="BIT",
            body_sql="SELECT order_status = 'completed'",
            expected_statements=(
                "CREATE OR ALTER FUNCTION dbo.is_completed_order(@order_status NVARCHAR(MAX))\n"
                "RETURNS BIT\n"
                "AS\n"
                "BEGIN\n"
                "    RETURN (CASE WHEN @order_status = 'completed' THEN 1 ELSE 0 END)\n"
                "END",
            ),
        ),
        SqlServerRenderCreateFunctionTestCase(
            description="renders a table function as an inline TVF that keeps its SELECT",
            arguments=(("p_floor", "FLOAT"),),
            returns="table",
            body_sql=(
                "SELECT id, CASE WHEN amount > p_floor THEN 'high' ELSE 'low' END AS band "
                "FROM dbo.orders;"
            ),
            expected_statements=(
                "CREATE OR ALTER FUNCTION dbo.is_completed_order(@p_floor FLOAT)\n"
                "RETURNS TABLE\n"
                "AS\n"
                "RETURN (\n"
                "SELECT id, CASE WHEN amount > @p_floor THEN 'high' ELSE 'low' END AS band "
                "FROM dbo.orders\n"
                ")",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sql_function_when_rendering_create_then_sqlserver_declares_function_body(
    test_case: SqlServerRenderCreateFunctionTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    statements: tuple[str, ...] = adapter.render_create_function(
        destination="dbo.is_completed_order",
        arguments=tuple(
            FunctionArgument(name=name, type=type_name) for name, type_name in test_case.arguments
        ),
        returns=test_case.returns,
        body_sql=test_case.body_sql,
    )

    assert statements == test_case.expected_statements


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerRenderCreateSchemaTestCase(
            description="renders conditional schema creation",
            schema="analytics",
            expected_statement=(
                "IF NOT EXISTS (SELECT 1 FROM sys.schemas WHERE name = 'analytics') "
                "EXEC('CREATE SCHEMA [analytics]')"
            ),
        ),
        SqlServerRenderCreateSchemaTestCase(
            description="doubles quotes in both the lookup and the EXEC string",
            schema="sqb'meta]x",
            expected_statement=(
                "IF NOT EXISTS (SELECT 1 FROM sys.schemas WHERE name = 'sqb''meta]x') "
                "EXEC('CREATE SCHEMA [sqb''meta]]x]')"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_schema_when_rendering_create_then_sqlserver_checks_sys_schemas(
    test_case: SqlServerRenderCreateSchemaTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    (statement,) = adapter.render_create_schema(database=None, schema=test_case.schema)

    assert statement == test_case.expected_statement


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerIndexSqlTestCase(
            description="renders guarded latest-read index for fingerprint table",
            database=None,
            schema="analytics",
            expected_statements=(
                "IF NOT EXISTS (SELECT 1 FROM sys.indexes "
                "WHERE name = '_sqlbuild_fingerprints_latest_idx' "
                "AND object_id = OBJECT_ID(N'analytics._sqlbuild_fingerprints')) "
                "CREATE INDEX _sqlbuild_fingerprints_latest_idx "
                "ON analytics._sqlbuild_fingerprints (node_type, node_name, ts DESC, run_id DESC)",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_fingerprint_table_when_rendering_indexes_then_sqlserver_uses_latest_read_keys(
    test_case: SqlServerIndexSqlTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    statements: tuple[str, ...] = adapter.render_create_fingerprint_index_sqls(
        database=test_case.database,
        schema=test_case.schema,
    )

    assert statements == test_case.expected_statements


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerIndexSqlTestCase(
            description="renders guarded latest-read index for source freshness table",
            database=None,
            schema="analytics",
            expected_statements=(
                "IF NOT EXISTS (SELECT 1 FROM sys.indexes "
                "WHERE name = '_sqlbuild_source_freshness_latest_idx' "
                "AND object_id = OBJECT_ID(N'analytics._sqlbuild_source_freshness')) "
                "CREATE INDEX _sqlbuild_source_freshness_latest_idx "
                "ON analytics._sqlbuild_source_freshness ("
                "source_name, target_database, target_schema, target_name, "
                "observed_at DESC, run_id DESC)",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_freshness_table_when_rendering_indexes_then_sqlserver_uses_latest_read_keys(
    test_case: SqlServerIndexSqlTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    statements: tuple[str, ...] = adapter.render_create_source_freshness_index_sqls(
        database=test_case.database,
        schema=test_case.schema,
    )

    assert statements == test_case.expected_statements


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerLatestReadSqlTestCase(
            description="renders guarded node result table create",
            database=None,
            schema="analytics",
            expected_fragments=(
                "IF NOT EXISTS (SELECT 1 FROM information_schema.tables ",
                "WHERE table_schema = 'analytics' AND table_name = '_sqlbuild_node_results')",
                "CREATE TABLE analytics._sqlbuild_node_results (",
                "node_type NVARCHAR(450) NOT NULL",
                "ts DATETIME2 NOT NULL",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_node_result_table_when_rendering_create_then_sqlserver_guards_missing_table(
    test_case: SqlServerLatestReadSqlTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    statement: str = adapter.render_create_node_result_table_sql(
        database=test_case.database,
        schema=test_case.schema,
    )

    expected_fragment: str
    for expected_fragment in test_case.expected_fragments:
        assert expected_fragment in statement


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerLatestReadSqlTestCase(
            description="renders guarded source freshness table with indexable keys",
            database="shop",
            schema="analytics",
            expected_fragments=(
                "IF NOT EXISTS (SELECT 1 FROM information_schema.tables ",
                "WHERE table_schema = 'analytics' AND table_name = '_sqlbuild_source_freshness' "
                "AND table_catalog = 'shop')",
                "CREATE TABLE shop.analytics._sqlbuild_source_freshness (",
                "source_name NVARCHAR(450)",
                "target_database NVARCHAR(450)",
                "target_schema NVARCHAR(450)",
                "target_name NVARCHAR(450)",
                "run_id NVARCHAR(450)",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_freshness_table_when_rendering_create_then_sqlserver_guards_missing_table(
    test_case: SqlServerLatestReadSqlTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    statement: str = adapter.render_create_source_freshness_table_sql(
        database=test_case.database,
        schema=test_case.schema,
    )

    for expected_fragment in test_case.expected_fragments:
        assert expected_fragment in statement, statement
    assert "CREATE TABLE IF NOT EXISTS" not in statement
    index_sql: str = adapter.render_create_source_freshness_index_sqls(
        database=test_case.database, schema=test_case.schema
    )[0]
    for key_column in ("source_name", "target_database", "target_schema", "target_name"):
        assert key_column in index_sql
        assert f"{key_column} NVARCHAR(MAX)" not in statement


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerLatestReadSqlTestCase(
            description="renders bounded fingerprint key columns",
            database=None,
            schema="analytics",
            expected_fragments=(
                "IF NOT EXISTS (SELECT 1 FROM information_schema.tables ",
                "WHERE table_schema = 'analytics' AND table_name = '_sqlbuild_fingerprints')",
                "CREATE TABLE analytics._sqlbuild_fingerprints (",
                "node_type NVARCHAR(450) NOT NULL",
                "node_name NVARCHAR(450) NOT NULL",
                "run_id NVARCHAR(450) NOT NULL",
                "definition_hash NVARCHAR(450) NOT NULL",
                "definition_b64 NVARCHAR(MAX) NOT NULL",
                "metadata_json_b64 NVARCHAR(MAX) NOT NULL",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_fingerprint_table_when_rendering_create_then_sqlserver_uses_indexable_keys(
    test_case: SqlServerLatestReadSqlTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    statement: str = adapter.render_create_fingerprint_table_sql(
        database=test_case.database,
        schema=test_case.schema,
    )

    expected_fragment: str
    for expected_fragment in test_case.expected_fragments:
        assert expected_fragment in statement


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerIndexSqlTestCase(
            description="renders guarded lookup indexes for node result table",
            database=None,
            schema="analytics",
            expected_statements=(
                "IF NOT EXISTS (SELECT 1 FROM sys.indexes "
                "WHERE name = '_sqlbuild_node_results_latest_idx' "
                "AND object_id = OBJECT_ID(N'analytics._sqlbuild_node_results')) "
                "CREATE INDEX _sqlbuild_node_results_latest_idx "
                "ON analytics._sqlbuild_node_results ("
                "node_type, node_name, target_database, target_schema, target_name, status, "
                "ts DESC, run_id DESC)",
                "IF NOT EXISTS (SELECT 1 FROM sys.indexes "
                "WHERE name = '_sqlbuild_node_results_run_id_idx' "
                "AND object_id = OBJECT_ID(N'analytics._sqlbuild_node_results')) "
                "CREATE INDEX _sqlbuild_node_results_run_id_idx "
                "ON analytics._sqlbuild_node_results ("
                "run_id, node_type, node_name, target_database, target_schema, target_name)",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_node_result_table_when_rendering_indexes_then_sqlserver_uses_lookup_keys(
    test_case: SqlServerIndexSqlTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    statements: tuple[str, ...] = adapter.render_create_node_result_index_sqls(
        database=test_case.database,
        schema=test_case.schema,
    )

    assert statements == test_case.expected_statements


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerLatestReadSqlTestCase(
            description="renders windowed fingerprint latest read",
            database=None,
            schema="analytics",
            expected_fragments=(
                "ROW_NUMBER() OVER",
                "PARTITION BY node_type, node_name",
                "ORDER BY ts DESC, run_id DESC",
                "FROM analytics._sqlbuild_fingerprints",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_fingerprint_table_when_rendering_latest_read_then_sqlserver_uses_window_query(
    test_case: SqlServerLatestReadSqlTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    sql: str = adapter.render_read_latest_fingerprints_sql(
        database=test_case.database,
        schema=test_case.schema,
    )

    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in sql


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerLatestReadSqlTestCase(
            description="renders windowed source freshness latest read",
            database=None,
            schema="analytics",
            expected_fragments=(
                "ROW_NUMBER() OVER",
                "PARTITION BY source_name, target_database, target_schema, target_name",
                "ORDER BY observed_at DESC, run_id DESC",
                "FROM analytics._sqlbuild_source_freshness",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_freshness_when_rendering_latest_read_then_sqlserver_uses_window_query(
    test_case: SqlServerLatestReadSqlTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    sql: str = adapter.render_read_latest_source_freshness_sql(
        database=test_case.database,
        schema=test_case.schema,
    )

    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in sql


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerPruneSqlTestCase(
            description="renders fingerprint pruning with writable ranked CTE",
            database=None,
            schema="analytics",
            retain_versions=5,
            expected_fragments=(
                "WITH __sqlbuild_ranked AS",
                "FROM analytics._sqlbuild_fingerprints",
                "PARTITION BY node_type, node_name",
                "ORDER BY ts DESC, run_id DESC",
                "DELETE FROM __sqlbuild_ranked WHERE __sqlbuild_history_rank > 5",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_fingerprint_table_when_rendering_prune_then_sqlserver_uses_history_rank(
    test_case: SqlServerPruneSqlTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    sql: str = adapter.render_prune_fingerprint_history_sql(
        database=test_case.database,
        schema=test_case.schema,
        retain_versions=test_case.retain_versions,
    )

    for fragment in test_case.expected_fragments:
        assert fragment in sql


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerPruneSqlTestCase(
            description="renders source freshness pruning with full identity",
            database=None,
            schema="analytics",
            retain_versions=3,
            expected_fragments=(
                "WITH __sqlbuild_ranked AS",
                "FROM analytics._sqlbuild_source_freshness",
                "PARTITION BY source_name, target_database, target_schema, target_name",
                "ORDER BY observed_at DESC, run_id DESC",
                "DELETE FROM __sqlbuild_ranked WHERE __sqlbuild_history_rank > 3",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_freshness_table_when_rendering_prune_then_sqlserver_uses_history_rank(
    test_case: SqlServerPruneSqlTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    sql: str = adapter.render_prune_source_freshness_history_sql(
        database=test_case.database,
        schema=test_case.schema,
        retain_versions=test_case.retain_versions,
    )

    for fragment in test_case.expected_fragments:
        assert fragment in sql


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerMoveOrCopyRelationTestCase(
            description="moves table across schemas with transfer and rename",
            source="[marts].[fact_orders]",
            target="[marts__sqb_physical].[fact_orders__v_abc123]",
            expected_statements=(
                "ALTER SCHEMA [marts__sqb_physical] TRANSFER [marts].[fact_orders]",
                "EXEC sp_rename '[marts__sqb_physical].[fact_orders]', 'fact_orders__v_abc123'",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_cross_schema_table_move_when_moving_then_sqlserver_uses_native_transfer(
    test_case: SqlServerMoveOrCopyRelationTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()
    connection: FakeSqlServerConnection = FakeSqlServerConnection()
    statement_recorder: StatementRecorder = StatementRecorder()

    adapter.move_or_copy_relation(
        connection=connection,
        origin=test_case.source,
        destination=test_case.target,
        remove_origin=True,
        allow_copy_fallback=False,
        statement_recorder=statement_recorder,
    )

    assert tuple(connection.executed_sql) == test_case.expected_statements
    assert tuple(event.content for event in statement_recorder.snapshot()) == (
        test_case.expected_statements
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SqlServerSourceExpressionRelationTestCase(
            description="a SELECT becomes a derived table",
            expression="SELECT id FROM raw.orders;",
            expected_relation="(SELECT id FROM raw.orders)",
            expected_error_code=None,
        ),
        SqlServerSourceExpressionRelationTestCase(
            description="a relation whose name starts with with stays a relation",
            expression="raw.with_orders",
            expected_relation="raw.with_orders",
            expected_error_code=None,
        ),
        SqlServerSourceExpressionRelationTestCase(
            description="a WITH query is refused because SQL Server cannot nest it",
            expression="WITH x AS (SELECT 1 AS id) SELECT id FROM x",
            expected_relation=None,
            expected_error_code="A715",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_expression_when_rendering_relation_then_sqlserver_nests_only_what_it_can(
    test_case: SqlServerSourceExpressionRelationTestCase,
) -> None:
    adapter: SqlServerAdapter = SqlServerAdapter()

    try:
        relation: str | None = adapter.render_source_expression_relation(
            expression=test_case.expression
        )
        error_code: str | None = None
    except AdapterUserError as error:
        relation = None
        error_code = error.code

    assert (relation, error_code) == (test_case.expected_relation, test_case.expected_error_code)
