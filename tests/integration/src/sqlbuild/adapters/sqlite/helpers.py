"""Shared helpers for SQLite adapter integration tests."""

from __future__ import annotations

from collections.abc import Callable
from types import MappingProxyType
from typing import Any

from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.models import ColumnInfo, RowDiffSampleRow
from sqlbuild.adapters.sqlite.classes.sqlite_adapter import SqliteAdapter
from sqlbuild.executor.run._helpers.validation.type_enforcement import enforce_types_staged


def run_sql(*, adapter: SqliteAdapter, connection: Any, statements: tuple[str, ...]) -> None:
    for statement in statements:
        adapter.execute(connection=connection, sql=statement)


def fetch_rows(
    *, adapter: SqliteAdapter, connection: Any, sql: str
) -> tuple[tuple[object, ...], ...]:
    cursor: Any = adapter.execute(connection=connection, sql=sql)
    return tuple(tuple(row) for row in cursor.fetchall())


def render_relations(
    *, adapter: SqliteAdapter, relations: tuple[tuple[str, str], ...]
) -> tuple[str | None, ...]:
    """Render relations as the planner does, which records the schema each table belongs to."""

    return tuple(
        adapter.render_qualified_name(database=None, schema=schema, name=name)
        for schema, name in relations
    )


def stage_contract_relation(
    *, adapter: SqliteAdapter, connection: Any, name: str, sql: str
) -> None:
    """Create a relation and enforce a DECIMAL(10,2) payment contract on it, as staging does."""

    adapter.create_table_as(
        connection=connection, destination=name, sql=sql, statement_recorder=recorder()
    )
    enforce_types_staged(
        adapter=adapter,
        connection=connection,
        staging_qualified=name,
        staging_database=None,
        staging_schema="main",
        staging_table=name,
        declared_columns=(
            ColumnInfo("payment_id", "INTEGER"),
            ColumnInfo("amount", "DECIMAL(10,2)"),
        ),
        table_type="table",
        statement_recorder=recorder(),
    )


def recorder() -> StatementRecorder:
    return StatementRecorder()


def create_orders(*, adapter: SqliteAdapter, connection: Any, relation: str) -> None:
    run_sql(
        adapter=adapter,
        connection=connection,
        statements=(
            f"CREATE TABLE {relation} (order_id INTEGER, customer_id INTEGER, "
            "amount DECIMAL(10,2), ordered_at TIMESTAMP, status VARCHAR(16))",
            f"INSERT INTO {relation} VALUES "
            "(1, 10, 12.5, '2026-01-01 10:00:00', 'placed'), "
            "(2, 10, 7.25, '2026-01-02 11:00:00', 'shipped'), "
            "(3, 20, 30, '2026-01-03 12:00:00', 'placed')",
        ),
    )


def hold_write_lock(*, database_path: str, ready_path: str, hold_seconds: float) -> None:
    """Child process: take the write lock, signal readiness, hold it, then commit."""

    import time
    from pathlib import Path

    adapter: SqliteAdapter = SqliteAdapter()
    connection: Any = adapter.connect({"path": database_path})
    adapter.begin(connection)
    adapter.execute(connection=connection, sql="INSERT INTO orders VALUES (100)")
    Path(ready_path).write_text("ready", encoding="utf-8")
    time.sleep(hold_seconds)
    adapter.commit(connection)
    adapter.close(connection)


def replace_orders_then_hang(*, database_path: str, ready_path: str) -> None:
    """Child process: rewrite orders inside an open transaction, signal, then wait to be killed."""

    import time
    from pathlib import Path

    adapter: SqliteAdapter = SqliteAdapter()
    connection: Any = adapter.connect({"path": database_path})
    adapter.begin(connection)
    adapter.execute(connection=connection, sql="DELETE FROM orders")
    adapter.execute(connection=connection, sql="INSERT INTO orders VALUES (999)")
    adapter.execute(connection=connection, sql="CREATE TABLE orders_rebuilt (order_id INTEGER)")
    Path(ready_path).write_text("ready", encoding="utf-8")
    time.sleep(60)


def attempt_write(*, database_path: str, busy_timeout: float, result_path: str) -> None:
    """Child process: try one insert and record the outcome code and how long it waited."""

    import json
    import time
    from pathlib import Path

    from sqlbuild.adapter.contract.exceptions import AdapterUserError

    adapter: SqliteAdapter = SqliteAdapter()
    connection: Any = adapter.connect({"path": database_path, "busy_timeout": busy_timeout})
    started: float = time.monotonic()
    code: str = "ok"
    message: str = ""
    try:
        adapter.execute(connection=connection, sql="INSERT INTO orders VALUES (200)")
    except AdapterUserError as error:
        code = error.code
        message = error.message
    waited: float = time.monotonic() - started
    adapter.close(connection)
    Path(result_path).write_text(
        json.dumps({"code": code, "message": message, "waited": waited}), encoding="utf-8"
    )


