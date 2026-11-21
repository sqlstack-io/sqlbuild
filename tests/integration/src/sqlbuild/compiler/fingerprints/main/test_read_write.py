from __future__ import annotations

import json
from collections.abc import Callable
from datetime import datetime
from typing import Any

import pytest

from sqlbuild._native import (
    FINGERPRINT_TABLE_NAME,
    Fingerprint,
    FingerprintSet,
    read_latest_fingerprints,
    select_latest_fingerprints,
    write_fingerprint,
    write_fingerprints,
)
from sqlbuild.adapter.contract.types import FrameworkType
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from tests.integration.src.sqlbuild.compiler.fingerprints.main._test_types import (
    BatchWriteTestCase,
    InvalidDefinitionStorageTestCase,
    LatestResolutionTestCase,
    OldFingerprintSchemaTestCase,
    PruneFingerprintHistoryTestCase,
    ReadNonExistentTableTestCase,
    SelectLatestTestCase,
    WriteAndReadTestCase,
    WriteCreatesTableTestCase,
)
from tests.integration.src.sqlbuild.compiler.fingerprints.main.helpers import (
    mixed_fingerprint_history,
)

RENDER_QUALIFIED_NAME: Callable[..., str | None] = DuckDbAdapter().render_qualified_name
RENDER_FRAMEWORK_TYPE: Callable[[FrameworkType], str] = DuckDbAdapter().render_framework_type
RENDER_READ_LATEST_SQL: Callable[..., str] = DuckDbAdapter().render_read_latest_fingerprints_sql
RELATION_EXISTS: Callable[..., bool] = DuckDbAdapter().relation_exists


