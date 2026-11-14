"""Shared helpers for ClickHouse integration tests."""

from __future__ import annotations

import threading
import time
import uuid
from collections.abc import Callable
from datetime import date, datetime
from typing import Any

import duckdb

from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapters.clickhouse.classes.clickhouse_adapter import ClickHouseAdapter
from sqlbuild.microbatches.models import MicrobatchEvent, MicrobatchScope
from sqlbuild.microbatches.types import (
    MicrobatchCompletionType,
    MicrobatchFingerprintStatus,
    MicrobatchRecordType,
    MicrobatchRunType,
)
from tests.integration.src.sqlbuild.adapters.clickhouse._test_types import (
    ClickHouseRewriteTestCase,
)

CLICKHOUSE_IMAGE: str = "clickhouse/clickhouse-server:26.1"
ORDERS_COLUMNS: str = "order_id Int64, customer String, amount Int64, order_date Date"
ORDER_COLUMN_NAMES: tuple[str, ...] = ("order_id", "customer", "amount", "order_date")
INITIAL_ORDERS: tuple[tuple[int, str, int, str], ...] = (
    (1, "ada", 10, "2026-01-05"),
    (2, "bo", 20, "2026-01-20"),
    (3, "cy", 30, "2026-02-03"),
    (4, "di", 40, "2026-02-17"),
    (5, "ed", 50, "2026-03-01"),
)


class InjectedFailure(RuntimeError):
    """Raised by ``FailingExecute`` in place of one chosen statement."""


class FailingExecute:
    """Wrap ``adapter.execute`` and fail the n-th executed statement (1-based)."""

    def __init__(self, *, execute: Callable[..., Any], fail_statement_number: int) -> None:
        self._execute: Callable[..., Any] = execute
        self._failures: dict[int, Callable[[str], None]] = {
            fail_statement_number: _raise_injected_failure
        }
        self._count: int = 0

    def __call__(self, *, connection: Any, sql: str) -> Any:
        self._count += 1
        self._failures.get(self._count, _ignore_statement)(sql)
        return self._execute(connection=connection, sql=sql)


def microbatch_completion(*, schema: str, created_at: datetime) -> MicrobatchEvent:
    return MicrobatchEvent(
        event_id="event-1",
        record_type=MicrobatchRecordType.PARTITION_COMPLETION,
        scope=MicrobatchScope(
            scope_kind="direct_logical",
            scope_key=f"clickhouse:{schema}.orders",
            model_name="orders",
            target_database=None,
            target_schema=schema,
            target_name="orders",
            physical_generation_id="*",
        ),
        origin_run_id="origin-run",
        execution_run_id="execution-run",
        run_type=MicrobatchRunType.REPLAY_ON_CHANGE,
        completion_type=MicrobatchCompletionType.RECOVERY,
        run_start="0",
        run_end="1",
        partition_start="0",
        partition_end="1",
        batch_size="1",
        cursor_column="batch_id",
        cursor_type="integer",
        model_version_hash="F2",
        definition_hash="definition",
        fingerprint_status=MicrobatchFingerprintStatus.KNOWN,
        rows_affected=0,
        created_at=created_at,
    )


def build_unique_schema_name(*, prefix: str) -> str:
    return f"{prefix}_{uuid.uuid4().hex[:10]}"


def fetch_rows(
    *, adapter: ClickHouseAdapter, connection: Any, sql: str
) -> tuple[tuple[object, ...], ...]:
    cursor: Any = adapter.execute(connection=connection, sql=sql)
    return tuple(tuple(row) for row in cursor.fetchall())


def execute_all(
    *, adapter: ClickHouseAdapter, connection: Any, statements: tuple[str, ...]
) -> None:
    for statement in statements:
        adapter.execute(connection=connection, sql=statement)


def recorded_statements(recorder: StatementRecorder) -> tuple[str, ...]:
    return tuple(event.content for event in recorder.snapshot())


