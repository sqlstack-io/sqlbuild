from __future__ import annotations

from datetime import UTC, date, datetime
from pathlib import Path
from types import SimpleNamespace
from typing import Any

import pytest

from sqlbuild._native import build_microbatch_insert_many_sql
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.contract.models import (
    ColumnInfo,
    CursorValue,
    QueryResult,
    RelationInfo,
    SchemaDiffResult,
)
from sqlbuild.adapter.contract.types import RelationReadStatus
from sqlbuild.adapters.clickhouse.classes.clickhouse_adapter import ClickHouseAdapter
from sqlbuild.compiler.compile.types import FunctionLanguage
from tests.integration.src.sqlbuild.adapters.clickhouse._test_types import (
    ClickHouseCloneTestCase,
    ClickHouseColumnMigrationTestCase,
    ClickHouseConcurrentWindowTestCase,
    ClickHouseCopyGenerationTestCase,
    ClickHouseCursorFilterTestCase,
    ClickHouseFunctionTestCase,
    ClickHouseGenerationTestCase,
    ClickHouseInspectionTestCase,
    ClickHouseInterruptTestCase,
    ClickHouseQualifiedNameTestCase,
    ClickHouseQueryTestCase,
    ClickHouseRefusedFunctionTestCase,
    ClickHouseRewriteFaultTestCase,
    ClickHouseRewriteTestCase,
    ClickHouseSchemaDiffTestCase,
    ClickHouseSeedTestCase,
    ClickHouseStateTableTestCase,
    ClickHouseStateTimestampTestCase,
    ClickHouseSwapTestCase,
)
from tests.integration.src.sqlbuild.adapters.clickhouse.helpers import (
    INITIAL_ORDERS,
    FailingExecute,
    InjectedFailure,
    apply_write,
    copy_relation,
    create_orders,
    cursor_delta_sql,
    execute_all,
    fetch_rows,
    incoming_rows_sql,
    interrupt_running_query,
    microbatch_completion,
    oracle_rows,
    order_rows_as_dates,
    recorded_statements,
    run_concurrent_cursor_windows,
    write_cursor_delta,
)

_PARTITIONED: str = "toYYYYMM(order_date)"
_UNPARTITIONED: str = "tuple()"
_CURSOR_SKIP_WRITE: ClickHouseRewriteTestCase = ClickHouseRewriteTestCase(
    description="an old partition's update outruns a new partition's insert",
    strategy="delete_insert",
    partition_by=_PARTITIONED,
    initial_rows=INITIAL_ORDERS,
    incoming_rows=((1, "ada", 61, "2026-01-05"), (6, "fa", 55, "2026-03-09")),
)
_CURSOR_SKIP_SOURCE: tuple[tuple[int, str, int, str], ...] = (
    *INITIAL_ORDERS[1:],
    *_CURSOR_SKIP_WRITE.incoming_rows,
)