@pytest.mark.parametrize(
    "test_case",
    [
        WriteAndReadTestCase(
            description="writes and reads back a single fingerprint",
            database=None,
            schema="test_schema",
            fingerprints=(
                Fingerprint(
                    node_type="model",
                    node_name="orders",
                    target_database=None,
                    target_schema=None,
                    target_name="orders",
                    run_id="run_001",
                    definition_hash="hash_a",
                    version_hash="version_a",
                    schema_fingerprint="schema_a",
                    definition="SELECT id FROM orders",
                    ts=datetime(2026, 1, 15, 12, 0, 0),
                ),
            ),
            expected_node_names=("orders",),
            expected_latest_definition_hashes={"orders": "hash_a"},
            expected_latest_target_names={"orders": "orders"},
            expected_latest_definitions={"orders": "SELECT id FROM orders"},
            expected_latest_version_hashes={"orders": "version_a"},
        ),
        WriteAndReadTestCase(
            description="writes and reads fingerprints for multiple models",
            database=None,
            schema="test_schema",
            fingerprints=(
                Fingerprint(
                    node_type="model",
                    node_name="orders",
                    target_database=None,
                    target_schema=None,
                    target_name="orders",
                    run_id="run_001",
                    definition_hash="hash_a",
                    version_hash="version_a",
                    schema_fingerprint="schema_a",
                    definition="SELECT id FROM orders",
                    ts=datetime(2026, 1, 15, 12, 0, 0),
                ),
                Fingerprint(
                    node_type="model",
                    node_name="customers",
                    target_database=None,
                    target_schema=None,
                    target_name="customers",
                    run_id="run_001",
                    definition_hash="hash_b",
                    version_hash="version_b",
                    schema_fingerprint="schema_b",
                    definition="SELECT id FROM customers",
                    ts=datetime(2026, 1, 15, 12, 0, 0),
                ),
            ),
            expected_node_names=("customers", "orders"),
            expected_latest_definition_hashes={"orders": "hash_a", "customers": "hash_b"},
            expected_latest_target_names={"orders": "orders", "customers": "customers"},
            expected_latest_definitions={
                "orders": "SELECT id FROM orders",
                "customers": "SELECT id FROM customers",
            },
            expected_latest_version_hashes={"orders": "version_a", "customers": "version_b"},
        ),
        WriteAndReadTestCase(
            description="writes and reads multiline query sql with quotes and backslashes",
            database=None,
            schema="test_schema",
            fingerprints=(
                Fingerprint(
                    node_type="model",
                    node_name="orders",
                    target_database=None,
                    target_schema=None,
                    target_name="orders",
                    run_id="run_001",
                    definition_hash="hash_a",
                    version_hash="version_a",
                    schema_fingerprint="schema_a",
                    definition="SELECT '\\n' AS slash_n\nFROM orders\nWHERE note = 'line\\nvalue'",
                    ts=datetime(2026, 1, 15, 12, 0, 0),
                ),
            ),
            expected_node_names=("orders",),
            expected_latest_definition_hashes={"orders": "hash_a"},
            expected_latest_target_names={"orders": "orders"},
            expected_latest_definitions={
                "orders": "SELECT '\\n' AS slash_n\nFROM orders\nWHERE note = 'line\\nvalue'"
            },
            expected_latest_version_hashes={"orders": "version_a"},
        ),
        WriteAndReadTestCase(
            description="writes and reads audit gate metadata from fingerprint storage",
            database=None,
            schema="test_schema",
            fingerprints=(
                Fingerprint(
                    node_type="model",
                    node_name="orders",
                    target_database=None,
                    target_schema=None,
                    target_name="orders",
                    run_id="run_001",
                    definition_hash="hash_a",
                    version_hash="version_a",
                    schema_fingerprint="schema_a",
                    definition="SELECT id FROM orders",
                    metadata_json=json.dumps(
                        {
                            "audit_gate": {
                                "status": "passed",
                                "binding_set_hash": "binding_hash",
                                "blocking_set_hash": "blocking_hash",
                                "mode": "executed",
                                "run_id": "run_001",
                                "results": [
                                    {
                                        "binding_key": "binding_key",
                                        "audit_name": "not_null_orders",
                                        "definition_fingerprint": "definition_hash",
                                        "execution_fingerprint": "execution_hash",
                                        "severity": "error",
                                        "run_scope_phase": "final",
                                        "outcome": "pass",
                                        "row_count": 0,
                                        "attached_target_name": "orders",
                                        "attached_column_name": "order_id",
                                    }
                                ],
                            },
                            "config": {"materialized": "table"},
                        },
                        sort_keys=True,
                        separators=(",", ":"),
                    ),
                    ts=datetime(2026, 1, 15, 12, 0, 0),
                ),
            ),
            expected_node_names=("orders",),
            expected_latest_definition_hashes={"orders": "hash_a"},
            expected_latest_target_names={"orders": "orders"},
            expected_latest_definitions={"orders": "SELECT id FROM orders"},
            expected_latest_version_hashes={"orders": "version_a"},
            expected_metadata_fragments=(
                '"audit_gate":{"binding_set_hash":"binding_hash"',
                '"execution_fingerprint":"execution_hash"',
                '"attached_column_name":"order_id"',
            ),
        ),
        WriteAndReadTestCase(
            description="reads latest fingerprints separately for mixed node types with same name",
            database=None,
            schema="test_schema",
            fingerprints=(
                Fingerprint(
                    node_type="model",
                    node_name="orders",
                    target_database=None,
                    target_schema=None,
                    target_name="orders",
                    run_id="run_001",
                    definition_hash="model_hash_old",
                    version_hash="model_version_old",
                    schema_fingerprint="model_schema_old",
                    definition="SELECT 1",
                    ts=datetime(2026, 1, 15, 10, 0, 0),
                ),
                Fingerprint(
                    node_type="model",
                    node_name="orders",
                    target_database=None,
                    target_schema=None,
                    target_name="orders",
                    run_id="run_002",
                    definition_hash="model_hash_new",
                    version_hash="model_version_new",
                    schema_fingerprint="model_schema_new",
                    definition="SELECT 2",
                    ts=datetime(2026, 1, 15, 11, 0, 0),
                ),
                Fingerprint(
                    node_type="asset",
                    node_name="orders",
                    target_database=None,
                    target_schema=None,
                    target_name=None,
                    run_id="run_003",
                    definition_hash="task_hash_new",
                    version_hash="task_version_new",
                    schema_fingerprint="task_schema_new",
                    definition='{"source_text":"def orders(ctx): ..."}',
                    ts=datetime(2026, 1, 15, 12, 0, 0),
                ),
            ),
            expected_node_names=("orders",),
            expected_latest_definition_hashes={"orders": "model_hash_new"},
            expected_latest_target_names={"orders": "orders"},
            expected_identity_definition_hashes={
                ("model", "orders"): "model_hash_new",
                ("asset", "orders"): "task_hash_new",
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_fingerprints_when_writing_and_reading_then_returns_expected(
    test_case: WriteAndReadTestCase,
    connection: Any,
    execute: Any,
) -> None:
    fp: Fingerprint
    for fp in test_case.fingerprints:
        _ = write_fingerprint(
            connection=connection,
            execute=execute,
            database=test_case.database,
            schema=test_case.schema,
            fingerprint=fp,
            render_qualified_name=RENDER_QUALIFIED_NAME,
            render_framework_type=RENDER_FRAMEWORK_TYPE,
        )

    result: FingerprintSet = read_latest_fingerprints(
        connection=connection,
        execute=execute,
        table_exists=RELATION_EXISTS(
            connection=connection,
            database=test_case.database,
            schema=test_case.schema,
            name=FINGERPRINT_TABLE_NAME,
        ),
        database=test_case.database,
        schema=test_case.schema,
        render_qualified_name=RENDER_QUALIFIED_NAME,
        render_read_latest_sql=RENDER_READ_LATEST_SQL,
    )

    assert tuple(sorted(result.fingerprints.keys())) == test_case.expected_node_names
    node_name: str
    expected_hash: str
    for node_name, expected_hash in test_case.expected_latest_definition_hashes.items():
        assert result.fingerprints[node_name].definition_hash == expected_hash
    expected_target_name: str | None
    for node_name, expected_target_name in test_case.expected_latest_target_names.items():
        assert result.fingerprints[node_name].target_name == expected_target_name
    fp: Fingerprint
    expected_definition: str
    for node_name, expected_definition in test_case.expected_latest_definitions.items():
        assert result.fingerprints[node_name].definition == expected_definition
    expected_version_hash: str
    for node_name, expected_version_hash in test_case.expected_latest_version_hashes.items():
        assert result.fingerprints[node_name].version_hash == expected_version_hash
    fragment: str
    for fragment in test_case.expected_metadata_fragments:
        assert fragment in result.fingerprints[test_case.expected_node_names[0]].metadata_json
    for identity_key, expected_hash in test_case.expected_identity_definition_hashes.items():
        assert result.fingerprints_by_identity is not None
        assert result.fingerprints_by_identity[identity_key].definition_hash == expected_hash


@pytest.mark.parametrize(
    "test_case",
    (
        BatchWriteTestCase(
            description="writes every row across multiple batches",
            row_count=101,
            expected_last_definition="SELECT 100 AS order_id",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_many_fingerprints_when_batch_writing_then_all_rows_are_readable(
    test_case: BatchWriteTestCase,
    connection: Any,
    execute: Any,
) -> None:
    fingerprints: tuple[Fingerprint, ...] = tuple(
        Fingerprint(
            node_type="model",
            node_name=f"orders_{index}",
            target_database=None,
            target_schema="test_schema",
            target_name=f"orders_{index}",
            run_id="batch_run",
            definition_hash=f"hash_{index}",
            version_hash=f"version_{index}",
            schema_fingerprint=f"schema_{index}",
            definition=f"SELECT {index} AS order_id",
            ts=datetime(2026, 1, 15, 12, 0, index % 60),
        )
        for index in range(test_case.row_count)
    )

    write_fingerprints(
        connection=connection,
        execute=execute,
        database=None,
        schema="test_schema",
        fingerprints=fingerprints,
        render_qualified_name=RENDER_QUALIFIED_NAME,
        render_framework_type=RENDER_FRAMEWORK_TYPE,
    )
    result: FingerprintSet = read_latest_fingerprints(
        connection=connection,
        execute=execute,
        table_exists=True,
        database=None,
        schema="test_schema",
        render_qualified_name=RENDER_QUALIFIED_NAME,
        render_read_latest_sql=RENDER_READ_LATEST_SQL,
    )

    assert len(result.fingerprints) == test_case.row_count
    assert result.fingerprints[f"orders_{test_case.row_count - 1}"].definition == (
        test_case.expected_last_definition
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ReadNonExistentTableTestCase(
            description="returns empty set when fingerprint table does not exist",
            database=None,
            schema="test_schema",
            expected_node_count=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_no_table_when_reading_then_returns_empty_set(
    test_case: ReadNonExistentTableTestCase,
    connection: Any,
    execute: Any,
) -> None:
    result: FingerprintSet = read_latest_fingerprints(
        connection=connection,
        execute=execute,
        table_exists=RELATION_EXISTS(
            connection=connection,
            database=test_case.database,
            schema=test_case.schema,
            name=FINGERPRINT_TABLE_NAME,
        ),
        database=test_case.database,
        schema=test_case.schema,
        render_qualified_name=RENDER_QUALIFIED_NAME,
        render_read_latest_sql=RENDER_READ_LATEST_SQL,
    )

    assert len(result.fingerprints) == test_case.expected_node_count


@pytest.mark.parametrize(
    "test_case",
    [
        OldFingerprintSchemaTestCase(
            description="old fingerprint table without version hash raises upgrade guidance",
            schema="old_fingerprint_schema",
            expected_error_fragments=(
                "Unable to read fingerprints from old_fingerprint_schema._sqlbuild_fingerprints",
                "upgrading from an older sqlbuild version",
                "delete or rebuild the SQLBuild fingerprint table",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_old_fingerprint_table_without_version_hash_when_reading_then_raises_upgrade_guidance(
    test_case: OldFingerprintSchemaTestCase,
    connection: Any,
    execute: Any,
) -> None:
    schema: str = test_case.schema
    connection.execute(f"CREATE SCHEMA IF NOT EXISTS {schema}")
    connection.execute(
        f"CREATE TABLE {schema}.{FINGERPRINT_TABLE_NAME} ("
        "model_name VARCHAR NOT NULL, "
        "target_database VARCHAR, "
        "target_schema VARCHAR, "
        "target_name VARCHAR, "
        "run_id VARCHAR NOT NULL, "
        "query_hash VARCHAR NOT NULL, "
        "schema_fingerprint VARCHAR NOT NULL, "
        "query_sql_b64 VARCHAR NOT NULL, "
        "metadata_json_b64 VARCHAR NOT NULL, "
        "ts TIMESTAMP NOT NULL)"
    )
    connection.execute(
        f"INSERT INTO {schema}.{FINGERPRINT_TABLE_NAME} VALUES (?, NULL, ?, ?, ?, ?, ?, ?, ?, ?)",
        (
            "orders",
            schema,
            "orders",
            "run_001",
            "hash_a",
            "schema_a",
            "U0VMRUNUIDo=",
            "e30=",
            datetime(2026, 1, 15, 12, 0, 0),
        ),
    )

    with pytest.raises(ValueError) as error_info:
        read_latest_fingerprints(
            connection=connection,
            execute=execute,
            table_exists=RELATION_EXISTS(
                connection=connection, database=None, schema=schema, name=FINGERPRINT_TABLE_NAME
            ),
            database=None,
            schema=schema,
            render_qualified_name=RENDER_QUALIFIED_NAME,
            render_read_latest_sql=RENDER_READ_LATEST_SQL,
        )

    message: str = str(error_info.value)
    fragment: str
    for fragment in test_case.expected_error_fragments:
        assert fragment in message


@pytest.mark.parametrize(
    "test_case",
    [
        WriteCreatesTableTestCase(
            description="write creates table if it does not exist",
            database=None,
            schema="test_schema",
            fingerprint=Fingerprint(
                node_type="model",
                node_name="orders",
                target_database=None,
                target_schema=None,
                target_name="orders",
                run_id="run_001",
                definition_hash="hash_a",
                version_hash="version_a",
                schema_fingerprint="schema_a",
                definition="SELECT 1",
                ts=datetime(2026, 1, 15, 12, 0, 0),
            ),
            expected_table_exists=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_no_table_when_writing_then_creates_table(
    test_case: WriteCreatesTableTestCase,
    connection: Any,
    execute: Any,
) -> None:
    _ = write_fingerprint(
        connection=connection,
        execute=execute,
        database=test_case.database,
        schema=test_case.schema,
        fingerprint=test_case.fingerprint,
        render_qualified_name=RENDER_QUALIFIED_NAME,
        render_framework_type=RENDER_FRAMEWORK_TYPE,
    )

    row: Any = connection.execute(
        f"SELECT 1 FROM information_schema.tables "
        f"WHERE table_schema = '{test_case.schema}' "
        f"AND table_name = '{FINGERPRINT_TABLE_NAME}'"
    ).fetchone()
    assert (row is not None) == test_case.expected_table_exists
    column_row: Any = connection.execute(
        "SELECT 1 FROM information_schema.columns "
        f"WHERE table_schema = '{test_case.schema}' "
        f"AND table_name = '{FINGERPRINT_TABLE_NAME}' "
        "AND column_name = 'definition_b64'"
    ).fetchone()
    assert column_row is not None


@pytest.mark.parametrize(
    "test_case",
    [
        LatestResolutionTestCase(
            description="resolves latest fingerprint when older row is inserted after newer",
            database=None,
            schema="test_schema",
            fingerprints=(
                Fingerprint(
                    node_type="model",
                    node_name="orders",
                    target_database=None,
                    target_schema=None,
                    target_name="orders",
                    run_id="run_002",
                    definition_hash="new_hash",
                    version_hash="new_version",
                    schema_fingerprint="new_schema",
                    definition="SELECT 2",
                    ts=datetime(2026, 1, 15, 12, 0, 0),
                ),
                Fingerprint(
                    node_type="model",
                    node_name="orders",
                    target_database=None,
                    target_schema=None,
                    target_name="orders",
                    run_id="run_001",
                    definition_hash="old_hash",
                    version_hash="old_version",
                    schema_fingerprint="old_schema",
                    definition="SELECT 1",
                    ts=datetime(2026, 1, 15, 10, 0, 0),
                ),
            ),
            expected_latest_run_id="run_002",
            expected_latest_definition_hash="new_hash",
            expected_latest_definition="SELECT 2",
        ),
        LatestResolutionTestCase(
            description="resolves latest fingerprint by run id when timestamps tie",
            database=None,
            schema="test_schema",
            fingerprints=(
                Fingerprint(
                    node_type="model",
                    node_name="orders",
                    target_database=None,
                    target_schema=None,
                    target_name="orders",
                    run_id="run_001",
                    definition_hash="low_run_hash",
                    version_hash="low_run_version",
                    schema_fingerprint="low_run_schema",
                    definition="SELECT 1",
                    ts=datetime(2026, 1, 15, 12, 0, 0),
                ),
                Fingerprint(
                    node_type="model",
                    node_name="orders",
                    target_database=None,
                    target_schema=None,
                    target_name="orders",
                    run_id="run_002",
                    definition_hash="high_run_hash",
                    version_hash="high_run_version",
                    schema_fingerprint="high_run_schema",
                    definition="SELECT 2",
                    ts=datetime(2026, 1, 15, 12, 0, 0),
                ),
            ),
            expected_latest_run_id="run_002",
            expected_latest_definition_hash="high_run_hash",
            expected_latest_definition="SELECT 2",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_multiple_fingerprints_when_reading_then_resolves_latest(
    test_case: LatestResolutionTestCase,
    connection: Any,
    execute: Any,
) -> None:
    fp: Fingerprint
    for fp in test_case.fingerprints:
        _ = write_fingerprint(
            connection=connection,
            execute=execute,
            database=test_case.database,
            schema=test_case.schema,
            fingerprint=fp,
            render_qualified_name=RENDER_QUALIFIED_NAME,
            render_framework_type=RENDER_FRAMEWORK_TYPE,
        )

    result: FingerprintSet = read_latest_fingerprints(
        connection=connection,
        execute=execute,
        table_exists=RELATION_EXISTS(
            connection=connection,
            database=test_case.database,
            schema=test_case.schema,
            name=FINGERPRINT_TABLE_NAME,
        ),
        database=test_case.database,
        schema=test_case.schema,
        render_qualified_name=RENDER_QUALIFIED_NAME,
        render_read_latest_sql=RENDER_READ_LATEST_SQL,
    )
    latest: Fingerprint = result.fingerprints["orders"]

    assert latest.run_id == test_case.expected_latest_run_id
    assert latest.definition_hash == test_case.expected_latest_definition_hash
    assert latest.definition == test_case.expected_latest_definition


@pytest.mark.parametrize(
    "test_case",
    [
        PruneFingerprintHistoryTestCase(
            description="prunes fingerprint history by node and run id tie breaker",
            database=None,
            schema="test_schema",
            retain_versions=2,
            fingerprints=(
                Fingerprint(
                    node_type="model",
                    node_name="orders",
                    target_database=None,
                    target_schema=None,
                    target_name="orders",
                    run_id="run_000",
                    definition_hash="hash_old",
                    version_hash="version_old",
                    schema_fingerprint="schema_old",
                    definition="SELECT 0",
                    ts=datetime(2026, 1, 15, 10, 0, 0),
                ),
                Fingerprint(
                    node_type="model",
                    node_name="orders",
                    target_database=None,
                    target_schema=None,
                    target_name="orders",
                    run_id="run_001",
                    definition_hash="hash_low_tie",
                    version_hash="version_low_tie",
                    schema_fingerprint="schema_low_tie",
                    definition="SELECT 1",
                    ts=datetime(2026, 1, 15, 12, 0, 0),
                ),
                Fingerprint(
                    node_type="model",
                    node_name="orders",
                    target_database=None,
                    target_schema=None,
                    target_name="orders",
                    run_id="run_002",
                    definition_hash="hash_high_tie",
                    version_hash="version_high_tie",
                    schema_fingerprint="schema_high_tie",
                    definition="SELECT 2",
                    ts=datetime(2026, 1, 15, 12, 0, 0),
                ),
                Fingerprint(
                    node_type="model",
                    node_name="orders",
                    target_database=None,
                    target_schema=None,
                    target_name="orders",
                    run_id="run_003",
                    definition_hash="hash_latest",
                    version_hash="version_latest",
                    schema_fingerprint="schema_latest",
                    definition="SELECT 3",
                    ts=datetime(2026, 1, 15, 13, 0, 0),
                ),
                Fingerprint(
                    node_type="model",
                    node_name="customers",
                    target_database=None,
                    target_schema=None,
                    target_name="customers",
                    run_id="run_010",
                    definition_hash="hash_customers_old",
                    version_hash="version_customers_old",
                    schema_fingerprint="schema_customers_old",
                    definition="SELECT 10",
                    ts=datetime(2026, 1, 15, 10, 0, 0),
                ),
                Fingerprint(
                    node_type="model",
                    node_name="customers",
                    target_database=None,
                    target_schema=None,
                    target_name="customers",
                    run_id="run_011",
                    definition_hash="hash_customers_latest",
                    version_hash="version_customers_latest",
                    schema_fingerprint="schema_customers_latest",
                    definition="SELECT 11",
                    ts=datetime(2026, 1, 15, 11, 0, 0),
                ),
                Fingerprint(
                    node_type="asset",
                    node_name="orders",
                    target_database=None,
                    target_schema=None,
                    target_name=None,
                    run_id="run_task_000",
                    definition_hash="hash_task_old",
                    version_hash="version_task_old",
                    schema_fingerprint="schema_task_old",
                    definition='{"source_text":"old"}',
                    ts=datetime(2026, 1, 15, 9, 0, 0),
                ),
                Fingerprint(
                    node_type="asset",
                    node_name="orders",
                    target_database=None,
                    target_schema=None,
                    target_name=None,
                    run_id="run_task_001",
                    definition_hash="hash_task_mid",
                    version_hash="version_task_mid",
                    schema_fingerprint="schema_task_mid",
                    definition='{"source_text":"mid"}',
                    ts=datetime(2026, 1, 15, 11, 0, 0),
                ),
                Fingerprint(
                    node_type="asset",
                    node_name="orders",
                    target_database=None,
                    target_schema=None,
                    target_name=None,
                    run_id="run_task_002",
                    definition_hash="hash_task_latest",
                    version_hash="version_task_latest",
                    schema_fingerprint="schema_task_latest",
                    definition='{"source_text":"latest"}',
                    ts=datetime(2026, 1, 15, 14, 0, 0),
                ),
            ),
            expected_run_ids_by_identity={
                ("model", "orders"): ("run_003", "run_002"),
                ("model", "customers"): ("run_011", "run_010"),
                ("asset", "orders"): ("run_task_002", "run_task_001"),
            },
            expected_latest_run_id="run_003",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_fingerprint_history_when_pruning_then_keeps_latest_versions_per_node(
    test_case: PruneFingerprintHistoryTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
    execute: Any,
) -> None:
    fp: Fingerprint
    for fp in test_case.fingerprints:
        _ = write_fingerprint(
            connection=connection,
            execute=execute,
            database=test_case.database,
            schema=test_case.schema,
            fingerprint=fp,
            render_qualified_name=RENDER_QUALIFIED_NAME,
            render_framework_type=RENDER_FRAMEWORK_TYPE,
        )

    execute(
        connection,
        adapter.render_prune_fingerprint_history_sql(
            database=test_case.database,
            schema=test_case.schema,
            retain_versions=test_case.retain_versions,
        ),
    )

    identity_key: tuple[str, str]
    expected_run_ids: tuple[str, ...]
    for identity_key, expected_run_ids in test_case.expected_run_ids_by_identity.items():
        node_type, node_name = identity_key
        rows: list[tuple[str]] = connection.execute(
            f"SELECT run_id FROM {test_case.schema}.{FINGERPRINT_TABLE_NAME} "
            "WHERE node_type = ? AND node_name = ? ORDER BY ts DESC, run_id DESC",
            (node_type, node_name),
        ).fetchall()
        assert tuple(row[0] for row in rows) == expected_run_ids
    latest_result: FingerprintSet = read_latest_fingerprints(
        connection=connection,
        execute=execute,
        table_exists=RELATION_EXISTS(
            connection=connection,
            database=test_case.database,
            schema=test_case.schema,
            name=FINGERPRINT_TABLE_NAME,
        ),
        database=test_case.database,
        schema=test_case.schema,
        render_qualified_name=RENDER_QUALIFIED_NAME,
        render_read_latest_sql=RENDER_READ_LATEST_SQL,
    )
    assert latest_result.fingerprints["orders"].run_id == test_case.expected_latest_run_id


@pytest.mark.parametrize(
    "test_case",
    [
        InvalidDefinitionStorageTestCase(
            description="invalid definition b64 storage raises contextual error",
            schema="test_schema",
            node_name="orders",
            raw_definition_storage="SELECT 1",
            expected_error_fragments=(
                "Invalid fingerprint definition storage for 'orders'",
                "expected base64-encoded UTF-8",
                "delete or rebuild",
                "_sqlbuild_fingerprints",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_definition_storage_when_reading_then_raises_contextual_error(
    test_case: InvalidDefinitionStorageTestCase,
    connection: Any,
    execute: Any,
) -> None:
    connection.execute(f"CREATE SCHEMA IF NOT EXISTS {test_case.schema}")
    connection.execute(
        f"CREATE TABLE {test_case.schema}.{FINGERPRINT_TABLE_NAME} ("
        "node_type VARCHAR NOT NULL, "
        "node_name VARCHAR NOT NULL, "
        "target_database VARCHAR, "
        "target_schema VARCHAR, "
        "target_name VARCHAR, "
        "run_id VARCHAR NOT NULL, "
        "definition_hash VARCHAR NOT NULL, "
        "version_hash VARCHAR NOT NULL, "
        "schema_fingerprint VARCHAR NOT NULL, "
        "definition_b64 VARCHAR NOT NULL, "
        "metadata_json_b64 VARCHAR NOT NULL, "
        "ts TIMESTAMP NOT NULL)"
    )
    connection.execute(
        f"INSERT INTO {test_case.schema}.{FINGERPRINT_TABLE_NAME} VALUES "
        "(?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        (
            "model",
            test_case.node_name,
            test_case.schema,
            test_case.node_name,
            "run_001",
            "hash_a",
            "version_a",
            "schema_a",
            test_case.raw_definition_storage,
            "e30=",
            datetime(2026, 1, 15, 12, 0, 0),
        ),
    )

    with pytest.raises(ValueError) as error_info:
        read_latest_fingerprints(
            connection=connection,
            execute=execute,
            table_exists=RELATION_EXISTS(
                connection=connection,
                database=None,
                schema=test_case.schema,
                name=FINGERPRINT_TABLE_NAME,
            ),
            database=None,
            schema=test_case.schema,
            render_qualified_name=RENDER_QUALIFIED_NAME,
            render_read_latest_sql=RENDER_READ_LATEST_SQL,
        )

    message: str = str(error_info.value)
    fragment: str
    for fragment in test_case.expected_error_fragments:
        assert fragment in message


@pytest.mark.parametrize(
    "test_case",
    [
        SelectLatestTestCase(
            description="keeps named nodes of every type and unfiltered node types",
            node_names=("orders",),
            filtered_node_types=("model", "udf", "seed"),
            expected_identities=frozenset(
                {("model", "orders"), ("seed", "orders"), ("python", "export_orders")}
            ),
        ),
        SelectLatestTestCase(
            description="keeps only unfiltered node types without names",
            node_names=(),
            filtered_node_types=("model", "udf", "seed"),
            expected_identities=frozenset({("python", "export_orders")}),
        ),
        SelectLatestTestCase(
            description="keeps only named nodes without filtered types",
            node_names=("customers", "is_large_order"),
            filtered_node_types=(),
            expected_identities=frozenset({("model", "customers"), ("udf", "is_large_order")}),
        ),
        SelectLatestTestCase(
            description="keeps nothing without names or filtered types",
            node_names=(),
            filtered_node_types=(),
            expected_identities=frozenset(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unfiltered_latest_read_when_selecting_then_matches_filtered_read(
    test_case: SelectLatestTestCase,
    connection: Any,
    execute: Any,
) -> None:
    write_fingerprints(
        connection=connection,
        execute=execute,
        database=None,
        schema="test_schema",
        fingerprints=mixed_fingerprint_history(),
        render_qualified_name=RENDER_QUALIFIED_NAME,
        render_framework_type=RENDER_FRAMEWORK_TYPE,
    )

    def read(node_names: tuple[str, ...] | None) -> FingerprintSet:
        return read_latest_fingerprints(
            connection=connection,
            execute=execute,
            table_exists=True,
            database=None,
            schema="test_schema",
            render_qualified_name=RENDER_QUALIFIED_NAME,
            render_read_latest_sql=RENDER_READ_LATEST_SQL,
            node_names=node_names,
            filtered_node_types=test_case.filtered_node_types,
        )

    filtered: FingerprintSet = read(test_case.node_names)
    selected: FingerprintSet = select_latest_fingerprints(
        fingerprint_set=read(None),
        node_names=test_case.node_names,
        filtered_node_types=test_case.filtered_node_types,
    )

    assert selected.fingerprints_by_identity == filtered.fingerprints_by_identity
    assert selected.fingerprints == filtered.fingerprints
    assert frozenset(selected.fingerprints_by_identity or {}) == test_case.expected_identities
    assert {fingerprint.run_id for fingerprint in selected.fingerprints.values()} <= {"run_003"}