def create_orders(
    *,
    adapter: ClickHouseAdapter,
    connection: Any,
    relation: str,
    partition_by: str,
    rows: tuple[tuple[int, str, int, str], ...],
) -> None:
    adapter.execute(
        connection=connection,
        sql=f"CREATE TABLE {relation} ({ORDERS_COLUMNS}) ENGINE = MergeTree "
        f"PARTITION BY {partition_by} ORDER BY order_id",
    )
    adapter.execute(
        connection=connection,
        sql=f"INSERT INTO {relation} VALUES " + ", ".join(_row_sql(row) for row in rows),
    )


def order_rows_as_dates(
    rows: tuple[tuple[int, str, int, str], ...],
) -> tuple[tuple[object, ...], ...]:
    return tuple((row[0], row[1], row[2], date.fromisoformat(row[3])) for row in rows)


def oracle_rows(test_case: ClickHouseRewriteTestCase) -> tuple[tuple[object, ...], ...]:
    """Apply the same write on DuckDB with plain DELETE, UPDATE and INSERT statements."""

    database: Any = duckdb.connect()
    database.execute(
        "CREATE TABLE orders (order_id BIGINT, customer VARCHAR, amount BIGINT, order_date DATE)"
    )
    database.execute(
        "CREATE TABLE incoming (order_id BIGINT, customer VARCHAR, amount BIGINT, order_date DATE)"
    )
    for table, rows in (("orders", test_case.initial_rows), ("incoming", test_case.incoming_rows)):
        for row in rows:
            database.execute(f"INSERT INTO {table} VALUES (?, ?, ?, ?)", list(row))
    statement: str
    for statement in _ORACLE_STATEMENTS[test_case.strategy](test_case):
        database.execute(statement)
    rows: list[tuple[object, ...]] = database.execute(
        "SELECT order_id, customer, amount, order_date FROM orders "
        "ORDER BY order_id, customer, amount"
    ).fetchall()
    database.close()
    return tuple(tuple(row) for row in rows)


def apply_write(
    *,
    adapter: ClickHouseAdapter,
    connection: Any,
    test_case: ClickHouseRewriteTestCase,
    relation: str,
) -> None:
    _WRITERS[test_case.strategy](
        adapter=adapter,
        connection=connection,
        test_case=test_case,
        relation=relation,
        sql=_incoming_sql(test_case.incoming_rows),
    )


def cursor_delta_sql(
    *, adapter: ClickHouseAdapter, connection: Any, relation: str, source: str
) -> str:
    """The source rows past the target's highest amount, as a cursor-driven run reads them."""

    cursor: object = fetch_rows(
        adapter=adapter, connection=connection, sql=f"SELECT max(amount) FROM {relation}"
    )[0][0]
    return f"SELECT order_id, customer, amount, order_date FROM {source} WHERE amount > {cursor}"


def incoming_rows_sql(rows: tuple[tuple[int, str, int, str], ...]) -> str:
    return _incoming_sql(rows)


def write_cursor_delta(
    *, adapter: ClickHouseAdapter, connection: Any, relation: str, sql: str
) -> None:
    adapter.delete_insert(
        connection=connection,
        destination=relation,
        sql=sql,
        unique_key=("order_id",),
        columns=ORDER_COLUMN_NAMES,
        statement_recorder=StatementRecorder(),
    )


def run_concurrent_cursor_windows(
    *,
    adapter: ClickHouseAdapter,
    config: dict[str, object],
    relation: str,
    first_day: date,
    start_offsets: tuple[int, ...],
    window_days: int,
) -> list[BaseException]:
    """Rewrite one cursor window per thread, each on its own connection, and collect errors."""

    errors: list[BaseException] = []

    def run_window(start_offset: int) -> None:
        worker: Any = adapter.connect(dict(config))
        try:
            start: date = date.fromordinal(first_day.toordinal() + start_offset)
            end: date = date.fromordinal(start.toordinal() + window_days)
            adapter.delete_insert_cursor(
                connection=worker,
                destination=relation,
                sql=(
                    f"SELECT event_id, event_day, toInt64(1) AS version FROM {relation} "
                    f"WHERE event_day >= '{start}' AND event_day < '{end}'"
                ),
                cursor_column="event_day",
                cursor_start=start.isoformat(),
                cursor_end=end.isoformat(),
                statement_recorder=StatementRecorder(),
                cursor_type="timestamp",
            )
        except BaseException as error:
            errors.append(error)
        finally:
            adapter.close(worker)

    threads: list[threading.Thread] = [
        threading.Thread(target=run_window, args=(offset,)) for offset in start_offsets
    ]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()
    return errors


