"""Project builders for model migration CLI e2e tests."""

from __future__ import annotations

import json
import subprocess
from collections.abc import Callable
from pathlib import Path
from typing import Any

from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

DATABASE_FILE: str = "orders.duckdb"
_PROJECT_TOML: str = (
    f'name = "orders_project"\nadapter = "duckdb"\n\n[connection]\ndatabase = "{DATABASE_FILE}"\n'
    "\n[migrations]\nold_name_views = false\n"
)
_SOURCES_YML: str = (
    "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
    "    schema: main\n    table: raw_orders\n"
)


def orders_sql(*, migrate_from: str) -> str:
    """Return an incremental orders model with an optional migrate_from header."""

    migration: str = {"": ""}.get(migrate_from, f'  migrate_from "{migrate_from}",\n')
    return (
        "MODEL (description 'Test model.',\n"
        "  materialized incremental,\n"
        "  incremental_strategy delete_insert,\n"
        "  unique_key order_id,\n"
        "  cursor order_date,\n"
        "  cursor_type timestamp,\n"
        "  cursor_grain day,\n"
        '  cursor_start "2026-01-01",\n'
        f"{migration}"
        ");\n\n"
        'SELECT order_id, order_date, amount_cents FROM __source("raw_orders")\n'
    )


def write_orders_project(*, tmp_path: Path, name: str, migrate_from: str) -> Path:
    """Write a project containing exactly one incremental orders model."""

    project_dir: Path = tmp_path / "orders_project"
    stale: Path
    for stale in (project_dir / "sql" / "models").glob("*.sql"):
        stale.unlink()
    return prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders_project",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML,
            "sql/sources/raw.yml": _SOURCES_YML,
            f"sql/models/{name}.sql": orders_sql(migrate_from=migrate_from),
        },
    )


def load_raw_orders(*, project_dir: Path, last_day: int) -> None:
    """Replace raw orders with one order per January day up to the given day."""

    execute_duckdb(
        db_path=project_dir / DATABASE_FILE,
        sql=(
            "CREATE OR REPLACE TABLE main.raw_orders AS SELECT i AS order_id, "
            "TIMESTAMP '2026-01-01' + to_days(CAST(i - 1 AS INTEGER)) AS order_date, "
            f"100 + i AS amount_cents FROM range(1, {last_day + 1}) AS t(i)"
        ),
    )


def plan_decisions(*, project_dir: Path) -> tuple[str, ...]:
    """Run sqb plan --json in a subprocess and return the migration decisions."""

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan", "--json"), project_dir=project_dir
    )
    payload: dict[str, Any] = json.loads(result.stdout)
    return tuple(str(migration["decision"]) for migration in payload["migrations"])


def build(*, project_dir: Path) -> subprocess.CompletedProcess[str]:
    """Run sqb build in a subprocess."""

    return run_sqb(command=("--no-color", "build"), project_dir=project_dir)


def order_ids(*, project_dir: Path, relation: str) -> tuple[int, ...]:
    """Return the sorted order IDs stored in one relation."""

    return tuple(
        int(row[0])
        for row in query_duckdb(
            db_path=project_dir / DATABASE_FILE,
            sql=f"SELECT order_id FROM {relation} ORDER BY 1",
        )
    )


def previous_archive_ids(*, project_dir: Path) -> tuple[tuple[int, ...], ...]:
    """Return the order IDs held by each displaced-destination archive."""

    names: list[tuple[Any, ...]] = query_duckdb(
        db_path=project_dir / DATABASE_FILE,
        sql=(
            "SELECT table_name FROM information_schema.tables WHERE table_schema = 'main' "
            "AND contains(table_name, '__migration_previous__') ORDER BY 1"
        ),
    )
    return tuple(order_ids(project_dir=project_dir, relation=f"main.{row[0]}") for row in names)


