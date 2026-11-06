from __future__ import annotations

import json
import subprocess
import sys
from contextlib import nullcontext
from decimal import Decimal
from pathlib import Path
from threading import Timer
from typing import Any

import duckdb
import pytest

from sqlbuild._native import InferredNullability, SeedCsvSettings
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.contract.models import (
    ColumnInfo,
    ExpressionInferenceProfile,
    QueryResult,
    RowDiffColumnResult,
    RowDiffResult,
    RowDiffSampleRow,
    RowDiffSampling,
    RowDiffTolerance,
    RowDiffTolerances,
    SchemaDiffResult,
)
from sqlbuild.adapter.contract.types import FunctionNullabilityRule
from sqlbuild.adapter.relations.main.cached_relation_exists import cached_relation_exists
from sqlbuild.adapter.relations.main.relation_metadata_cache_scope import (
    relation_metadata_cache_scope,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from tests.integration.src.sqlbuild.adapters.duckdb._test_types import (
    CompositeRowDiffSamplingTestCase,
    ConnectSettingsTestCase,
    ConnectTestCase,
    ContendedConnectTestCase,
    DeleteInsertTestCase,
    DiffRowsErrorTestCase,
    DiffRowsTestCase,
    DiffSchemaTestCase,
    DropTestCase,
    ExpressionNullabilityRuleTestCase,
    GetAllColumnsTestCase,
    GetColumnsTestCase,
    ListRelationsTestCase,
    LoadSeedTestCase,
    MaterializeTestCase,
    MergeTestCase,
    PhysicalRelationGenerationTestCase,
    PythonFunctionRegistrationTestCase,
    QualifiedViewDefinitionTestCase,
    QueryTestCase,
    RecorderWriteTestCase,
    RelationExistsTestCase,
    RenameTestCase,
    SearchPathCacheTestCase,
    StorageVersionConnectTestCase,
    SwapTestCase,
    TransactionalAtomicityTestCase,
    UserStorageVersionKeyTestCase,
    ViewSwapTestCase,
)
from tests.integration.src.sqlbuild.adapters.duckdb.helpers import count_relation_rows


@pytest.mark.parametrize(
    "test_case",
    [
        PhysicalRelationGenerationTestCase(
            description="generation marker survives reads and changes after recreation",
            expected_stable=True,
            expected_recreated_distinct=True,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_duckdb_relation_when_recreated_then_generation_identity_changes(
    test_case: PhysicalRelationGenerationTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    connection.execute("CREATE TABLE main.generation_target (id INTEGER)")
    initial: str | None = adapter.physical_relation_generation(
        connection=connection,
        database=None,
        schema="main",
        name="generation_target",
    )
    repeated: str | None = adapter.physical_relation_generation(
        connection=connection,
        database=None,
        schema="main",
        name="generation_target",
    )
    connection.execute("DROP TABLE main.generation_target")
    connection.execute("CREATE TABLE main.generation_target (id INTEGER)")
    recreated: str | None = adapter.physical_relation_generation(
        connection=connection,
        database=None,
        schema="main",
        name="generation_target",
    )

    assert (initial == repeated) is test_case.expected_stable
    assert (initial != recreated) is test_case.expected_recreated_distinct


@pytest.mark.parametrize(
    "test_case",
    [
        ExpressionNullabilityRuleTestCase(
            description="LOWER preserves non-null literal",
            function_name="LOWER",
            sql_expression="LOWER('READY')",
            rule_args=(InferredNullability.NON_NULL,),
            expected_nullability=InferredNullability.NON_NULL,
            expected_is_null=False,
        ),
        ExpressionNullabilityRuleTestCase(
            description="LOWER preserves nullable input",
            function_name="LOWER",
            sql_expression="LOWER(CAST(NULL AS VARCHAR))",
            rule_args=(InferredNullability.NULLABLE,),
            expected_nullability=InferredNullability.NULLABLE,
            expected_is_null=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_expression_rule_when_querying_then_duckdb_matches_nullability_expectation(
    test_case: ExpressionNullabilityRuleTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    profile: ExpressionInferenceProfile = adapter.expression_inference_profile()
    rule: FunctionNullabilityRule | None = profile.function_nullability_rule(
        test_case.function_name
    )
    assert rule is not None

    result: QueryResult = adapter.query(
        connection=connection,
        sql=f"SELECT {test_case.sql_expression} IS NULL AS is_null",
        limit=None,
    )

    assert rule(test_case.rule_args) == test_case.expected_nullability
    assert result.rows == ((test_case.expected_is_null,),)


@pytest.mark.parametrize(
    "test_case",
    [
        ConnectTestCase(
            description="connects with default in-memory database",
            config={},
            expected_connects=True,
        ),
        ConnectTestCase(
            description="connects with explicit memory database",
            config={"database": ":memory:"},
            expected_connects=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_config_when_connecting_then_returns_usable_connection(
    test_case: ConnectTestCase,
    adapter: DuckDbAdapter,
) -> None:
    connection: Any = adapter.connect(test_case.config)

    assert (connection is not None) == test_case.expected_connects
    adapter.close(connection)


@pytest.mark.parametrize(
    "test_case",
    [
        ContendedConnectTestCase(
            description="writer waits for a catalog reader",
            read_only=False,
            release_key="release_after_seconds",
            expected_connection=nullcontext(),
            expected_rows=[(23,)],
        ),
        ContendedConnectTestCase(
            description="catalog reader waits for a writer",
            read_only=True,
            release_key="release_after_seconds",
            expected_connection=nullcontext(),
            expected_rows=[(23,)],
        ),
        ContendedConnectTestCase(
            description="persistent contention remains an error",
            read_only=False,
            release_key="persistent_release_after_seconds",
            expected_connection=pytest.raises(duckdb.IOException, match="lock|already open"),
            expected_rows=[(23,)],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_catalog_contention_when_connecting_then_temporary_locks_wait_and_persistent_locks_fail(
    test_case: ContendedConnectTestCase, adapter: DuckDbAdapter, tmp_path: Path
) -> None:
    inputs: dict[str, Any] = json.loads(
        Path(
            "benchmarks/duckshop/edge_cases/orchestration/local_catalog_contention.json"
        ).read_text()
    )
    database: Path = tmp_path / "orders.duckdb"
    connection: Any = adapter.connect({"database": str(database)})
    adapter.execute(connection=connection, sql=inputs["setup_sql"])
    adapter.close(connection)
    with subprocess.Popen(
        [
            sys.executable,
            "-c",
            "import duckdb,sys; c=duckdb.connect(sys.argv[1],read_only=sys.argv[2]=='true'); "
            "print('ready',flush=True); sys.stdin.read(1); c.close()",
            str(database),
            str(not test_case.read_only).lower(),
        ],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        text=True,
    ) as holder:
        assert holder.stdout is not None
        assert holder.stdout.readline().strip() == "ready"
        release: Timer = Timer(
            inputs[test_case.release_key],
            holder.communicate,
            kwargs={"input": "\n", "timeout": 5},
        )
        release.start()
        try:
            with test_case.expected_connection:
                connection = adapter.connect(
                    {"database": str(database), "read_only": test_case.read_only}
                )
                try:
                    assert connection.execute(inputs["query"]).fetchall() == test_case.expected_rows
                finally:
                    adapter.close(connection)
        finally:
            release.join()
        assert holder.returncode == 0


@pytest.mark.parametrize(
    "test_case",
    [
        StorageVersionConnectTestCase(
            description="an existing default-storage database opens for local replay",
            expected_rows=[(1,)],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_existing_database_when_opening_for_local_replay_then_its_tables_read(
    test_case: StorageVersionConnectTestCase,
    adapter: DuckDbAdapter,
    tmp_path: Path,
) -> None:
    database: Path = tmp_path / "existing.duckdb"
    written: duckdb.DuckDBPyConnection = duckdb.connect(str(database))
    written.execute("CREATE TABLE orders AS SELECT 1 AS order_id")
    written.close()

    connection: Any = adapter.connect_local_replay(database)
    rows: list[tuple[int]] = connection.execute("SELECT order_id FROM orders").fetchall()
    adapter.close(connection)

    assert rows == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        UserStorageVersionKeyTestCase(
            description="a storage version in user config is ignored like any unknown key",
            config_key="storage_compatibility_version",
            expected_storage_version="v2.0.0+",
        ),
        UserStorageVersionKeyTestCase(
            description="an unknown connection key is ignored",
            config_key="unknown_option",
            expected_storage_version="v2.0.0+",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_user_config_with_storage_version_when_connecting_then_it_is_ignored(
    test_case: UserStorageVersionKeyTestCase,
    adapter: DuckDbAdapter,
    tmp_path: Path,
) -> None:
    database: Path = tmp_path / "warehouse.duckdb"

    connection: Any = adapter.connect({"database": str(database), test_case.config_key: "latest"})
    tags: list[tuple[dict[str, str]]] = connection.execute(
        "SELECT tags FROM duckdb_databases() WHERE database_name = 'warehouse'"
    ).fetchall()
    adapter.close(connection)

    assert tags == [({"storage_version": test_case.expected_storage_version},)]


@pytest.mark.parametrize(
    "test_case",
    [
        ConnectSettingsTestCase(
            description="applies settings from config",
            config={"database": ":memory:", "settings": {"threads": "1"}},
            expected_setting_value="1",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_config_with_settings_when_connecting_then_applies_settings(
    test_case: ConnectSettingsTestCase,
    adapter: DuckDbAdapter,
) -> None:
    connection: Any = adapter.connect(test_case.config)
    result: Any = connection.execute("SELECT current_setting('threads')").fetchone()

    assert str(result[0]) == test_case.expected_setting_value
    adapter.close(connection)


@pytest.mark.parametrize(
    "test_case",
    [
        QueryTestCase(
            description="returns rows and marks limit truncation",
            sql="SELECT * FROM (VALUES (1, 'alice'), (2, 'bob')) AS t(id, name) ORDER BY id",
            limit=1,
            expected_result=QueryResult(
                columns=("id", "name"),
                rows=((1, "alice"),),
                truncated=True,
                column_types=("INTEGER", "VARCHAR"),
            ),
        ),
        QueryTestCase(
            description="returns all rows without limit",
            sql="SELECT * FROM (VALUES (1), (2)) AS t(id) ORDER BY id",
            limit=None,
            expected_result=QueryResult(
                columns=("id",), rows=((1,), (2,)), column_types=("INTEGER",)
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sql_when_querying_then_returns_normalized_query_result(
    test_case: QueryTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    result: QueryResult = adapter.query(
        connection=connection, sql=test_case.sql, limit=test_case.limit
    )

    assert result == test_case.expected_result


@pytest.mark.parametrize(
    "test_case",
    [
        RelationExistsTestCase(
            description="returns true for existing table",
            setup_sql=("CREATE TABLE test_table (id INTEGER)",),
            database=None,
            schema=None,
            name="test_table",
            expected_exists=True,
        ),
        RelationExistsTestCase(
            description="returns false for non-existing table",
            setup_sql=(),
            database=None,
            schema=None,
            name="missing_table",
            expected_exists=False,
        ),
        RelationExistsTestCase(
            description="an unqualified name off the search path does not resolve",
            setup_sql=("CREATE SCHEMA staging", "CREATE TABLE staging.raw_orders (id INTEGER)"),
            database=None,
            schema=None,
            name="raw_orders",
            expected_exists=False,
        ),
        RelationExistsTestCase(
            description="an unqualified name on the search path resolves",
            setup_sql=(
                "CREATE SCHEMA staging",
                "CREATE TABLE staging.raw_orders (id INTEGER)",
                "SET search_path = 'staging'",
            ),
            database=None,
            schema=None,
            name="raw_orders",
            expected_exists=True,
        ),
        RelationExistsTestCase(
            description="a search path naming the schema in another case resolves as DuckDB does",
            setup_sql=(
                "CREATE SCHEMA staging",
                "CREATE TABLE staging.raw_orders (id INTEGER)",
                "SET search_path = 'STAGING'",
            ),
            database=None,
            schema=None,
            name="raw_orders",
            expected_exists=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_relation_state_when_checking_exists_then_returns_expected(
    test_case: RelationExistsTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    statement: str
    for statement in test_case.setup_sql:
        connection.execute(statement)

    result: bool = adapter.relation_exists(
        connection=connection,
        database=test_case.database,
        schema=test_case.schema,
        name=test_case.name,
    )

    assert result == test_case.expected_exists


@pytest.mark.parametrize(
    "test_case",
    [
        ListRelationsTestCase(
            description="lists only relations in the requested schemas",
            setup_sql=(
                "CREATE TABLE orders (id INTEGER)",
                "CREATE VIEW orders_view AS SELECT id FROM orders",
                "CREATE SCHEMA other_schema",
                "CREATE TABLE other_schema.hidden (id INTEGER)",
            ),
            database=None,
            schemas=("main",),
            names=None,
            expected_names=("orders", "orders_view"),
        ),
        ListRelationsTestCase(
            description="lists relations across multiple requested schemas",
            setup_sql=(
                "CREATE TABLE orders (id INTEGER)",
                "CREATE SCHEMA staging",
                "CREATE TABLE staging.raw_orders (id INTEGER)",
                "CREATE SCHEMA excluded",
                "CREATE TABLE excluded.hidden (id INTEGER)",
            ),
            database=None,
            schemas=("main", "staging"),
            names=None,
            expected_names=("orders", "raw_orders"),
        ),
        ListRelationsTestCase(
            description="lists only requested relation names across schemas",
            setup_sql=(
                "CREATE TABLE orders (id INTEGER)",
                "CREATE TABLE customers (id INTEGER)",
                "CREATE SCHEMA staging",
                "CREATE TABLE staging.raw_orders (id INTEGER)",
            ),
            database=None,
            schemas=("main", "staging"),
            names=("orders", "raw_orders"),
            expected_names=("orders", "raw_orders"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_schema_with_relations_when_listing_then_returns_expected_names(
    test_case: ListRelationsTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    statement: str
    for statement in test_case.setup_sql:
        connection.execute(statement)

    relations: tuple[Any, ...] = adapter.list_relations(
        connection=connection,
        database=test_case.database,
        schemas=test_case.schemas,
        names=test_case.names,
    )
    names: tuple[str, ...] = tuple(r.name for r in relations)

    assert names == test_case.expected_names


@pytest.mark.parametrize(
    "test_case",
    [
        GetColumnsTestCase(
            description="returns columns with types in ordinal order",
            setup_sql=("CREATE TABLE typed_table (id INTEGER, name VARCHAR, active BOOLEAN)",),
            database=None,
            schema=None,
            name="typed_table",
            expected_columns=(
                ColumnInfo(name="id", type="INTEGER"),
                ColumnInfo(name="name", type="VARCHAR"),
                ColumnInfo(name="active", type="BOOLEAN"),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_table_when_getting_columns_then_returns_typed_column_info(
    test_case: GetColumnsTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    statement: str
    for statement in test_case.setup_sql:
        connection.execute(statement)

    columns: tuple[ColumnInfo, ...] = adapter.get_columns(
        connection=connection,
        database=test_case.database,
        schema=test_case.schema,
        name=test_case.name,
    )

    assert columns == test_case.expected_columns


@pytest.mark.parametrize(
    "test_case",
    [
        GetAllColumnsTestCase(
            description="returns all columns grouped by table name",
            setup_sql=(
                "CREATE TABLE t1 (a INTEGER, b VARCHAR)",
                "CREATE TABLE t2 (x BOOLEAN)",
            ),
            database=None,
            schemas=("main",),
            names=None,
            expected_columns_by_table={
                "t1": (ColumnInfo(name="a", type="INTEGER"), ColumnInfo(name="b", type="VARCHAR")),
                "t2": (ColumnInfo(name="x", type="BOOLEAN"),),
            },
        ),
        GetAllColumnsTestCase(
            description="returns only columns for requested table names",
            setup_sql=(
                "CREATE TABLE t1 (a INTEGER, b VARCHAR)",
                "CREATE TABLE t2 (x BOOLEAN)",
            ),
            database=None,
            schemas=("main",),
            names=("t1",),
            expected_columns_by_table={
                "t1": (ColumnInfo(name="a", type="INTEGER"), ColumnInfo(name="b", type="VARCHAR")),
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_schema_with_tables_when_getting_all_columns_then_returns_grouped(
    test_case: GetAllColumnsTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    statement: str
    for statement in test_case.setup_sql:
        connection.execute(statement)

    all_columns: dict[str, tuple[ColumnInfo, ...]] = adapter.get_all_columns(
        connection=connection,
        database=test_case.database,
        schemas=test_case.schemas,
        names=test_case.names,
    )

    assert frozenset(all_columns.keys()) == frozenset(test_case.expected_columns_by_table.keys())
    table_name: str
    expected_cols: tuple[ColumnInfo, ...]
    for table_name, expected_cols in test_case.expected_columns_by_table.items():
        assert all_columns[table_name] == expected_cols


@pytest.mark.parametrize(
    "test_case",
    [
        MaterializeTestCase(
            description="creates writable table from select",
            setup_sql=(),
            expected_row_count=4,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sql_when_creating_table_as_then_table_is_writable(
    test_case: MaterializeTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    adapter.create_table_as(
        connection=connection,
        destination="result",
        sql="SELECT * FROM (VALUES (1), (2), (3)) AS t(id)",
        statement_recorder=StatementRecorder(),
    )
    connection.execute("INSERT INTO result VALUES (4)")
    count: int = count_relation_rows(adapter=adapter, connection=connection, relation="result")

    assert count == test_case.expected_row_count


@pytest.mark.parametrize(
    "test_case",
    [
        RecorderWriteTestCase(
            description="create_table_as records rendered statement before execution",
            setup_sql=(),
            operation="create_table_as",
            target="recorded_table",
            sql="SELECT * FROM (VALUES (1), (2)) AS t(id)",
            expected_recorded_statements=(
                "CREATE OR REPLACE TABLE recorded_table AS SELECT * FROM "
                "(VALUES (1), (2)) AS t(id)",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_statement_recorder_when_creating_table_then_records_expected_sql(
    test_case: RecorderWriteTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    statement: str
    for statement in test_case.setup_sql:
        connection.execute(statement)

    recorder: StatementRecorder = StatementRecorder()

    adapter.create_table_as(
        connection=connection,
        destination=test_case.target,
        sql=test_case.sql,
        statement_recorder=recorder,
    )

    actual_statements: tuple[str, ...] = tuple(event.content for event in recorder.snapshot())
    assert actual_statements == test_case.expected_recorded_statements


@pytest.mark.parametrize(
    "test_case",
    [
        RecorderWriteTestCase(
            description="delete_insert records delete and insert statements before execution",
            setup_sql=(
                "CREATE TABLE recorded_delete_insert (id INTEGER, val VARCHAR)",
                "INSERT INTO recorded_delete_insert VALUES (1, 'old'), (2, 'keep')",
            ),
            operation="delete_insert",
            target="recorded_delete_insert",
            sql="SELECT * FROM (VALUES (1, 'new')) AS t(id, val)",
            expected_recorded_statements=(
                "DELETE FROM recorded_delete_insert WHERE EXISTS "
                "(SELECT 1 FROM (SELECT * FROM (VALUES (1, 'new')) AS t(id, val)) "
                'AS __source WHERE recorded_delete_insert."id" = __source."id")',
                "INSERT INTO recorded_delete_insert SELECT * FROM "
                "(VALUES (1, 'new')) AS t(id, val)",
            ),
            unique_key="id",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_statement_recorder_when_delete_inserting_then_records_expected_sql(
    test_case: RecorderWriteTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    statement: str
    for statement in test_case.setup_sql:
        connection.execute(statement)

    recorder: StatementRecorder = StatementRecorder()

    adapter.delete_insert(
        connection=connection,
        destination=test_case.target,
        sql=test_case.sql,
        unique_key=test_case.unique_key or "id",
        statement_recorder=recorder,
    )

    actual_statements: tuple[str, ...] = tuple(event.content for event in recorder.snapshot())
    assert actual_statements == test_case.expected_recorded_statements


@pytest.mark.parametrize(
    "test_case",
    [
        MaterializeTestCase(
            description="creates view from select",
            setup_sql=(
                "CREATE TABLE source (id INTEGER)",
                "INSERT INTO source VALUES (1), (2)",
            ),
            expected_row_count=2,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_table_when_creating_view_then_view_reflects_source(
    test_case: MaterializeTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    statement: str
    for statement in test_case.setup_sql:
        connection.execute(statement)
    adapter.create_view_as(
        connection=connection,
        destination="result_view",
        sql="SELECT id FROM source",
        statement_recorder=StatementRecorder(),
    )
    count: int = count_relation_rows(adapter=adapter, connection=connection, relation="result_view")

    assert count == test_case.expected_row_count


@pytest.mark.parametrize(
    "test_case",
    [
        DropTestCase(
            description="drops existing table",
            setup_sql=("CREATE TABLE to_drop (id INTEGER)",),
            target="to_drop",
            expected_exists=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_existing_table_when_dropping_then_table_no_longer_exists(
    test_case: DropTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    statement: str
    for statement in test_case.setup_sql:
        connection.execute(statement)
    adapter.drop(
        connection=connection, destination=test_case.target, statement_recorder=StatementRecorder()
    )

    result: bool = adapter.relation_exists(
        connection=connection, database=None, schema=None, name=test_case.target
    )
    assert result == test_case.expected_exists


@pytest.mark.parametrize(
    "test_case",
    [
        RenameTestCase(
            description="renames table",
            setup_sql=(
                "CREATE TABLE original (id INTEGER)",
                "INSERT INTO original VALUES (1)",
            ),
            source="original",
            target="renamed",
            expected_source_exists=False,
            expected_target_exists=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_existing_table_when_renaming_then_new_name_exists(
    test_case: RenameTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    statement: str
    for statement in test_case.setup_sql:
        connection.execute(statement)
    adapter.rename(
        connection=connection,
        origin=test_case.source,
        destination=test_case.target,
        statement_recorder=StatementRecorder(),
    )

    source_exists: bool = adapter.relation_exists(
        connection=connection, database=None, schema=None, name=test_case.source
    )
    target_exists: bool = adapter.relation_exists(
        connection=connection, database=None, schema=None, name=test_case.target
    )
    assert source_exists == test_case.expected_source_exists
    assert target_exists == test_case.expected_target_exists


@pytest.mark.parametrize(
    "test_case",
    [
        RenameTestCase(
            description="move-or-copy uses native rename within one schema",
            setup_sql=(
                "CREATE SCHEMA move_native",
                "CREATE TABLE move_native.original (id INTEGER)",
                "INSERT INTO move_native.original VALUES (1)",
            ),
            source="move_native.original",
            target="move_native.renamed",
            expected_source_exists=False,
            expected_target_exists=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_same_schema_table_when_moving_without_copy_then_native_rename_is_used(
    test_case: RenameTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    statement: str
    for statement in test_case.setup_sql:
        connection.execute(statement)

    adapter.move_or_copy_relation(
        connection=connection,
        origin=test_case.source,
        destination=test_case.target,
        remove_origin=True,
        allow_copy_fallback=False,
        statement_recorder=StatementRecorder(),
    )

    source_exists: bool = adapter.relation_exists(
        connection=connection, database=None, schema="move_native", name="original"
    )
    target_exists: bool = adapter.relation_exists(
        connection=connection, database=None, schema="move_native", name="renamed"
    )
    assert source_exists == test_case.expected_source_exists
    assert target_exists == test_case.expected_target_exists


@pytest.mark.parametrize(
    "test_case",
    [
        SwapTestCase(
            description="swaps two tables",
            setup_sql=(
                "CREATE TABLE left_t (val VARCHAR)",
                "INSERT INTO left_t VALUES ('left')",
                "CREATE TABLE right_t (val VARCHAR)",
                "INSERT INTO right_t VALUES ('right')",
            ),
            expected_left_value="right",
            expected_right_value="left",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_two_tables_when_swapping_then_contents_are_exchanged(
    test_case: SwapTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    statement: str
    for statement in test_case.setup_sql:
        connection.execute(statement)
    adapter.swap(
        connection=connection,
        left="left_t",
        right="right_t",
        statement_recorder=StatementRecorder(),
    )
    left_val: Any = connection.execute("SELECT val FROM left_t").fetchone()
    right_val: Any = connection.execute("SELECT val FROM right_t").fetchone()

    assert left_val[0] == test_case.expected_left_value
    assert right_val[0] == test_case.expected_right_value


@pytest.mark.parametrize(
    "test_case",
    [
        ViewSwapTestCase(
            description="an unqualified view is refused and left in place",
            setup_sql=(
                "CREATE VIEW orders AS SELECT 1 AS order_id",
                "CREATE TABLE orders__staging AS SELECT 2 AS order_id",
            ),
            left="orders",
            expected_relations=[("orders", "VIEW"), ("orders__staging", "BASE TABLE")],
        ),
        ViewSwapTestCase(
            description="a schema-qualified view is refused and left in place",
            setup_sql=(
                "CREATE VIEW main.orders AS SELECT 1 AS order_id",
                "CREATE TABLE main.orders__staging AS SELECT 2 AS order_id",
            ),
            left="main.orders",
            expected_relations=[("orders", "VIEW"), ("orders__staging", "BASE TABLE")],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_view_on_left_when_swapping_then_swap_is_refused_without_renaming(
    test_case: ViewSwapTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    statement: str
    for statement in test_case.setup_sql:
        connection.execute(statement)

    with pytest.raises(AdapterUserError, match="cannot swap the view"):
        adapter.swap(
            connection=connection,
            left=test_case.left,
            right="main.orders__staging",
            statement_recorder=StatementRecorder(),
        )

    relations: list[tuple[str, str]] = connection.execute(
        "SELECT table_name, table_type FROM information_schema.tables ORDER BY table_name"
    ).fetchall()
    assert relations == test_case.expected_relations


@pytest.mark.parametrize(
    "test_case",
    [
        MaterializeTestCase(
            description="clones table contents",
            setup_sql=(
                "CREATE TABLE source_t (id INTEGER)",
                "INSERT INTO source_t VALUES (1), (2), (3)",
            ),
            expected_row_count=3,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_table_when_cloning_then_target_has_same_rows(
    test_case: MaterializeTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    statement: str
    for statement in test_case.setup_sql:
        connection.execute(statement)
    adapter.clone(
        connection=connection,
        origin="source_t",
        destination="cloned_t",
        statement_recorder=StatementRecorder(),
    )
    count: int = count_relation_rows(adapter=adapter, connection=connection, relation="cloned_t")

    assert count == test_case.expected_row_count


@pytest.mark.parametrize(
    "test_case",
    [
        MaterializeTestCase(
            description="appends rows to existing table",
            setup_sql=(
                "CREATE TABLE append_t (id INTEGER)",
                "INSERT INTO append_t VALUES (1)",
            ),
            expected_row_count=3,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_existing_table_when_appending_then_row_count_increases(
    test_case: MaterializeTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    statement: str
    for statement in test_case.setup_sql:
        connection.execute(statement)
    adapter.append(
        connection=connection,
        destination="append_t",
        sql="SELECT * FROM (VALUES (2), (3)) AS t(id)",
        statement_recorder=StatementRecorder(),
    )
    count: int = count_relation_rows(adapter=adapter, connection=connection, relation="append_t")

    assert count == test_case.expected_row_count


@pytest.mark.parametrize(
    "test_case",
    [
        DeleteInsertTestCase(
            description="delete inserts by unique key",
            setup_sql=(
                "CREATE TABLE di_target (id INTEGER, val VARCHAR)",
                "INSERT INTO di_target VALUES (1, 'old'), (2, 'keep')",
            ),
            sql="SELECT * FROM (VALUES (1, 'new')) AS t(id, val)",
            unique_key="id",
            expected_row_count=2,
            expected_updated_value="new",
        ),
        DeleteInsertTestCase(
            description="removes duplicate target rows when source is deduplicated",
            setup_sql=(
                "CREATE TABLE di_target (id INTEGER, val VARCHAR)",
                "INSERT INTO di_target VALUES (1, 'dup_a'), (1, 'dup_b'), (2, 'keep')",
            ),
            sql="SELECT * FROM (VALUES (1, 'fixed')) AS t(id, val)",
            unique_key="id",
            expected_row_count=2,
            expected_updated_value="fixed",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_target_when_delete_inserting_then_matching_rows_replaced(
    test_case: DeleteInsertTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    statement: str
    for statement in test_case.setup_sql:
        connection.execute(statement)
    adapter.delete_insert(
        connection=connection,
        destination="di_target",
        sql=test_case.sql,
        unique_key=test_case.unique_key,
        statement_recorder=StatementRecorder(),
    )
    count: int = count_relation_rows(adapter=adapter, connection=connection, relation="di_target")
    updated_val: Any = connection.execute("SELECT val FROM di_target WHERE id = 1").fetchone()

    assert count == test_case.expected_row_count
    assert updated_val[0] == test_case.expected_updated_value


@pytest.mark.parametrize(
    "test_case",
    [
        MergeTestCase(
            description="inserts new rows and updates existing via merge",
            setup_sql=(
                "CREATE TABLE merge_target (id INTEGER, name VARCHAR)",
                "INSERT INTO merge_target VALUES (1, 'alice'), (2, 'bob')",
            ),
            source_sql="SELECT * FROM (VALUES (2, 'robert'), (3, 'charlie')) AS t(id, name)",
            unique_key="id",
            expected_row_count=3,
            expected_values=((1, "alice"), (2, "robert"), (3, "charlie")),
        ),
        MergeTestCase(
            description="preserves duplicate target rows that upsert cannot remove",
            setup_sql=(
                "CREATE TABLE merge_target (id INTEGER, name VARCHAR)",
                "INSERT INTO merge_target VALUES (1, 'dup_a'), (1, 'dup_b'), (2, 'bob')",
            ),
            source_sql="SELECT * FROM (VALUES (1, 'fixed')) AS t(id, name)",
            unique_key="id",
            expected_row_count=3,
            expected_values=((1, "fixed"), (1, "fixed"), (2, "bob")),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_target_and_source_when_merging_then_upserts_correctly(
    test_case: MergeTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    statement: str
    for statement in test_case.setup_sql:
        connection.execute(statement)
    adapter.merge(
        connection=connection,
        destination="merge_target",
        sql=test_case.source_sql,
        unique_key=test_case.unique_key,
        statement_recorder=StatementRecorder(),
    )
    count: int = count_relation_rows(
        adapter=adapter, connection=connection, relation="merge_target"
    )
    rows: list[tuple[Any, ...]] = connection.execute(
        "SELECT id, name FROM merge_target ORDER BY id, name"
    ).fetchall()

    assert count == test_case.expected_row_count
    assert tuple(tuple(r) for r in rows) == test_case.expected_values


@pytest.mark.parametrize(
    "test_case",
    [
        LoadSeedTestCase(
            description="loads csv with explicit column types",
            csv_content="id,name\n1,alice\n2,bob\n",
            columns=(
                ColumnInfo(name="id", type="INTEGER"),
                ColumnInfo(name="name", type="VARCHAR"),
            ),
            infer_types=False,
            expected_row_count=2,
            expected_first_row=(1, "alice"),
        ),
        LoadSeedTestCase(
            description="loads csv with inferred types",
            csv_content="id,name\n1,alice\n2,bob\n",
            columns=(),
            infer_types=True,
            expected_row_count=2,
            expected_first_row=(1, "alice"),
        ),
        LoadSeedTestCase(
            description="loads csv with custom delimiter",
            csv_content="id|name\n1|alice\n2|bob\n",
            columns=(
                ColumnInfo(name="id", type="INTEGER"),
                ColumnInfo(name="name", type="VARCHAR"),
            ),
            infer_types=False,
            expected_row_count=2,
            expected_first_row=(1, "alice"),
            csv_settings=SeedCsvSettings(delimiter="|"),
            expected_recorded_fragment="delim='|'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_csv_file_when_loading_seed_twice_then_table_is_replaced(
    test_case: LoadSeedTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
    tmp_path: Path,
) -> None:
    csv_path: Path = tmp_path / "seed.csv"
    csv_path.write_text(test_case.csv_content, encoding="utf-8")
    first_recorder: StatementRecorder = StatementRecorder()
    adapter.load_seed(
        connection=connection,
        destination="seed_table",
        file_path=csv_path,
        columns=test_case.columns,
        csv_settings=test_case.csv_settings,
        infer_types=test_case.infer_types,
        statement_recorder=first_recorder,
    )
    adapter.load_seed(
        connection=connection,
        destination="seed_table",
        file_path=csv_path,
        columns=test_case.columns,
        csv_settings=test_case.csv_settings,
        replace=True,
        infer_types=test_case.infer_types,
        statement_recorder=StatementRecorder(),
    )
    count: int = count_relation_rows(adapter=adapter, connection=connection, relation="seed_table")
    first_row: tuple[Any, ...] = connection.execute(
        "SELECT * FROM seed_table ORDER BY id LIMIT 1"
    ).fetchone()

    assert count == test_case.expected_row_count
    assert tuple(first_row) == test_case.expected_first_row
    assert test_case.expected_recorded_fragment in "\n".join(
        event.content for event in first_recorder.snapshot()
    )


@pytest.mark.parametrize(
    "test_case",
    [
        DiffSchemaTestCase(
            description="detects added removed and type changed columns",
            left_sql="CREATE TABLE left_t (id INTEGER, name VARCHAR, old_col BOOLEAN)",
            right_sql="CREATE TABLE right_t (id BIGINT, name VARCHAR, new_col DATE)",
            expected_result=SchemaDiffResult(
                added_columns=(ColumnInfo(name="new_col", type="DATE"),),
                removed_columns=(ColumnInfo(name="old_col", type="BOOLEAN"),),
                type_changed_columns=(
                    (
                        ColumnInfo(name="id", type="INTEGER"),
                        ColumnInfo(name="id", type="BIGINT"),
                    ),
                ),
            ),
        ),
        DiffSchemaTestCase(
            description="returns empty result for identical schemas",
            left_sql="CREATE TABLE left_t (id INTEGER, name VARCHAR)",
            right_sql="CREATE TABLE right_t (id INTEGER, name VARCHAR)",
            expected_result=SchemaDiffResult(),
        ),
        DiffSchemaTestCase(
            description="ignores equivalent scalar aliases and detects numeric scale changes",
            left_sql=(
                "CREATE TABLE left_t ("
                "id INTEGER, flag BOOLEAN, amount DECIMAL(10,2), widened DECIMAL(10,2))"
            ),
            right_sql=(
                "CREATE TABLE right_t (id INT, flag BOOL, amount NUMERIC(10,2), widened NUMERIC(10,3))"
            ),
            expected_result=SchemaDiffResult(
                type_changed_columns=(
                    (
                        ColumnInfo(name="widened", type="DECIMAL(10,2)"),
                        ColumnInfo(name="widened", type="DECIMAL(10,3)"),
                    ),
                ),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_two_tables_when_diffing_schema_then_returns_expected_differences(
    test_case: DiffSchemaTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    connection.execute(test_case.left_sql)
    connection.execute(test_case.right_sql)

    result: SchemaDiffResult = adapter.diff_schema(
        connection=connection, left="left_t", right="right_t"
    )

    assert result == test_case.expected_result


@pytest.mark.parametrize(
    "test_case",
    [
        DiffRowsTestCase(
            description="detects matching mismatched and missing rows",
            left_sql=(
                "CREATE TABLE left_t AS SELECT * FROM "
                "(VALUES (1, 'a'), (2, 'b'), (3, 'c')) AS t(id, val)"
            ),
            right_sql=(
                "CREATE TABLE right_t AS SELECT * FROM "
                "(VALUES (1, 'a'), (2, 'x'), (4, 'd')) AS t(id, val)"
            ),
            unique_key="id",
            expected_result=RowDiffResult(
                left_count=3,
                right_count=3,
                joined_count=4,
                equal_count=1,
                unequal_count=1,
                left_only_count=1,
                right_only_count=1,
                column_results=(RowDiffColumnResult(name="val", mismatched_count=1),),
            ),
        ),
        DiffRowsTestCase(
            description="excludes columns from comparison",
            left_sql=(
                "CREATE TABLE left_t AS SELECT * FROM (VALUES (1, 'a', 'ignore1')) AS t(id, val, extra)"
            ),
            right_sql=(
                "CREATE TABLE right_t AS SELECT * FROM "
                "(VALUES (1, 'a', 'ignore2')) AS t(id, val, extra)"
            ),
            unique_key="id",
            excluded_columns=("extra",),
            expected_result=RowDiffResult(
                left_count=1,
                right_count=1,
                joined_count=1,
                equal_count=1,
                unequal_count=0,
                left_only_count=0,
                right_only_count=0,
                column_results=(RowDiffColumnResult(name="val", mismatched_count=0),),
            ),
        ),
        DiffRowsTestCase(
            description="allows absolute tolerance for numeric columns",
            left_sql="CREATE TABLE left_t AS SELECT * FROM (VALUES (1, 100.00)) AS t(id, amount)",
            right_sql="CREATE TABLE right_t AS SELECT * FROM (VALUES (1, 100.005)) AS t(id, amount)",
            unique_key="id",
            tolerances=RowDiffTolerances(
                by_column={"amount": RowDiffTolerance(absolute=Decimal("0.01"))},
            ),
            expected_result=RowDiffResult(
                left_count=1,
                right_count=1,
                joined_count=1,
                equal_count=1,
                unequal_count=0,
                left_only_count=0,
                right_only_count=0,
                column_results=(
                    RowDiffColumnResult(
                        name="amount",
                        mismatched_count=0,
                        tolerance=RowDiffTolerance(absolute=Decimal("0.01")),
                    ),
                ),
            ),
        ),
        DiffRowsTestCase(
            description="allows relative tolerance for float columns",
            left_sql=(
                "CREATE TABLE left_t AS SELECT * FROM "
                "(VALUES (1, CAST(1000000.0 AS DOUBLE))) AS t(id, metric)"
            ),
            right_sql=(
                "CREATE TABLE right_t AS SELECT * FROM "
                "(VALUES (1, CAST(1000050.0 AS DOUBLE))) AS t(id, metric)"
            ),
            unique_key="id",
            tolerances=RowDiffTolerances(
                by_type={"float": RowDiffTolerance(relative=Decimal("0.0001"))},
            ),
            expected_result=RowDiffResult(
                left_count=1,
                right_count=1,
                joined_count=1,
                equal_count=1,
                unequal_count=0,
                left_only_count=0,
                right_only_count=0,
                column_results=(
                    RowDiffColumnResult(
                        name="metric",
                        mismatched_count=0,
                        tolerance=RowDiffTolerance(relative=Decimal("0.0001")),
                    ),
                ),
            ),
        ),
        DiffRowsTestCase(
            description="reports numeric difference outside tolerance",
            left_sql="CREATE TABLE left_t AS SELECT * FROM (VALUES (1, 100.00)) AS t(id, amount)",
            right_sql="CREATE TABLE right_t AS SELECT * FROM (VALUES (1, 100.02)) AS t(id, amount)",
            unique_key="id",
            tolerances=RowDiffTolerances(
                by_column={"amount": RowDiffTolerance(absolute=Decimal("0.01"))},
            ),
            expected_result=RowDiffResult(
                left_count=1,
                right_count=1,
                joined_count=1,
                equal_count=0,
                unequal_count=1,
                left_only_count=0,
                right_only_count=0,
                column_results=(
                    RowDiffColumnResult(
                        name="amount",
                        mismatched_count=1,
                        tolerance=RowDiffTolerance(absolute=Decimal("0.01")),
                    ),
                ),
            ),
        ),
        DiffRowsTestCase(
            description="uses larger of absolute and relative tolerance",
            left_sql=(
                "CREATE TABLE left_t AS SELECT * FROM "
                "(VALUES (1, CAST(1000000.0 AS DOUBLE))) AS t(id, metric)"
            ),
            right_sql=(
                "CREATE TABLE right_t AS SELECT * FROM "
                "(VALUES (1, CAST(1000050.0 AS DOUBLE))) AS t(id, metric)"
            ),
            unique_key="id",
            tolerances=RowDiffTolerances(
                by_column={
                    "metric": RowDiffTolerance(
                        absolute=Decimal("0.01"),
                        relative=Decimal("0.0001"),
                    )
                },
            ),
            expected_result=RowDiffResult(
                left_count=1,
                right_count=1,
                joined_count=1,
                equal_count=1,
                unequal_count=0,
                left_only_count=0,
                right_only_count=0,
                column_results=(
                    RowDiffColumnResult(
                        name="metric",
                        mismatched_count=0,
                        tolerance=RowDiffTolerance(
                            absolute=Decimal("0.01"),
                            relative=Decimal("0.0001"),
                        ),
                    ),
                ),
            ),
        ),
        DiffRowsTestCase(
            description="keeps null versus value different with tolerance",
            left_sql=(
                "CREATE TABLE left_t AS SELECT * FROM "
                "(VALUES (1, CAST(NULL AS DOUBLE))) AS t(id, amount)"
            ),
            right_sql="CREATE TABLE right_t AS SELECT * FROM (VALUES (1, 100.00)) AS t(id, amount)",
            unique_key="id",
            tolerances=RowDiffTolerances(
                by_column={"amount": RowDiffTolerance(absolute=Decimal("1000"))},
            ),
            expected_result=RowDiffResult(
                left_count=1,
                right_count=1,
                joined_count=1,
                equal_count=0,
                unequal_count=1,
                left_only_count=0,
                right_only_count=0,
                column_results=(
                    RowDiffColumnResult(
                        name="amount",
                        mismatched_count=1,
                        tolerance=RowDiffTolerance(absolute=Decimal("1000")),
                    ),
                ),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_two_tables_when_diffing_rows_then_returns_expected_counts(
    test_case: DiffRowsTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    connection.execute(test_case.left_sql)
    connection.execute(test_case.right_sql)

    result: RowDiffResult = adapter.diff_rows(
        connection=connection,
        left="left_t",
        right="right_t",
        unique_key=test_case.unique_key,
        excluded_columns=test_case.excluded_columns,
        tolerances=test_case.tolerances,
        cursor_column=test_case.cursor_column,
        start_cursor=test_case.start_cursor,
        end_cursor=test_case.end_cursor,
    )

    assert result == test_case.expected_result


@pytest.mark.parametrize(
    "test_case",
    [
        CompositeRowDiffSamplingTestCase(
            description="length-prefixed composite keys have seeded stable membership",
            row_limit=3,
            seed=7,
            alternate_seed=11,
            expected_population_count=6,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_composite_key_population_when_sampling_then_membership_is_seeded_and_stable(
    test_case: CompositeRowDiffSamplingTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    connection.execute(
        "CREATE TABLE left_t AS SELECT * FROM (VALUES "
        "(1, '23', 'left-a'), (12, '3', 'left-b'), (2, '8', 'left-c'), "
        "(3, '5', 'left-d'), (4, '2', 'left-e'), (5, '1', 'left-f')"
        ") AS t(order_id, line_code, value)"
    )
    connection.execute(
        "CREATE TABLE right_t AS SELECT * FROM (VALUES "
        "(1, '23', 'right-a'), (12, '3', 'right-b'), (2, '8', 'right-c'), "
        "(3, '5', 'right-d'), (4, '2', 'right-e'), (5, '1', 'right-f')"
        ") AS t(order_id, line_code, value)"
    )
    sampling: RowDiffSampling = RowDiffSampling(
        row_limit=test_case.row_limit,
        seed=test_case.seed,
    )

    result: RowDiffResult = adapter.diff_rows(
        connection=connection,
        left="left_t",
        right="right_t",
        unique_key=("order_id", "line_code"),
        tolerances=RowDiffTolerances(sampling=sampling),
    )
    first_samples: tuple[RowDiffSampleRow, ...] = adapter.sample_unequal_rows(
        connection=connection,
        left="left_t",
        right="right_t",
        unique_key=("order_id", "line_code"),
        tolerances=RowDiffTolerances(sampling=sampling),
        limit=test_case.row_limit,
    )
    repeated_samples: tuple[RowDiffSampleRow, ...] = adapter.sample_unequal_rows(
        connection=connection,
        left="left_t",
        right="right_t",
        unique_key=("order_id", "line_code"),
        tolerances=RowDiffTolerances(sampling=sampling),
        limit=test_case.row_limit,
    )
    alternate_samples: tuple[RowDiffSampleRow, ...] = adapter.sample_unequal_rows(
        connection=connection,
        left="left_t",
        right="right_t",
        unique_key=("order_id", "line_code"),
        tolerances=RowDiffTolerances(
            sampling=RowDiffSampling(
                row_limit=test_case.row_limit,
                seed=test_case.alternate_seed,
            ),
        ),
        limit=test_case.row_limit,
    )

    assert result.population_count == test_case.expected_population_count
    assert result.compared_count == test_case.row_limit
    assert result.unequal_count == test_case.row_limit
    assert first_samples == repeated_samples
    assert first_samples != alternate_samples
    assert len(first_samples) == test_case.row_limit
    assert len(alternate_samples) == test_case.row_limit


@pytest.mark.parametrize(
    "test_case",
    [
        DiffRowsErrorTestCase(
            description="rejects tolerance for non numeric column",
            left_sql="CREATE TABLE left_t AS SELECT * FROM (VALUES (1, 'a')) AS t(id, status)",
            right_sql="CREATE TABLE right_t AS SELECT * FROM (VALUES (1, 'b')) AS t(id, status)",
            unique_key="id",
            tolerances=RowDiffTolerances(
                by_column={"status": RowDiffTolerance(absolute=Decimal("1"))},
            ),
            expected_error_fragment="row diff tolerance for non-numeric column 'status' is invalid",
        ),
        DiffRowsErrorTestCase(
            description="rejects empty tolerance",
            left_sql="CREATE TABLE left_t AS SELECT * FROM (VALUES (1, 100.0)) AS t(id, amount)",
            right_sql="CREATE TABLE right_t AS SELECT * FROM (VALUES (1, 100.0)) AS t(id, amount)",
            unique_key="id",
            tolerances=RowDiffTolerances(by_column={"amount": RowDiffTolerance()}),
            expected_error_fragment="must define absolute or relative",
        ),
        DiffRowsErrorTestCase(
            description="rejects null left unique key",
            left_sql=(
                "CREATE TABLE left_t AS SELECT * FROM "
                "(VALUES (CAST(NULL AS INTEGER), 'a')) AS t(id, status)"
            ),
            right_sql="CREATE TABLE right_t AS SELECT * FROM (VALUES (1, 'a')) AS t(id, status)",
            unique_key="id",
            expected_error_fragment="left relation contains null unique_key values",
        ),
        DiffRowsErrorTestCase(
            description="rejects duplicate right unique key",
            left_sql="CREATE TABLE left_t AS SELECT * FROM (VALUES (1, 'a')) AS t(id, status)",
            right_sql=(
                "CREATE TABLE right_t AS SELECT * FROM (VALUES (1, 'a'), (1, 'b')) AS t(id, status)"
            ),
            unique_key="id",
            expected_error_fragment="right relation contains duplicate unique_key values",
        ),
        DiffRowsErrorTestCase(
            description="rejects null composite unique key component",
            left_sql=(
                "CREATE TABLE left_t AS SELECT * FROM "
                "(VALUES (1, CAST(NULL AS INTEGER), 'a')) AS t(id, line_number, status)"
            ),
            right_sql=(
                "CREATE TABLE right_t AS SELECT * FROM "
                "(VALUES (1, 1, 'a')) AS t(id, line_number, status)"
            ),
            unique_key=("id", "line_number"),
            expected_error_fragment="left relation contains null unique_key values",
        ),
        DiffRowsErrorTestCase(
            description="rejects duplicate composite unique key",
            left_sql=(
                "CREATE TABLE left_t AS SELECT * FROM "
                "(VALUES (1, 1, 'a'), (1, 1, 'b')) AS t(id, line_number, status)"
            ),
            right_sql=(
                "CREATE TABLE right_t AS SELECT * FROM "
                "(VALUES (1, 1, 'a')) AS t(id, line_number, status)"
            ),
            unique_key=("id", "line_number"),
            expected_error_fragment="left relation contains duplicate unique_key values",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_tolerance_when_diffing_rows_then_raises_clear_error(
    test_case: DiffRowsErrorTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    connection.execute(test_case.left_sql)
    connection.execute(test_case.right_sql)

    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        adapter.diff_rows(
            connection=connection,
            left="left_t",
            right="right_t",
            unique_key=test_case.unique_key,
            tolerances=test_case.tolerances,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        TransactionalAtomicityTestCase(
            description="delete_insert rolls back delete when insert fails",
            setup_sql=(
                "CREATE TABLE atomic_di (id INTEGER NOT NULL, val VARCHAR NOT NULL)",
                "INSERT INTO atomic_di VALUES (1, 'keep'), (2, 'keep')",
            ),
            target="atomic_di",
            source_sql="SELECT * FROM (VALUES (1, NULL)) AS t(id, val)",
            unique_key="id",
            verify_sql="SELECT id, val FROM atomic_di ORDER BY id",
            expected_rows_after_failure=((1, "keep"), (2, "keep")),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failing_insert_when_delete_inserting_then_original_rows_preserved(
    test_case: TransactionalAtomicityTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    statement: str
    for statement in test_case.setup_sql:
        connection.execute(statement)

    with pytest.raises(duckdb.ConstraintException):
        adapter.delete_insert(
            connection=connection,
            destination=test_case.target,
            sql=test_case.source_sql,
            unique_key=test_case.unique_key,
            statement_recorder=StatementRecorder(),
        )

    actual: tuple[tuple[object, ...], ...] = tuple(
        connection.execute(test_case.verify_sql).fetchall()
    )
    assert actual == test_case.expected_rows_after_failure


@pytest.mark.parametrize(
    "test_case",
    [
        TransactionalAtomicityTestCase(
            description="delete_insert_cursor rolls back on NOT NULL failure",
            setup_sql=(
                "CREATE TABLE atomic_dic "
                "(id INTEGER NOT NULL, val VARCHAR NOT NULL, cursor_ts VARCHAR NOT NULL)",
                "INSERT INTO atomic_dic VALUES "
                "(1, 'keep', '2024-01-01'), (2, 'keep', '2024-01-02')",
            ),
            target="atomic_dic",
            source_sql="SELECT * FROM (VALUES (1, NULL, '2024-01-01')) AS t(id, val, cursor_ts)",
            unique_key="id",
            verify_sql="SELECT id, val FROM atomic_dic ORDER BY id",
            expected_rows_after_failure=((1, "keep"), (2, "keep")),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failing_insert_when_delete_insert_cursor_then_original_rows_preserved(
    test_case: TransactionalAtomicityTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    statement: str
    for statement in test_case.setup_sql:
        connection.execute(statement)

    with pytest.raises(duckdb.ConstraintException):
        adapter.delete_insert_cursor(
            connection=connection,
            destination=test_case.target,
            sql=test_case.source_sql,
            cursor_column="cursor_ts",
            cursor_start="2024-01-01",
            cursor_end="2024-01-03",
            statement_recorder=StatementRecorder(),
        )

    actual: tuple[tuple[object, ...], ...] = tuple(
        connection.execute(test_case.verify_sql).fetchall()
    )
    assert actual == test_case.expected_rows_after_failure


@pytest.mark.parametrize(
    "test_case",
    [
        TransactionalAtomicityTestCase(
            description="swap rolls back partial renames on failure",
            setup_sql=(
                "CREATE TABLE swap_left (val VARCHAR)",
                "INSERT INTO swap_left VALUES ('left')",
            ),
            target="swap_left",
            source_sql="",
            unique_key="",
            verify_sql="SELECT val FROM swap_left",
            expected_rows_after_failure=(("left",),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_missing_right_table_when_swapping_then_left_table_preserved(
    test_case: TransactionalAtomicityTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    statement: str
    for statement in test_case.setup_sql:
        connection.execute(statement)

    with pytest.raises(duckdb.CatalogException):
        adapter.swap(
            connection=connection,
            left="swap_left",
            right="swap_nonexistent",
            statement_recorder=StatementRecorder(),
        )

    actual: tuple[tuple[object, ...], ...] = tuple(
        connection.execute(test_case.verify_sql).fetchall()
    )
    assert actual == test_case.expected_rows_after_failure


@pytest.mark.parametrize(
    "test_case",
    [
        PythonFunctionRegistrationTestCase(
            description="a one-row aggregate result with its row read",
            open_result_sql="SELECT COUNT(*) FROM range(3)",
            call_sql="SELECT double_amount(21)",
            expected_rows=[(42,)],
        ),
        PythonFunctionRegistrationTestCase(
            description="a many-row result with one row read",
            open_result_sql="SELECT * FROM range(100)",
            call_sql="SELECT double_amount(5)",
            expected_rows=[(10,)],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_partly_read_result_when_registering_python_function_then_function_is_callable(
    test_case: PythonFunctionRegistrationTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    cursor: Any = connection.execute(test_case.open_result_sql)
    _ = cursor.fetchmany(1)

    adapter.register_python_function(
        connection=connection,
        name="double_amount",
        function=lambda amount: amount * 2,
        parameter_types=["BIGINT"],
        returns="BIGINT",
    )

    assert connection.execute(test_case.call_sql).fetchall() == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        QualifiedViewDefinitionTestCase(
            description="a view in an attached database matches its three-part SELECT",
            setup_sql=(
                "CREATE SCHEMA shop.sales",
                "CREATE TABLE shop.sales.orders AS SELECT 1 AS order_id",
                "CREATE VIEW shop.sales.old_orders AS SELECT * FROM shop.sales.orders",
            ),
            database="shop",
            schema="sales",
            name="old_orders",
            sql="SELECT * FROM shop.sales.orders",
            expected_match=True,
        ),
        QualifiedViewDefinitionTestCase(
            description="a quoted three-part SELECT matches too",
            setup_sql=(
                "CREATE SCHEMA shop.sales",
                "CREATE TABLE shop.sales.orders AS SELECT 1 AS order_id",
                "CREATE VIEW shop.sales.old_orders AS SELECT * FROM shop.sales.orders",
            ),
            database="shop",
            schema="sales",
            name="old_orders",
            sql='SELECT * FROM "shop"."sales"."orders"',
            expected_match=True,
        ),
        QualifiedViewDefinitionTestCase(
            description="a view in the default database matches with no database given",
            setup_sql=(
                "CREATE TABLE memory.main.orders AS SELECT 1 AS order_id",
                "CREATE VIEW memory.main.old_orders AS SELECT * FROM memory.main.orders",
            ),
            database=None,
            schema="main",
            name="old_orders",
            sql="SELECT * FROM memory.main.orders",
            expected_match=True,
        ),
        QualifiedViewDefinitionTestCase(
            description="a three-part SELECT of another relation still differs",
            setup_sql=(
                "CREATE SCHEMA shop.sales",
                "CREATE TABLE shop.sales.orders AS SELECT 1 AS order_id",
                "CREATE TABLE shop.sales.customers AS SELECT 1 AS customer_id",
                "CREATE VIEW shop.sales.old_orders AS SELECT * FROM shop.sales.orders",
            ),
            database="shop",
            schema="sales",
            name="old_orders",
            sql="SELECT * FROM shop.sales.customers",
            expected_match=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_database_qualified_view_when_comparing_definition_then_catalog_prefix_is_ignored(
    test_case: QualifiedViewDefinitionTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
    tmp_path: Path,
) -> None:
    connection.execute(f"ATTACH '{tmp_path / 'shop.duckdb'}' AS shop")
    statement: str
    for statement in test_case.setup_sql:
        connection.execute(statement)

    matches: bool = adapter.view_definition_matches(
        connection=connection,
        database=test_case.database,
        schema=test_case.schema,
        name=test_case.name,
        sql=test_case.sql,
    )

    assert matches is test_case.expected_match


@pytest.mark.parametrize(
    "test_case",
    [
        SearchPathCacheTestCase(
            description="a hook's SET search_path between two models changes the cached answer",
            hook_sql="SET search_path = 'staging'",
            expected_before=False,
            expected_after=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cached_unqualified_lookup_when_hook_sets_search_path_then_lookup_reads_again(
    test_case: SearchPathCacheTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    connection.execute("CREATE SCHEMA staging")
    connection.execute("CREATE TABLE staging.raw_orders (id INTEGER)")

    with relation_metadata_cache_scope(adapter=adapter):
        before: bool = cached_relation_exists(
            adapter=adapter, connection=connection, database=None, schema=None, name="raw_orders"
        )
        adapter.execute(connection=connection, sql=test_case.hook_sql)
        after: bool = cached_relation_exists(
            adapter=adapter, connection=connection, database=None, schema=None, name="raw_orders"
        )

    assert (before, after) == (test_case.expected_before, test_case.expected_after)