def interrupt_running_query(
    *, adapter: ClickHouseAdapter, connection: Any, sql: str, delay_seconds: float
) -> tuple[bool, bool, int, float]:
    """Interrupt ``sql`` mid-flight; return (interrupted, finished, error count, seconds)."""

    errors: list[BaseException] = []
    started: float = time.monotonic()

    def run() -> None:
        try:
            adapter.execute(connection=connection, sql=sql)
        except BaseException as error:
            errors.append(error)

    thread: threading.Thread = threading.Thread(target=run)
    thread.start()
    time.sleep(delay_seconds)
    interrupted: bool = adapter.interrupt_connection(connection)
    thread.join(timeout=20)
    return interrupted, not thread.is_alive(), len(errors), time.monotonic() - started


def _row_sql(row: tuple[int, str, int, str]) -> str:
    return f"({row[0]}, '{row[1]}', {row[2]}, '{row[3]}')"


def _incoming_sql(rows: tuple[tuple[int, str, int, str], ...]) -> str:
    empty: str = (
        "SELECT toInt64(0) AS order_id, '' AS customer, toInt64(0) AS amount, "
        "toDate('2026-01-01') AS order_date WHERE 0"
    )
    return " UNION ALL ".join(
        (
            empty,
            *(
                f"SELECT toInt64({row[0]}) AS order_id, '{row[1]}' AS customer, "
                f"toInt64({row[2]}) AS amount, toDate('{row[3]}') AS order_date"
                for row in rows
            ),
        )
    )


def _raise_injected_failure(sql: str) -> None:
    raise InjectedFailure(sql)


def _ignore_statement(sql: str) -> None:
    del sql


def _append_oracle(test_case: ClickHouseRewriteTestCase) -> tuple[str, ...]:
    del test_case
    return ("INSERT INTO orders SELECT * FROM incoming",)


def _delete_insert_oracle(test_case: ClickHouseRewriteTestCase) -> tuple[str, ...]:
    keys: str = ", ".join(test_case.unique_key)
    return (
        f"DELETE FROM orders WHERE ({keys}) IN (SELECT {keys} FROM incoming)",
        "INSERT INTO orders SELECT * FROM incoming",
    )


def _cursor_oracle(test_case: ClickHouseRewriteTestCase) -> tuple[str, ...]:
    assert test_case.window is not None
    return (
        f"DELETE FROM orders WHERE order_date >= '{test_case.window[0]}' "
        f"AND order_date < '{test_case.window[1]}'",
        "INSERT INTO orders SELECT * FROM incoming",
    )


def _keyed_cursor_oracle(test_case: ClickHouseRewriteTestCase) -> tuple[str, ...]:
    assert test_case.window is not None
    keys: str = ", ".join(test_case.unique_key)
    return (
        f"DELETE FROM orders WHERE (order_date >= '{test_case.window[0]}' "
        f"AND order_date < '{test_case.window[1]}') "
        f"OR ({keys}) IN (SELECT {keys} FROM incoming)",
        "INSERT INTO orders SELECT * FROM incoming",
    )


def _merge_oracle(test_case: ClickHouseRewriteTestCase) -> tuple[str, ...]:
    updated_columns: list[str] = sorted(
        set(ORDER_COLUMN_NAMES[1:]) - set(test_case.exclude_columns)
    )
    updated: str = ", ".join(f"{column} = incoming.{column}" for column in updated_columns)
    return (
        f"UPDATE orders SET {updated} FROM incoming WHERE orders.order_id = incoming.order_id",
        "INSERT INTO orders SELECT * FROM incoming WHERE order_id NOT IN "
        "(SELECT order_id FROM orders)",
    )