def migration_events(*, project_dir: Path) -> tuple[tuple[str, str, str], ...]:
    """Return (origin, destination, decision) for every recorded migration event in order."""

    return tuple(
        (str(row[0]), str(row[1]), str(row[2]))
        for row in query_duckdb(
            db_path=project_dir / DATABASE_FILE,
            sql=(
                "SELECT origin_name, destination_name, decision FROM main._sqlbuild_migrations "
                "ORDER BY created_at, event_id"
            ),
        )
    )


_REPLAY_LINES: dict[bool, str] = {True: "  replay_on_change full,\n", False: ""}


def fct_orders_sql(*, columns: str, extra_config: str = "", replay: bool = True) -> str:
    """Return an incremental orders fact model projecting the given value columns."""

    return (
        "MODEL (description 'Test model.',\n"
        "  materialized incremental,\n"
        "  incremental_strategy delete_insert,\n"
        "  unique_key order_id,\n"
        "  cursor order_date,\n"
        "  cursor_type timestamp,\n"
        "  cursor_grain day,\n"
        '  cursor_start "2026-01-01",\n'
        f"{_REPLAY_LINES[replay]}"
        f"{extra_config}"
        ");\n\n"
        f'SELECT order_id, order_date, {columns} FROM __source("raw_orders")\n'
    )


def write_fct_orders_project(*, tmp_path: Path, model_sql: str) -> Path:
    """Write a project containing exactly the incremental orders fact model."""

    return prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders_project",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML,
            "sql/sources/raw.yml": _SOURCES_YML,
            "sql/models/fct_orders.sql": model_sql,
        },
    )


def load_raw_order_amounts(
    *,
    project_dir: Path,
    last_day: int,
    changed_day: int = 0,
    failing_day: int = 0,
) -> None:
    """Replace raw orders with a view whose amounts can change or fail for one day."""

    amount: str = (
        f"CASE WHEN i = {failing_day} THEN CAST('broken' AS BIGINT) "
        f"WHEN i = {changed_day} THEN 999 ELSE 100 + i END"
    )
    execute_duckdb(db_path=project_dir / DATABASE_FILE, sql="DROP VIEW IF EXISTS main.raw_orders")
    execute_duckdb(
        db_path=project_dir / DATABASE_FILE,
        sql=(
            "CREATE VIEW main.raw_orders AS SELECT i AS order_id, "
            "TIMESTAMP '2026-01-01' + to_days(CAST(i - 1 AS INTEGER)) AS order_date, "
            f"{amount} AS amount FROM range(1, {last_day + 1}) AS t(i)"
        ),
    )


def plan_payload(*, project_dir: Path) -> dict[str, Any]:
    """Run sqb plan --json in a subprocess and parse stdout."""

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan", "--json"), project_dir=project_dir
    )
    assert result.returncode == 0, result.stdout + result.stderr
    return json.loads(result.stdout)


def plan_output(*, project_dir: Path) -> str:
    """Run sqb plan in a subprocess and return its text output."""

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan"), project_dir=project_dir
    )
    assert result.returncode == 0, result.stdout + result.stderr
    return result.stdout


def fct_order_values(*, project_dir: Path, column: str) -> tuple[tuple[int, int], ...]:
    """Return (order_id, value) for every built order."""

    return tuple(
        (int(row[0]), int(row[1]))
        for row in query_duckdb(
            db_path=project_dir / DATABASE_FILE,
            sql=f"SELECT order_id, {column} FROM main.fct_orders ORDER BY 1",
        )
    )


def fct_order_columns(*, project_dir: Path) -> tuple[str, ...]:
    """Return the physical columns of the orders fact table in order."""

    return tuple(
        str(row[0])
        for row in query_duckdb(
            db_path=project_dir / DATABASE_FILE,
            sql=(
                "SELECT column_name FROM information_schema.columns WHERE table_schema = 'main' "
                "AND table_name = 'fct_orders' ORDER BY ordinal_position"
            ),
        )
    )