def pragma_values(
    *, adapter: SqliteAdapter, connection: Any, pragmas: tuple[str, ...]
) -> tuple[object, ...]:
    return tuple(
        fetch_rows(adapter=adapter, connection=connection, sql=f"PRAGMA {name}")[0][0]
        for name in pragmas
    )


def changed_column_names(samples: tuple[RowDiffSampleRow, ...]) -> tuple[str, ...]:
    names: list[str] = []
    sample: RowDiffSampleRow
    for sample in samples:
        names.extend(cell.name for cell in sample.changed_cells)
    return tuple(names)


def write_incrementally(
    *, adapter: SqliteAdapter, connection: Any, strategy: str, sql: str
) -> None:
    _WRITERS[strategy](adapter, connection, sql)


def evolve_orders(*, adapter: SqliteAdapter, connection: Any, operation: str) -> None:
    _EVOLUTIONS[operation](adapter, connection)


def _append(adapter: SqliteAdapter, connection: Any, sql: str) -> None:
    adapter.append(
        connection=connection, destination="orders", sql=sql, statement_recorder=recorder()
    )


def _merge(adapter: SqliteAdapter, connection: Any, sql: str) -> None:
    adapter.merge(
        connection=connection,
        destination="orders",
        sql=sql,
        unique_key="order_id",
        statement_recorder=recorder(),
    )


def _delete_insert(adapter: SqliteAdapter, connection: Any, sql: str) -> None:
    adapter.delete_insert(
        connection=connection,
        destination="orders",
        sql=sql,
        unique_key=("order_id",),
        statement_recorder=recorder(),
    )


def _cursor_window(adapter: SqliteAdapter, connection: Any, sql: str) -> None:
    adapter.delete_insert_cursor(
        connection=connection,
        destination="orders",
        sql=sql,
        cursor_column="ordered_at",
        cursor_start="2026-01-02 00:00:00",
        cursor_end="2026-01-03 00:00:00",
        cursor_type="timestamp",
        statement_recorder=recorder(),
    )


def _keyed_cursor_window(adapter: SqliteAdapter, connection: Any, sql: str) -> None:
    adapter.delete_insert_cursor(
        connection=connection,
        destination="orders",
        sql=sql,
        cursor_column="ordered_at",
        cursor_start="2026-01-02 00:00:00",
        cursor_end="2026-01-03 00:00:00",
        cursor_type="timestamp",
        statement_recorder=recorder(),
        unique_key=("order_id",),
    )


def _add_column(adapter: SqliteAdapter, connection: Any) -> None:
    adapter.add_columns(
        connection=connection,
        destination="orders",
        columns=(ColumnInfo("channel", "TEXT"),),
        statement_recorder=recorder(),
    )


def _drop_column(adapter: SqliteAdapter, connection: Any) -> None:
    adapter.drop_columns(
        connection=connection,
        destination="orders",
        column_names=("status",),
        statement_recorder=recorder(),
    )


def _rename_column(adapter: SqliteAdapter, connection: Any) -> None:
    adapter.rename_column(
        connection=connection,
        destination="orders",
        old_name="status",
        new_name="order_status",
        statement_recorder=recorder(),
    )


def _retype_column(adapter: SqliteAdapter, connection: Any) -> None:
    adapter.alter_column_types(
        connection=connection,
        destination="orders",
        columns=(ColumnInfo("order_id", "TEXT"),),
        statement_recorder=recorder(),
    )


_WRITERS: MappingProxyType[str, Callable[[SqliteAdapter, Any, str], None]] = MappingProxyType(
    {
        "append": _append,
        "merge": _merge,
        "delete_insert": _delete_insert,
        "cursor": _cursor_window,
        "keyed_cursor": _keyed_cursor_window,
    }
)
_EVOLUTIONS: MappingProxyType[str, Callable[[SqliteAdapter, Any], None]] = MappingProxyType(
    {
        "add": _add_column,
        "drop": _drop_column,
        "rename": _rename_column,
        "retype": _retype_column,
    }
)


class RecordingCursorSqliteAdapter(SqliteAdapter):
    """Record every statement and the cursor it returned."""

    def __init__(self) -> None:
        super().__init__()
        self.executed: list[tuple[str, Any]] = []

    def _execute(self, *, connection: Any, sql: str) -> Any:
        cursor: Any = super()._execute(connection=connection, sql=sql)
        self.executed.append((sql, cursor))
        return cursor