def _write_append(
    *,
    adapter: ClickHouseAdapter,
    connection: Any,
    test_case: ClickHouseRewriteTestCase,
    relation: str,
    sql: str,
) -> None:
    del test_case
    adapter.append(
        connection=connection,
        destination=relation,
        sql=sql,
        columns=ORDER_COLUMN_NAMES,
        statement_recorder=StatementRecorder(),
    )


def _write_delete_insert(
    *,
    adapter: ClickHouseAdapter,
    connection: Any,
    test_case: ClickHouseRewriteTestCase,
    relation: str,
    sql: str,
) -> None:
    adapter.delete_insert(
        connection=connection,
        destination=relation,
        sql=sql,
        unique_key=test_case.unique_key,
        columns=ORDER_COLUMN_NAMES,
        statement_recorder=StatementRecorder(),
    )


def _write_cursor(
    *,
    adapter: ClickHouseAdapter,
    connection: Any,
    test_case: ClickHouseRewriteTestCase,
    relation: str,
    sql: str,
) -> None:
    assert test_case.window is not None
    adapter.delete_insert_cursor(
        connection=connection,
        destination=relation,
        sql=sql,
        cursor_column="order_date",
        cursor_start=test_case.window[0],
        cursor_end=test_case.window[1],
        columns=ORDER_COLUMN_NAMES,
        statement_recorder=StatementRecorder(),
        cursor_type="timestamp",
    )


def _write_keyed_cursor(
    *,
    adapter: ClickHouseAdapter,
    connection: Any,
    test_case: ClickHouseRewriteTestCase,
    relation: str,
    sql: str,
) -> None:
    assert test_case.window is not None
    adapter.delete_insert_cursor(
        connection=connection,
        destination=relation,
        sql=sql,
        cursor_column="order_date",
        cursor_start=test_case.window[0],
        cursor_end=test_case.window[1],
        columns=ORDER_COLUMN_NAMES,
        statement_recorder=StatementRecorder(),
        cursor_type="timestamp",
        unique_key=test_case.unique_key,
    )


def _write_merge(
    *,
    adapter: ClickHouseAdapter,
    connection: Any,
    test_case: ClickHouseRewriteTestCase,
    relation: str,
    sql: str,
) -> None:
    adapter.merge(
        connection=connection,
        destination=relation,
        sql=sql,
        unique_key=test_case.unique_key,
        statement_recorder=StatementRecorder(),
        exclude_columns=test_case.exclude_columns,
    )


_ORACLE_STATEMENTS: dict[str, Callable[[ClickHouseRewriteTestCase], tuple[str, ...]]] = {
    "append": _append_oracle,
    "delete_insert": _delete_insert_oracle,
    "cursor": _cursor_oracle,
    "keyed_cursor": _keyed_cursor_oracle,
    "merge": _merge_oracle,
}
_WRITERS: dict[str, Callable[..., None]] = {
    "append": _write_append,
    "delete_insert": _write_delete_insert,
    "cursor": _write_cursor,
    "keyed_cursor": _write_keyed_cursor,
    "merge": _write_merge,
}


def copy_relation(
    *, adapter: ClickHouseAdapter, connection: Any, kind: str, origin: str, destination: str
) -> None:
    """Copy ``origin`` to ``destination`` through one of the adapter's copy statements."""

    execute_all(
        adapter=adapter,
        connection=connection,
        statements=_COPY_STATEMENTS[kind](adapter, origin, destination),
    )


_COPY_STATEMENTS: dict[str, Callable[[ClickHouseAdapter, str, str], tuple[str, ...]]] = {
    "clone": lambda adapter, origin, destination: adapter.render_clone(
        origin=origin, destination=destination
    ),
    "hard copy": lambda adapter, origin, destination: adapter.render_clone(
        origin=origin, destination=destination, hard_copy=True
    ),
    "replace from relation": lambda adapter, origin, destination: (
        adapter.render_replace_table_from_relation(destination=destination, origin=origin)
    ),
    "migration stage": lambda adapter, origin, destination: (
        adapter.render_migration_stage(origin=origin, stage=destination).statements
    ),
}