_EXCHANGE_REWRITE_STEPS: tuple[str, ...] = (
    "drop stale delta",
    "create delta",
    "stage delta rows",
    "count delta rows",
    "find touched partitions",
    "read the table generation",
    "drop stale replacement",
    "create replacement",
    "attach every partition",
    "drop rewritten partitions",
    "insert kept rows",
    "insert delta rows",
    "carry the generation",
    "exchange tables",
    "drop replacement",
    "drop delta",
)


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseQueryTestCase(
            description="returns named rows",
            sql="SELECT 1 AS id, 'hello' AS name",
            expected_columns=("id", "name"),
            expected_rows=((1, "hello"),),
        ),
        ClickHouseQueryTestCase(
            description="pins distinct set operations and outer join nulls",
            sql=(
                "SELECT a, b.y FROM (SELECT 1 AS a UNION SELECT 1 UNION SELECT 2) AS l "
                "LEFT JOIN (SELECT 1 AS x, 'v' AS y) AS b ON l.a = b.x ORDER BY a"
            ),
            expected_columns=("a", "y"),
            expected_rows=((1, "v"), (2, None)),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sql_when_querying_then_clickhouse_returns_rows(
    test_case: ClickHouseQueryTestCase, adapter: ClickHouseAdapter, connection: Any
) -> None:
    result: QueryResult = adapter.query(connection=connection, sql=test_case.sql, limit=None)

    assert result.columns == test_case.expected_columns
    assert tuple(tuple(row) for row in result.rows) == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseInspectionTestCase(
            description="table columns in position order",
            ddl="CREATE TABLE {target} (id Int64, total Nullable(Decimal(10, 2))) "
            "ENGINE = MergeTree ORDER BY id",
            expected_columns=(
                ColumnInfo(name="id", type="Int64"),
                ColumnInfo(name="total", type="Nullable(Decimal(10, 2))"),
            ),
            expected_relation_type="BASE TABLE",
        ),
        ClickHouseInspectionTestCase(
            description="view columns",
            ddl="CREATE VIEW {target} AS SELECT toInt64(1) AS id, 'x' AS label",
            expected_columns=(
                ColumnInfo(name="id", type="Int64"),
                ColumnInfo(name="label", type="String"),
            ),
            expected_relation_type="VIEW",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_relation_when_inspecting_then_clickhouse_reports_columns_and_kind(
    test_case: ClickHouseInspectionTestCase,
    adapter: ClickHouseAdapter,
    connection: Any,
    clickhouse_schema: str,
) -> None:
    target: str = f"{clickhouse_schema}.orders"
    adapter.execute(connection=connection, sql=test_case.ddl.format(target=target))

    relations: tuple[RelationInfo, ...] = adapter.list_relations(
        connection=connection, database=None, schemas=(clickhouse_schema,)
    )

    assert relations == (
        RelationInfo(
            database=None,
            schema=clickhouse_schema,
            name="orders",
            relation_type=test_case.expected_relation_type,
        ),
    )
    assert adapter.describe_relation(connection=connection, relation=target) == (
        test_case.expected_columns
    )
    assert adapter.get_columns(
        connection=connection, database=None, schema=clickhouse_schema, name="orders"
    ) == (test_case.expected_columns)
    assert adapter.relation_exists(
        connection=connection, database=None, schema=clickhouse_schema, name="orders"
    )
    assert adapter.schema_exists(connection=connection, database=None, schema=clickhouse_schema)
    assert (
        adapter.probe_relation_read(connection=connection, relation=target).status
        == RelationReadStatus.READABLE
    )
    assert (
        adapter.probe_relation_read(connection=connection, relation=f"{target}_missing").status
        == RelationReadStatus.MISSING
    )
    assert (
        adapter.probe_relation_read(connection=connection, relation="sqb_missing_db.orders").status
        == RelationReadStatus.MISSING
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseQualifiedNameTestCase(
            description="database qualifier refused",
            database="warehouse",
            schema="sales",
            name="orders",
            expected_code="A704",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_database_in_target_when_qualifying_then_clickhouse_refuses_with_code(
    test_case: ClickHouseQualifiedNameTestCase, adapter: ClickHouseAdapter
) -> None:
    with pytest.raises(AdapterUserError) as error:
        adapter.render_qualified_name(
            database=test_case.database, schema=test_case.schema, name=test_case.name
        )

    assert error.value.code == test_case.expected_code


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseSeedTestCase(
            description="typed seed round-trips nulls, quotes and backslashes",
            csv_contents=(
                "id,label,amount,ordered_at,is_gift,ship_date\n"
                '1,"plain",1.50,2026-01-02T03:04:05.123456,true,2026-01-03\n'
                '2,"it\'s ""quoted""",,2026-01-02 00:00:00,false,\n'
                '3,"back\\slash",2.25,,,1950-06-01\n'
            ),
            columns=(
                ColumnInfo(name="id", type="Int64"),
                ColumnInfo(name="label", type="String"),
                ColumnInfo(name="amount", type="Decimal(10, 2)"),
                ColumnInfo(name="ordered_at", type="DateTime64(6, 'UTC')"),
                ColumnInfo(name="is_gift", type="Bool"),
                ColumnInfo(name="ship_date", type="Date32"),
            ),
            expected_rows=(
                (1, "plain", "1.5", "2026-01-02 03:04:05.123456", True, date(2026, 1, 3)),
                (2, 'it\'s "quoted"', None, "2026-01-02 00:00:00.000000", False, None),
                (3, "back\\slash", "2.25", None, None, date(1950, 6, 1)),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_seed_file_when_loading_then_clickhouse_streams_typed_rows(
    test_case: ClickHouseSeedTestCase,
    adapter: ClickHouseAdapter,
    connection: Any,
    clickhouse_schema: str,
    tmp_path: Path,
) -> None:
    seed_path: Path = tmp_path / "orders.csv"
    seed_path.write_text(test_case.csv_contents, encoding="utf-8")
    target: str = f"{clickhouse_schema}.orders"
    recorder: StatementRecorder = StatementRecorder()

    adapter.load_seed(
        connection=connection,
        destination=target,
        file_path=seed_path,
        columns=test_case.columns,
        statement_recorder=recorder,
    )

    rows: tuple[tuple[object, ...], ...] = fetch_rows(
        adapter=adapter,
        connection=connection,
        sql=(
            "SELECT id, label, toString(amount), "
            "toString(ordered_at), is_gift, ship_date FROM " + target + " ORDER BY id"
        ),
    )
    assert rows == test_case.expected_rows
    assert all("VALUES" not in statement for statement in recorded_statements(recorder))


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseSwapTestCase(
            description="dependent view sees the exchanged table",
            target_sql="SELECT 1 AS id",
            staging_sql="SELECT number AS id FROM numbers(5)",
            expected_view_rows=((5,),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_staged_table_when_swapping_then_clickhouse_exchanges_atomically(
    test_case: ClickHouseSwapTestCase,
    adapter: ClickHouseAdapter,
    connection: Any,
    clickhouse_schema: str,
) -> None:
    recorder: StatementRecorder = StatementRecorder()
    target: str = f"{clickhouse_schema}.orders"
    staging: str = f"{clickhouse_schema}.orders__staging"
    adapter.create_table_as(
        connection=connection,
        destination=target,
        sql=test_case.target_sql,
        statement_recorder=recorder,
    )
    adapter.create_table_as(
        connection=connection,
        destination=staging,
        sql=test_case.staging_sql,
        statement_recorder=recorder,
    )
    adapter.create_view_as(
        connection=connection,
        destination=f"{clickhouse_schema}.orders_view",
        sql=f"SELECT count() AS n FROM {target}",
        statement_recorder=recorder,
    )
    before: str | None = adapter.physical_relation_generation(
        connection=connection, database=None, schema=clickhouse_schema, name="orders"
    )

    adapter.swap(connection=connection, left=target, right=staging, statement_recorder=recorder)

    after: str | None = adapter.physical_relation_generation(
        connection=connection, database=None, schema=clickhouse_schema, name="orders"
    )
    assert before is not None and after is not None and before != after
    assert fetch_rows(
        adapter=adapter,
        connection=connection,
        sql=f"SELECT n FROM {clickhouse_schema}.orders_view",
    ) == (test_case.expected_view_rows)
    assert f"EXCHANGE TABLES {target} AND {staging}" in recorded_statements(recorder)


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseStateTableTestCase(
            description="every state table is a MergeTree", expected_engines=(("MergeTree",),)
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_framework_state_ddl_when_creating_then_clickhouse_accepts_every_table(
    test_case: ClickHouseStateTableTestCase,
    adapter: ClickHouseAdapter,
    connection: Any,
    clickhouse_schema: str,
) -> None:
    statements: tuple[str, ...] = (
        adapter.render_create_fingerprint_table_sql(database=None, schema=clickhouse_schema),
        adapter.render_create_source_freshness_table_sql(database=None, schema=clickhouse_schema),
        adapter.render_create_node_result_table_sql(database=None, schema=clickhouse_schema),
        adapter.render_create_audit_result_table_sql(database=None, schema=clickhouse_schema),
        adapter.render_create_janitor_event_table_sql(database=None, schema=clickhouse_schema),
        adapter.render_create_migration_state_table_sql(database=None, schema=clickhouse_schema),
        adapter.render_create_old_name_view_state_table_sql(
            database=None, schema=clickhouse_schema
        ),
        adapter.render_create_column_migration_state_table_sql(
            database=None, schema=clickhouse_schema
        ),
        adapter.render_create_microbatch_state_table_sql(database=None, schema=clickhouse_schema),
    )

    execute_all(adapter=adapter, connection=connection, statements=statements)
    execute_all(adapter=adapter, connection=connection, statements=statements)
    execute_all(
        adapter=adapter,
        connection=connection,
        statements=(
            adapter.render_read_latest_fingerprints_sql(database=None, schema=clickhouse_schema),
            adapter.render_prune_fingerprint_history_sql(
                database=None, schema=clickhouse_schema, retain_versions=1
            ),
            adapter.render_prune_source_freshness_history_sql(
                database=None, schema=clickhouse_schema, retain_versions=1
            ),
        ),
    )

    engines: tuple[tuple[object, ...], ...] = fetch_rows(
        adapter=adapter,
        connection=connection,
        sql=f"SELECT DISTINCT engine FROM system.tables WHERE database = '{clickhouse_schema}'",
    )
    assert engines == test_case.expected_engines


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseRewriteTestCase(
            description="append keeps every row (partitioned)",
            strategy="append",
            partition_by=_PARTITIONED,
            initial_rows=INITIAL_ORDERS,
            incoming_rows=((6, "fa", 60, "2026-03-09"),),
        ),
        ClickHouseRewriteTestCase(
            description="delete_insert replaces keys moving between partitions (partitioned)",
            strategy="delete_insert",
            partition_by=_PARTITIONED,
            initial_rows=INITIAL_ORDERS,
            incoming_rows=(
                (2, "bo", 21, "2026-03-02"),
                (4, "di", 41, "2026-02-17"),
                (7, "gu", 70, "2026-04-01"),
            ),
        ),
        ClickHouseRewriteTestCase(
            description="merge upserts and keeps excluded columns (partitioned)",
            strategy="merge",
            partition_by=_PARTITIONED,
            initial_rows=INITIAL_ORDERS,
            incoming_rows=((1, "ann", 11, "2026-01-05"), (8, "ho", 80, "2026-01-09")),
            exclude_columns=("customer",),
        ),
        ClickHouseRewriteTestCase(
            description="cursor window replaces only the window (partitioned)",
            strategy="cursor",
            partition_by=_PARTITIONED,
            initial_rows=INITIAL_ORDERS,
            incoming_rows=((9, "iv", 90, "2026-01-21"),),
            window=("2026-01-15", "2026-02-10"),
        ),
        ClickHouseRewriteTestCase(
            description="keyed cursor window also replaces a key from another partition (partitioned)",
            strategy="keyed_cursor",
            partition_by=_PARTITIONED,
            initial_rows=INITIAL_ORDERS,
            incoming_rows=((1, "ada", 11, "2026-01-21"), (9, "iv", 90, "2026-01-22")),
            window=("2026-01-15", "2026-02-10"),
        ),
        ClickHouseRewriteTestCase(
            description="empty window clears its rows (partitioned)",
            strategy="cursor",
            partition_by=_PARTITIONED,
            initial_rows=INITIAL_ORDERS,
            incoming_rows=(),
            window=("2026-02-01", "2026-03-01"),
        ),
        ClickHouseRewriteTestCase(
            description="append keeps every row (unpartitioned)",
            strategy="append",
            partition_by=_UNPARTITIONED,
            initial_rows=INITIAL_ORDERS,
            incoming_rows=((6, "fa", 60, "2026-03-09"),),
        ),
        ClickHouseRewriteTestCase(
            description="delete_insert replaces keys moving between partitions (unpartitioned)",
            strategy="delete_insert",
            partition_by=_UNPARTITIONED,
            initial_rows=INITIAL_ORDERS,
            incoming_rows=(
                (2, "bo", 21, "2026-03-02"),
                (4, "di", 41, "2026-02-17"),
                (7, "gu", 70, "2026-04-01"),
            ),
        ),
        ClickHouseRewriteTestCase(
            description="merge upserts and keeps excluded columns (unpartitioned)",
            strategy="merge",
            partition_by=_UNPARTITIONED,
            initial_rows=INITIAL_ORDERS,
            incoming_rows=((1, "ann", 11, "2026-01-05"), (8, "ho", 80, "2026-01-09")),
            exclude_columns=("customer",),
        ),
        ClickHouseRewriteTestCase(
            description="cursor window replaces only the window (unpartitioned)",
            strategy="cursor",
            partition_by=_UNPARTITIONED,
            initial_rows=INITIAL_ORDERS,
            incoming_rows=((9, "iv", 90, "2026-01-21"),),
            window=("2026-01-15", "2026-02-10"),
        ),
        ClickHouseRewriteTestCase(
            description="keyed cursor window also replaces a key from another partition (unpartitioned)",
            strategy="keyed_cursor",
            partition_by=_UNPARTITIONED,
            initial_rows=INITIAL_ORDERS,
            incoming_rows=((1, "ada", 11, "2026-01-21"), (9, "iv", 90, "2026-01-22")),
            window=("2026-01-15", "2026-02-10"),
        ),
        ClickHouseRewriteTestCase(
            description="empty window clears its rows (unpartitioned)",
            strategy="cursor",
            partition_by=_UNPARTITIONED,
            initial_rows=INITIAL_ORDERS,
            incoming_rows=(),
            window=("2026-02-01", "2026-03-01"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_incremental_write_when_rewriting_partitions_then_rows_match_duckdb_oracle(
    test_case: ClickHouseRewriteTestCase,
    adapter: ClickHouseAdapter,
    connection: Any,
    clickhouse_schema: str,
) -> None:
    relation: str = f"{clickhouse_schema}.orders"
    create_orders(
        adapter=adapter,
        connection=connection,
        relation=relation,
        partition_by=test_case.partition_by,
        rows=test_case.initial_rows,
    )
    apply_write(adapter=adapter, connection=connection, test_case=test_case, relation=relation)

    rows: tuple[tuple[object, ...], ...] = fetch_rows(
        adapter=adapter,
        connection=connection,
        sql=f"SELECT order_id, customer, amount, order_date FROM {relation} "
        "ORDER BY order_id, customer, amount",
    )
    assert rows == oracle_rows(test_case)
    leftovers: tuple[tuple[object, ...], ...] = fetch_rows(
        adapter=adapter,
        connection=connection,
        sql=f"SELECT name FROM system.tables WHERE database = '{clickhouse_schema}' "
        "AND name != 'orders'",
    )
    assert leftovers == test_case.expected_leftover_tables


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseRewriteFaultTestCase(
            description=step,
            fail_statement_number=number,
            expected_states_after_crash=(
                order_rows_as_dates(_CURSOR_SKIP_WRITE.initial_rows),
                oracle_rows(_CURSOR_SKIP_WRITE),
            ),
            expected_rows_after_rerun=oracle_rows(_CURSOR_SKIP_WRITE),
        )
        for number, step in enumerate(_EXCHANGE_REWRITE_STEPS, start=1)
    ],
    ids=lambda case: case.description,
)
def test_given_interruption_at_any_rewrite_step_when_next_cursor_run_then_no_row_is_skipped(
    test_case: ClickHouseRewriteFaultTestCase,
    adapter: ClickHouseAdapter,
    connection: Any,
    clickhouse_schema: str,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    relation: str = f"{clickhouse_schema}.orders"
    source: str = f"{clickhouse_schema}.source_orders"
    create_orders(
        adapter=adapter,
        connection=connection,
        relation=relation,
        partition_by=_CURSOR_SKIP_WRITE.partition_by,
        rows=_CURSOR_SKIP_WRITE.initial_rows,
    )
    create_orders(
        adapter=adapter,
        connection=connection,
        relation=source,
        partition_by=_UNPARTITIONED,
        rows=_CURSOR_SKIP_SOURCE,
    )
    first_delta: str = cursor_delta_sql(
        adapter=adapter, connection=connection, relation=relation, source=source
    )
    monkeypatch.setattr(
        adapter,
        "execute",
        FailingExecute(
            execute=adapter.execute, fail_statement_number=test_case.fail_statement_number
        ),
    )
    with pytest.raises(InjectedFailure):
        write_cursor_delta(
            adapter=adapter, connection=connection, relation=relation, sql=first_delta
        )
    monkeypatch.undo()
    select_sql: str = (
        f"SELECT order_id, customer, amount, order_date FROM {relation} "
        "ORDER BY order_id, customer, amount"
    )
    after_crash: tuple[tuple[object, ...], ...] = fetch_rows(
        adapter=adapter, connection=connection, sql=select_sql
    )

    write_cursor_delta(
        adapter=adapter,
        connection=connection,
        relation=relation,
        sql=cursor_delta_sql(
            adapter=adapter, connection=connection, relation=relation, source=source
        ),
    )

    assert after_crash in test_case.expected_states_after_crash
    assert fetch_rows(adapter=adapter, connection=connection, sql=select_sql) == (
        test_case.expected_rows_after_rerun
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseConcurrentWindowTestCase(
            description="four two-week windows over two months",
            day_count=56,
            start_offsets=(0, 14, 28, 42),
            window_days=14,
            expected_error_count=0,
            expected_summary=((56, 56, 56),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_four_concurrent_windows_sharing_partitions_when_rewriting_then_no_update_is_lost(
    test_case: ClickHouseConcurrentWindowTestCase,
    adapter: ClickHouseAdapter,
    clickhouse_config: dict[str, object],
    connection: Any,
    clickhouse_schema: str,
) -> None:
    relation: str = f"{clickhouse_schema}.events"
    adapter.execute(
        connection=connection,
        sql=f"CREATE TABLE {relation} (event_id Int64, event_day Date, version Int64) "
        "ENGINE = MergeTree PARTITION BY toYYYYMM(event_day) ORDER BY event_id",
    )
    adapter.execute(
        connection=connection,
        sql=f"INSERT INTO {relation} SELECT number, toDate('2026-01-01') + number, 0 "
        f"FROM numbers({test_case.day_count})",
    )

    errors: list[BaseException] = run_concurrent_cursor_windows(
        adapter=adapter,
        config=clickhouse_config,
        relation=relation,
        first_day=date(2026, 1, 1),
        start_offsets=test_case.start_offsets,
        window_days=test_case.window_days,
    )

    assert len(errors) == test_case.expected_error_count, errors
    assert fetch_rows(
        adapter=adapter,
        connection=connection,
        sql=f"SELECT count(), sum(version), uniqExact(event_id) FROM {relation}",
    ) == (test_case.expected_summary)


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseCloneTestCase(
            description="monthly partitions survive the clone",
            partition_by=_PARTITIONED,
            expected_summary=((5, 150),),
            expected_partition_key=(("toYYYYMM(order_date)",),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_table_when_cloning_then_clickhouse_hard_links_parts(
    test_case: ClickHouseCloneTestCase,
    adapter: ClickHouseAdapter,
    connection: Any,
    clickhouse_schema: str,
) -> None:
    origin: str = f"{clickhouse_schema}.orders"
    clone: str = f"{clickhouse_schema}.orders_clone"
    create_orders(
        adapter=adapter,
        connection=connection,
        relation=origin,
        partition_by=test_case.partition_by,
        rows=INITIAL_ORDERS,
    )

    adapter.durable_clone(
        connection=connection,
        origin=origin,
        destination=clone,
        statement_recorder=StatementRecorder(),
    )
    adapter.drop(connection=connection, destination=origin, statement_recorder=StatementRecorder())

    assert fetch_rows(
        adapter=adapter, connection=connection, sql=f"SELECT count(), sum(amount) FROM {clone}"
    ) == (test_case.expected_summary)
    partition_key: tuple[tuple[object, ...], ...] = fetch_rows(
        adapter=adapter,
        connection=connection,
        sql="SELECT partition_key FROM system.tables "
        f"WHERE database = '{clickhouse_schema}' AND name = 'orders_clone'",
    )
    assert partition_key == test_case.expected_partition_key


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseFunctionTestCase(
            description="SQL lambda named after its schema",
            arguments=(("amount", "Int64"), ("rate", "Float64")),
            returns="Float64",
            body_sql="amount * rate",
            call_sql="(10, 0.5)",
            expected_value=5.0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sql_function_when_creating_then_clickhouse_mangles_server_global_name(
    test_case: ClickHouseFunctionTestCase,
    adapter: ClickHouseAdapter,
    connection: Any,
    clickhouse_schema: str,
) -> None:
    destination: str = f"{clickhouse_schema}.discounted"
    statements: tuple[str, ...] = adapter.render_create_function(
        destination=destination,
        arguments=tuple(
            SimpleNamespace(name=name, type=kind) for name, kind in test_case.arguments
        ),
        returns=test_case.returns,
        body_sql=test_case.body_sql,
    )
    try:
        execute_all(adapter=adapter, connection=connection, statements=statements)
        call: str = adapter.render_udf_call(target=destination, call_suffix_sql=test_case.call_sql)

        assert fetch_rows(adapter=adapter, connection=connection, sql=f"SELECT {call}") == (
            (test_case.expected_value,),
        )
        assert [
            function.name
            for function in adapter.list_functions(
                connection=connection, database=None, schemas=(clickhouse_schema,)
            )
        ] == ["discounted"]
    finally:
        adapter.execute(
            connection=connection,
            sql=f"DROP FUNCTION IF EXISTS {clickhouse_schema}__discounted",
        )


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseRefusedFunctionTestCase(
            description="python function refused",
            body_sql="def score(): return 1.0",
            expected_code="A707",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_function_when_rendering_then_clickhouse_refuses_with_code(
    test_case: ClickHouseRefusedFunctionTestCase, adapter: ClickHouseAdapter
) -> None:
    with pytest.raises(AdapterUserError) as error:
        adapter.render_create_function(
            destination="sales.score",
            arguments=(),
            returns="Float64",
            body_sql=test_case.body_sql,
            language=FunctionLanguage.PYTHON,
        )

    assert error.value.code == test_case.expected_code


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseInterruptTestCase(
            description="slow scan is killed",
            sql="SELECT sleepEachRow(1) FROM numbers(30) SETTINGS max_block_size = 1, "
            "function_sleep_max_microseconds_per_block = 0",
            delay_seconds=1.5,
            expected_outcome=(True, True, 1),
            expected_max_seconds=20.0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_long_query_when_interrupting_then_clickhouse_kills_it(
    test_case: ClickHouseInterruptTestCase, adapter: ClickHouseAdapter, connection: Any
) -> None:
    interrupted, finished, error_count, seconds = interrupt_running_query(
        adapter=adapter,
        connection=connection,
        sql=test_case.sql,
        delay_seconds=test_case.delay_seconds,
    )

    assert (interrupted, finished, error_count) == test_case.expected_outcome
    assert seconds < test_case.expected_max_seconds


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseCursorFilterTestCase(
            description="sub-second row inside a UTC window",
            rows_sql="('2026-01-01 00:00:00.5'), ('2026-01-02 00:00:00')",
            start="2026-01-01T00:00:00+00:00",
            end="2026-01-01T12:00:00+00:00",
            expected_count=((1,),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cursor_values_when_filtering_then_clickhouse_parses_iso_timestamps(
    test_case: ClickHouseCursorFilterTestCase,
    adapter: ClickHouseAdapter,
    connection: Any,
    clickhouse_schema: str,
) -> None:
    relation: str = f"{clickhouse_schema}.events"
    adapter.execute(
        connection=connection,
        sql=f"CREATE TABLE {relation} (at DateTime64(6, 'UTC')) ENGINE = MergeTree ORDER BY at",
    )
    adapter.execute(
        connection=connection, sql=f"INSERT INTO {relation} VALUES {test_case.rows_sql}"
    )
    condition: str = adapter.build_cursor_filter(
        cursor_column="at",
        start_cursor=CursorValue(kind="timestamp", value=datetime.fromisoformat(test_case.start)),
        end_cursor=CursorValue(kind="timestamp", value=datetime.fromisoformat(test_case.end)),
    )

    assert fetch_rows(
        adapter=adapter,
        connection=connection,
        sql=f"SELECT count() FROM {relation} WHERE {condition}",
    ) == (test_case.expected_count)


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseColumnMigrationTestCase(
            description="add, widen, rename and drop in place",
            added=(ColumnInfo(name="note", type="String"),),
            widened=(ColumnInfo(name="amount", type="Int64"),),
            renamed=("customer", "customer_name"),
            dropped=("order_date",),
            expected_columns=(
                ColumnInfo(name="order_id", type="Int64"),
                ColumnInfo(name="customer_name", type="String"),
                ColumnInfo(name="amount", type="Int64"),
                ColumnInfo(name="note", type="Nullable(String)"),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_table_when_migrating_columns_then_clickhouse_alters_in_place(
    test_case: ClickHouseColumnMigrationTestCase,
    adapter: ClickHouseAdapter,
    connection: Any,
    clickhouse_schema: str,
) -> None:
    relation: str = f"{clickhouse_schema}.orders"
    adapter.execute(
        connection=connection,
        sql=f"CREATE TABLE {relation} (order_id Int64, customer String, amount Int32, "
        "order_date Date) ENGINE = MergeTree ORDER BY order_id",
    )
    recorder: StatementRecorder = StatementRecorder()

    adapter.add_columns(
        connection=connection,
        destination=relation,
        columns=test_case.added,
        statement_recorder=recorder,
    )
    adapter.alter_column_types(
        connection=connection,
        destination=relation,
        columns=test_case.widened,
        statement_recorder=recorder,
    )
    adapter.rename_column(
        connection=connection,
        destination=relation,
        old_name=test_case.renamed[0],
        new_name=test_case.renamed[1],
        statement_recorder=recorder,
    )
    adapter.drop_columns(
        connection=connection,
        destination=relation,
        column_names=test_case.dropped,
        statement_recorder=recorder,
    )

    assert adapter.describe_relation(connection=connection, relation=relation) == (
        test_case.expected_columns
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseStateTimestampTestCase(
            description="microbatch event times keep microseconds",
            created_at=datetime(2026, 1, 1, 0, 0, 0, 123456, tzinfo=UTC),
            expected_created_at=((datetime(2026, 1, 1, 0, 0, 0, 123456),),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_microbatch_event_when_publishing_state_then_clickhouse_keeps_microseconds(
    test_case: ClickHouseStateTimestampTestCase,
    adapter: ClickHouseAdapter,
    connection: Any,
    clickhouse_schema: str,
) -> None:
    adapter.execute(
        connection=connection,
        sql=adapter.render_create_microbatch_state_table_sql(
            database=None, schema=clickhouse_schema
        ),
    )

    adapter.execute(
        connection=connection,
        sql=build_microbatch_insert_many_sql(
            events=(
                microbatch_completion(schema=clickhouse_schema, created_at=test_case.created_at),
            ),
            render_qualified_name=adapter.render_qualified_name,
            timestamp_type=adapter.state_timestamp_type,
        ),
    )

    assert fetch_rows(
        adapter=adapter,
        connection=connection,
        sql=f"SELECT created_at FROM {clickhouse_schema}._sqlbuild_microbatches",
    ) == (test_case.expected_created_at)


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseSchemaDiffTestCase(
            description="nullability differences are type changes",
            left_ddl_columns="id Int64, amount Int64, note Nullable(String), code String",
            right_ddl_columns=(
                "id Int64, amount Nullable(Int64), note String, code LowCardinality(String)"
            ),
            expected_type_changes=(
                ("amount", "Int64", "Nullable(Int64)"),
                ("note", "Nullable(String)", "String"),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_relations_differing_in_nullability_when_diffing_schema_then_changes_are_reported(
    test_case: ClickHouseSchemaDiffTestCase,
    adapter: ClickHouseAdapter,
    connection: Any,
    clickhouse_schema: str,
) -> None:
    left: str = f"{clickhouse_schema}.left_orders"
    right: str = f"{clickhouse_schema}.right_orders"
    adapter.execute(
        connection=connection,
        sql=f"CREATE TABLE {left} ({test_case.left_ddl_columns}) ENGINE = MergeTree ORDER BY id",
    )
    adapter.execute(
        connection=connection,
        sql=f"CREATE TABLE {right} ({test_case.right_ddl_columns}) ENGINE = MergeTree ORDER BY id",
    )

    result: SchemaDiffResult = adapter.diff_schema(connection=connection, left=left, right=right)

    assert tuple(
        (before.name, before.type, after.type) for before, after in result.type_changed_columns
    ) == (test_case.expected_type_changes)


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseGenerationTestCase(
            description="one partition is replaced in place",
            incoming_rows=((1, "ada", 11, "2026-01-05"),),
            expected_generation_kept=True,
        ),
        ClickHouseGenerationTestCase(
            description="an exchanged copy keeps the generation",
            incoming_rows=((1, "ada", 11, "2026-01-05"), (6, "fa", 60, "2026-03-09")),
            expected_generation_kept=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_partition_rewrite_when_reading_generation_then_microbatch_scope_is_kept(
    test_case: ClickHouseGenerationTestCase,
    adapter: ClickHouseAdapter,
    connection: Any,
    clickhouse_schema: str,
) -> None:
    relation: str = f"{clickhouse_schema}.orders"
    create_orders(
        adapter=adapter,
        connection=connection,
        relation=relation,
        partition_by=_PARTITIONED,
        rows=INITIAL_ORDERS,
    )
    before: str | None = adapter.physical_relation_generation(
        connection=connection, database=None, schema=clickhouse_schema, name="orders"
    )

    write_cursor_delta(
        adapter=adapter,
        connection=connection,
        relation=relation,
        sql=incoming_rows_sql(test_case.incoming_rows),
    )

    after: str | None = adapter.physical_relation_generation(
        connection=connection, database=None, schema=clickhouse_schema, name="orders"
    )
    assert (before is not None and before == after) is test_case.expected_generation_kept


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseCopyGenerationTestCase(
            description=kind, copy_kind=kind, expected_copy_shares_generation=False
        )
        for kind in ("clone", "hard copy", "replace from relation", "migration stage")
    ],
    ids=lambda case: case.description,
)
def test_given_exchanged_table_when_copying_then_copy_gets_its_own_generation(
    test_case: ClickHouseCopyGenerationTestCase,
    adapter: ClickHouseAdapter,
    connection: Any,
    clickhouse_schema: str,
) -> None:
    relation: str = f"{clickhouse_schema}.orders"
    create_orders(
        adapter=adapter,
        connection=connection,
        relation=relation,
        partition_by=_PARTITIONED,
        rows=INITIAL_ORDERS,
    )
    write_cursor_delta(
        adapter=adapter,
        connection=connection,
        relation=relation,
        sql=incoming_rows_sql(((1, "ada", 11, "2026-01-05"), (6, "fa", 60, "2026-03-09"))),
    )
    origin_generation: str | None = adapter.physical_relation_generation(
        connection=connection, database=None, schema=clickhouse_schema, name="orders"
    )

    copy_relation(
        adapter=adapter,
        connection=connection,
        kind=test_case.copy_kind,
        origin=relation,
        destination=f"{clickhouse_schema}.orders_copy",
    )

    copy_generation: str | None = adapter.physical_relation_generation(
        connection=connection, database=None, schema=clickhouse_schema, name="orders_copy"
    )
    assert (copy_generation == origin_generation) is test_case.expected_copy_shares_generation