def column_migration_events(*, project_dir: Path) -> tuple[tuple[str, str, str, str], ...]:
    """Return (origin, destination, discovery, decision) for every recorded column rename."""

    return tuple(
        (str(row[0]), str(row[1]), str(row[2]), str(row[3]))
        for row in query_duckdb(
            db_path=project_dir / DATABASE_FILE,
            sql=(
                "SELECT origin_column, destination_column, discovery, decision "
                "FROM main._sqlbuild_column_migrations ORDER BY created_at, event_id"
            ),
        )
    )


OLD_NAME_ORIGIN: str = "revenue"
OLD_NAME_DESTINATION: str = "daily_revenue"
OLD_NAME_SCHEMA: str = "analytics"
_OLD_NAME_PROJECT_TOML: str = (
    'name = "orders_project"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
    f'[connection]\ndatabase = "{DATABASE_FILE}"\n\n'
    f'[targets.dev]\nschema = "{OLD_NAME_SCHEMA}"\n\n'
    "[janitor]\nenabled = true\n"
)
_OLD_NAME_HEADERS: dict[str, str] = {
    "table": "  materialized table,\n",
    "view": "  materialized view,\n",
    "incremental": (
        "  materialized incremental,\n"
        "  incremental_strategy delete_insert,\n"
        "  unique_key order_id,\n"
        "  cursor order_date,\n"
        "  cursor_type timestamp,\n"
        "  cursor_grain day,\n"
        '  cursor_start "2026-01-01",\n'
    ),
    "snapshot": (
        "  materialized snapshot,\n"
        "  unique_key [order_id],\n"
        "  snapshot_strategy timestamp,\n"
        "  updated_at order_date,\n"
    ),
}
_OLD_NAME_FACT_ORDER: tuple[str, ...] = (
    "required",
    "origin_archived",
    "view_created",
    "view_dropped",
)


def old_name_model_sql(
    *, materialized: str, columns: str = "amount_cents", extra_config: str = ""
) -> str:
    """Return a model of the given materialization over raw orders."""

    return (
        "MODEL (description 'Test model.',\n"
        f"{_OLD_NAME_HEADERS[materialized]}"
        f"{extra_config}"
        ");\n\n"
        f'SELECT order_id, order_date, {columns} FROM __source("raw_orders")\n'
    )


def write_old_name_project(
    *,
    tmp_path: Path,
    models: dict[str, str],
    project_toml_extra: str = "",
    python_files: dict[str, str] | None = None,
) -> Path:
    """Write a project with exactly the given models and optional Python files."""

    project_dir: Path = tmp_path / "orders_project"
    stale: Path
    for stale in (project_dir / "sql" / "models").glob("*.sql"):
        stale.unlink()
    for stale in (project_dir / "python").glob("*.py"):
        stale.unlink()
    files: dict[str, str] = {
        "sqlbuild_project.toml": _OLD_NAME_PROJECT_TOML + project_toml_extra,
        "sql/sources/raw.yml": _SOURCES_YML,
        **{f"sql/models/{name}.sql": sql for name, sql in models.items()},
        **{f"python/nodes/{name}": source for name, source in (python_files or {}).items()},
    }
    return prepare_inline_project(
        tmp_path=tmp_path, project_name="orders_project", repo_files=files
    )


def old_name_facts(*, project_dir: Path) -> tuple[str, ...]:
    """Return old-name fact types in lifecycle order, or () when the table is absent."""

    readers: dict[bool, Callable[[Path], tuple[str, ...]]] = {
        True: _read_old_name_facts,
        False: _no_old_name_facts,
    }
    exists: bool = (
        relation_type(project_dir=project_dir, name="_sqlbuild_old_name_views") is not None
    )
    return readers[exists](project_dir)


def _read_old_name_facts(project_dir: Path) -> tuple[str, ...]:
    return tuple(
        str(row[0])
        for row in query_duckdb(
            db_path=project_dir / DATABASE_FILE,
            sql=(
                f"SELECT event_type FROM {OLD_NAME_SCHEMA}._sqlbuild_old_name_views ORDER BY "
                f"list_position({list(_OLD_NAME_FACT_ORDER)}, event_type), created_at"
            ),
        )
    )


def _no_old_name_facts(project_dir: Path) -> tuple[str, ...]:
    del project_dir
    return ()


def old_name_fact_values(*, project_dir: Path, event_type: str, column: str) -> tuple[Any, ...]:
    """Return one column of every old-name fact of one type, or () when the table is absent."""

    sql: str = (
        f"SELECT {column} FROM {OLD_NAME_SCHEMA}._sqlbuild_old_name_views "
        f"WHERE event_type = '{event_type}' ORDER BY created_at"
    )
    readers: dict[bool, Callable[[], tuple[Any, ...]]] = {
        True: lambda: tuple(
            row[0] for row in query_duckdb(db_path=project_dir / DATABASE_FILE, sql=sql)
        ),
        False: tuple,
    }
    table_type: str | None = relation_type(project_dir=project_dir, name="_sqlbuild_old_name_views")
    return readers[table_type is not None]()


def relation_type(*, project_dir: Path, name: str) -> str | None:
    """Return the information_schema table type of one relation in main, or None."""

    rows: list[tuple[Any, ...]] = query_duckdb(
        db_path=project_dir / DATABASE_FILE,
        sql=(
            "SELECT table_type FROM information_schema.tables "
            f"WHERE table_schema = '{OLD_NAME_SCHEMA}' AND table_name = '{name}'"
        ),
    )
    return next((str(row[0]) for row in rows), None)


def relation_rows(*, project_dir: Path, relation: str, columns: str) -> tuple[tuple[Any, ...], ...]:
    """Return the given columns of one relation ordered by order_id."""

    return tuple(
        tuple(row)
        for row in query_duckdb(
            db_path=project_dir / DATABASE_FILE,
            sql=f"SELECT {columns} FROM {relation} ORDER BY order_id",
        )
    )


def origin_archive_count(*, project_dir: Path) -> int:
    """Return how many migration_origin archives exist in main."""

    return int(
        query_duckdb(
            db_path=project_dir / DATABASE_FILE,
            sql=(
                "SELECT count(*) FROM information_schema.tables "
                f"WHERE table_schema = '{OLD_NAME_SCHEMA}' "
                "AND contains(table_name, '__migration_origin__')"
            ),
        )[0][0]
    )


def sqb(*, project_dir: Path, args: tuple[str, ...]) -> subprocess.CompletedProcess[str]:
    """Run one sqb command without colour in a subprocess."""

    return run_sqb(command=("--no-color", *args), project_dir=project_dir)


OLD_NAME_MIGRATE_FROM: str = f"  migrate_from {OLD_NAME_ORIGIN},\n"


def build_old_name_project(project_dir: Path) -> subprocess.CompletedProcess[str]:
    """Run sqb build and require success."""

    result: subprocess.CompletedProcess[str] = sqb(project_dir=project_dir, args=("build",))
    assert result.returncode == 0, result.stdout + result.stderr
    return result


def prepare_old_name_rename(
    *,
    tmp_path: Path,
    materialized: str,
    origin_columns: str = "amount_cents",
    destination_columns: str = "amount_cents",
    destination_schema: str = "",
    extra_config: str = "",
    project_toml_extra: str = "",
) -> Path:
    """Build the origin model, then declare its rename to the destination."""

    project_dir: Path = write_old_name_project(
        tmp_path=tmp_path,
        models={
            OLD_NAME_ORIGIN: old_name_model_sql(materialized=materialized, columns=origin_columns)
        },
        project_toml_extra=project_toml_extra,
    )
    load_raw_orders(project_dir=project_dir, last_day=3)
    _ = build_old_name_project(project_dir)
    return write_old_name_project(
        tmp_path=tmp_path,
        models={
            OLD_NAME_DESTINATION: old_name_model_sql(
                materialized=materialized,
                columns=destination_columns,
                extra_config=OLD_NAME_MIGRATE_FROM + destination_schema + extra_config,
            )
        },
        project_toml_extra=project_toml_extra,
    )
