"""Project builders and CLI runners for model migration integration tests."""

from __future__ import annotations

import json
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from textwrap import dedent
from typing import Any

import duckdb
import pytest
from _pytest.capture import CaptureResult

from sqlbuild._native import FINGERPRINT_TABLE_NAME, migration_fingerprint_work_info
from sqlbuild._native import run_cli as main
from sqlbuild.adapter.contract.models import MigrationStagePlan
from sqlbuild.adapter.contract.types import MigrationTransfer
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.build._helpers import scheduler as scheduler_module

DATABASE_FILE: str = "orders.duckdb"
PROJECT_TOML: str = dedent(
    f"""
    name = "orders_project"
    adapter = "duckdb"

    [connection]
    database = "{DATABASE_FILE}"

    [migrations]
    old_name_views = false
    """
).lstrip()
DENY_MISSING_ORIGIN_PROJECT_TOML: str = dedent(
    f"""
    name = "orders_project"
    adapter = "duckdb"
    default_target = "main"

    [connection]
    database = "{DATABASE_FILE}"

    [targets.main]
    schema = "main"
    missing_migration_origin = "deny"

    [migrations]
    old_name_views = false
    """
).lstrip()
TARGETS_PROJECT_TOML: str = dedent(
    f"""
    name = "orders_project"
    adapter = "duckdb"
    default_target = "dev"

    [connection]
    database = "{DATABASE_FILE}"

    [targets.dev]
    schema = "dev"

    [targets.prod]
    schema = "prod"

    [migrations]
    old_name_views = false
    """
).lstrip()
JANITOR_PROJECT_TOML: str = dedent(
    f"""
    name = "orders_project"
    adapter = "duckdb"

    default_target = "dev"

    [connection]
    database = "{DATABASE_FILE}"

    [targets.dev]
    schema = "dev"

    [janitor]
    enabled = true
    archive_retention_days = 0
    delete_tracked_only = false

    [migrations]
    old_name_views = false
    """
).lstrip()
RAW_SOURCES_YML: str = dedent(
    """
    sources:
      - name: raw_orders
        description: Test source raw_orders.
        schema: main
        table: raw_orders
    """
).lstrip()


_FORCE_LINES: dict[bool, str] = {True: "  migrate_force true,\n", False: ""}
_DAILY_TOTALS_HEADER: str = (
    "MODEL (description 'Test model.',\n"
    "  materialized incremental,\n"
    "  incremental_strategy delete_insert,\n"
    "  unique_key order_date,\n"
    "  cursor order_date,\n"
    "  cursor_type timestamp,\n"
    "  cursor_grain day,\n"
    '  cursor_start "2026-01-01",\n'
    "{extra_config}"
    ");\n\n"
)
_SNAPSHOT_SQL: str = (
    "MODEL (description 'Test model.',\n"
    "  materialized snapshot,\n"
    "  unique_key [order_id],\n"
    "  snapshot_strategy timestamp,\n"
    "  updated_at order_date,\n"
    "{migration}"
    ");\n\n"
    'SELECT order_id, order_date, amount_cents FROM __source("raw_orders")\n'
)
ORIGIN_MODEL: str = "stg_orders"
DESTINATION_MODEL: str = "stg_customer_orders"


@dataclass(frozen=True)
class CliRun:
    """Captured result of one in-process sqb invocation."""

    exit_code: int
    output: str


def incremental_orders_sql(
    *,
    migrate_from: str | None = None,
    migrate_force: bool = False,
    extra_config: str = "",
    select_sql: str = 'SELECT order_id, order_date, amount_cents FROM __source("raw_orders")',
) -> str:
    """Return an incremental orders model with optional migration header keys."""

    origin_lines: dict[str | None, str] = {None: ""}
    migration_lines: str = origin_lines.get(
        migrate_from, f'  migrate_from "{migrate_from}",\n'
    ) + _FORCE_LINES.get(migrate_force, "")
    return (
        "MODEL (description 'Test model.',\n"
        "  materialized incremental,\n"
        "  incremental_strategy delete_insert,\n"
        "  unique_key order_id,\n"
        "  cursor order_date,\n"
        "  cursor_type timestamp,\n"
        "  cursor_grain day,\n"
        '  cursor_start "2026-01-01",\n'
        f"{extra_config}"
        f"{migration_lines}"
        ");\n\n"
        f"{select_sql}\n"
    )


def write_project(
    *, project_dir: Path, models: dict[str, str], project_toml: str = PROJECT_TOML
) -> None:
    """Write the project config, sources, and exactly the given model files."""

    project_dir.mkdir(parents=True, exist_ok=True)
    (project_dir / "sqlbuild_project.toml").write_text(project_toml, encoding="utf-8")
    sources_dir: Path = project_dir / "sql" / "sources"
    sources_dir.mkdir(parents=True, exist_ok=True)
    (sources_dir / "raw.yml").write_text(RAW_SOURCES_YML, encoding="utf-8")
    models_dir: Path = project_dir / "sql" / "models"
    models_dir.mkdir(parents=True, exist_ok=True)
    existing: Path
    for existing in models_dir.glob("*.sql"):
        existing.unlink()
    name: str
    sql: str
    for name, sql in models.items():
        (models_dir / f"{name}.sql").write_text(sql, encoding="utf-8")


def load_raw_orders(*, project_dir: Path, first_day: int, last_day: int) -> None:
    """Replace raw orders with one order per day in the inclusive January day range."""

    execute(
        project_dir=project_dir,
        sql=(
            "CREATE OR REPLACE TABLE main.raw_orders AS SELECT i AS order_id, "
            "TIMESTAMP '2026-01-01' + to_days(CAST(i - 1 AS INTEGER)) AS order_date, "
            "100 + i AS amount_cents "
            f"FROM range({first_day}, {last_day + 1}) AS t(i)"
        ),
    )


def execute(*, project_dir: Path, sql: str) -> None:
    """Run one mutating statement against the project database."""

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(project_dir / DATABASE_FILE))
    try:
        _ = connection.execute(sql)
    finally:
        connection.close()


def query(*, project_dir: Path, sql: str) -> list[tuple[Any, ...]]:
    """Read rows from the project database."""

    connection: duckdb.DuckDBPyConnection = duckdb.connect(
        str(project_dir / DATABASE_FILE), read_only=True
    )
    try:
        return connection.execute(sql).fetchall()
    finally:
        connection.close()


def order_ids(*, project_dir: Path, relation: str) -> tuple[int, ...]:
    """Return the sorted order IDs stored in one relation."""

    return tuple(
        int(row[0])
        for row in query(project_dir=project_dir, sql=f"SELECT order_id FROM {relation} ORDER BY 1")
    )


def migration_events(
    *, project_dir: Path, schema: str = "main"
) -> tuple[tuple[str, str, str], ...]:
    """Return (origin, destination, decision) for every recorded event in creation order."""

    return tuple(
        (str(row[0]), str(row[1]), str(row[2]))
        for row in query(
            project_dir=project_dir,
            sql=(
                "SELECT origin_name, destination_name, decision "
                f"FROM {schema}._sqlbuild_migrations ORDER BY created_at, event_id"
            ),
        )
    )


def relation_names(*, project_dir: Path, schema: str) -> list[str]:
    """Return every relation name in one schema."""

    return [
        str(row[0])
        for row in query(
            project_dir=project_dir,
            sql=(
                "SELECT table_name FROM information_schema.tables "
                f"WHERE table_schema = '{schema}' ORDER BY 1"
            ),
        )
    ]


def run_sqb(
    *, project_dir: Path, args: tuple[str, ...], capsys: pytest.CaptureFixture[str]
) -> CliRun:
    """Run sqb in-process and capture combined output."""

    _ = capsys.readouterr()
    exit_code: int = main(["--project-dir", str(project_dir), "--no-color", *args])
    captured: CaptureResult[str] = capsys.readouterr()
    return CliRun(exit_code=exit_code, output=captured.out + captured.err)


def build(*, project_dir: Path, capsys: pytest.CaptureFixture[str]) -> CliRun:
    """Run sqb build, reporting an escaped exception as a failed run."""

    try:
        return run_sqb(project_dir=project_dir, args=("build",), capsys=capsys)
    except Exception as error:
        return CliRun(exit_code=1, output=str(error))


def build_ok(*, project_dir: Path, capsys: pytest.CaptureFixture[str]) -> CliRun:
    """Run sqb build and require success."""

    result: CliRun = build(project_dir=project_dir, capsys=capsys)
    assert result.exit_code == 0, result.output
    return result


def plan_json(
    *, project_dir: Path, capsys: pytest.CaptureFixture[str], args: tuple[str, ...] = ()
) -> dict[str, Any]:
    """Run sqb plan --json and parse stdout."""

    _ = capsys.readouterr()
    exit_code: int = main(
        ["--project-dir", str(project_dir), "--no-color", "plan", "--json", *args]
    )
    captured: CaptureResult[str] = capsys.readouterr()
    payload: dict[str, Any] = json.loads(captured.out)
    assert exit_code == int(
        any(warning["severity"] == "error" for warning in payload["warnings"])
    ), captured.out + captured.err
    return payload


def model_entry(*, plan: dict[str, Any], name: str) -> dict[str, Any]:
    """Return one serialized model entry from plan JSON."""

    models_by_name: dict[str, dict[str, Any]] = {model["name"]: model for model in plan["models"]}
    return models_by_name[name]


def migration_decisions(plan: dict[str, Any]) -> tuple[str, ...]:
    """Return every planned migration decision in plan order."""

    return tuple(migration["decision"] for migration in plan["migrations"])


def planned_migrations(plan: dict[str, Any]) -> tuple[tuple[str | None, str, str, str], ...]:
    """Return sorted (origin, destination, discovery, decision) for every planned migration."""

    return tuple(
        sorted(
            (
                migration["origin_model"],
                migration["model"],
                migration["discovery"],
                migration["decision"],
            )
            for migration in plan["migrations"]
        )
    )


def is_renamed_model(model: dict[str, Any]) -> bool:
    """Return whether the plan builds this model under a new name."""

    return model["reason"] == "renamed"


def is_rename_migration(migration: dict[str, Any]) -> bool:
    """Return whether this migration is a renamed model's own."""

    return bool(migration["is_rename"])


def renamed_models(plan: dict[str, Any]) -> tuple[tuple[str, str], ...]:
    """Return sorted (origin_model, name) for every model the plan builds under a new name."""

    return tuple(
        sorted(
            (model["origin_model"], model["name"])
            for model in filter(is_renamed_model, plan["models"])
        )
    )


def rename_migrations(plan: dict[str, Any]) -> tuple[tuple[str, str], ...]:
    """Return sorted (origin_model, model) for every migration marked as a renamed model's own."""

    return tuple(
        sorted(
            (migration["origin_model"], migration["model"])
            for migration in filter(is_rename_migration, plan["migrations"])
        )
    )


def renamed_old_names(plan: dict[str, Any]) -> dict[str, Any]:
    """Return each renamed model's old_name, by model, as its own row carries it."""

    return {
        model["name"]: model.get("old_name") for model in filter(is_renamed_model, plan["models"])
    }


def rename_migration_old_names(plan: dict[str, Any]) -> dict[str, Any]:
    """Return each rename migration's old_name, by model."""

    return {
        migration["model"]: migration["old_name"]
        for migration in filter(is_rename_migration, plan["migrations"])
    }


def warning_messages(plan: dict[str, Any]) -> tuple[str, ...]:
    """Return every plan warning message."""

    return tuple(str(warning["message"]) for warning in plan["warnings"])


def row_count(*, project_dir: Path, relation: str) -> int:
    """Return the number of rows stored in one relation."""

    return int(query(project_dir=project_dir, sql=f"SELECT count(*) FROM {relation}")[0][0])


def build_origin(*, project_dir: Path, capsys: pytest.CaptureFixture[str]) -> None:
    """Build the origin orders model over five days of raw orders."""

    write_project(project_dir=project_dir, models={ORIGIN_MODEL: incremental_orders_sql()})
    load_raw_orders(project_dir=project_dir, first_day=1, last_day=5)
    _ = build_ok(project_dir=project_dir, capsys=capsys)


def rename_model(
    *,
    project_dir: Path,
    name: str,
    migrate_from: str | None,
    migrate_force: bool = False,
    extra_config: str = "",
) -> None:
    """Replace the project with one incremental orders model under a new name."""

    write_project(
        project_dir=project_dir,
        models={
            name: incremental_orders_sql(
                migrate_from=migrate_from, migrate_force=migrate_force, extra_config=extra_config
            )
        },
    )


def snapshot_orders_sql(*, migration_lines: str) -> str:
    """Return a timestamp snapshot of raw orders with optional migration header lines."""

    return _SNAPSHOT_SQL.format(migration=migration_lines)


def _raise_interruption(*_: object, **__: object) -> None:
    raise RuntimeError("simulated interruption")


def fail_clone(monkeypatch: pytest.MonkeyPatch) -> None:
    """Make the migration stage statement fail before any stage exists."""

    monkeypatch.setattr(
        DuckDbAdapter,
        "render_migration_stage",
        lambda self, **_: MigrationStagePlan(
            transfer=MigrationTransfer.COPY,
            statements=("SELECT * FROM main.simulated_missing_relation",),
        ),
    )


def refuse_clone_then_copy(monkeypatch: pytest.MonkeyPatch) -> None:
    """Plan a clone with a copy fallback, as BigQuery does, and refuse the clone."""

    monkeypatch.setattr(
        DuckDbAdapter,
        "render_migration_stage",
        lambda self, *, origin, stage, **_: MigrationStagePlan(
            transfer=MigrationTransfer.CLONE,
            statements=("SELECT error('simulated clone refusal')",),
            fallback_statements=(f"CREATE TABLE {stage} AS SELECT * FROM {origin}",),
            is_clone_refusal=lambda error: "simulated clone refusal" in str(error),
        ),
    )


def fail_partial_stage(monkeypatch: pytest.MonkeyPatch) -> None:
    """Leave an empty, incomplete stage behind and then fail the copy."""

    monkeypatch.setattr(
        DuckDbAdapter,
        "render_migration_stage",
        lambda self, *, origin, stage, **_: MigrationStagePlan(
            transfer=MigrationTransfer.COPY,
            statements=(
                f"CREATE TABLE {stage} AS SELECT * FROM {origin} WHERE false",
                "SELECT * FROM main.simulated_missing_relation",
            ),
        ),
    )


def fail_verification(monkeypatch: pytest.MonkeyPatch) -> None:
    """Complete the stage, then refuse promotion."""

    monkeypatch.setattr(
        "sqlbuild.executor.migrations.main._apply.verify_stage", _raise_interruption
    )


def fail_clone_into(*, monkeypatch: pytest.MonkeyPatch, origin: str) -> None:
    """Make only the stage copied from one origin fail; every other stage runs normally."""

    original: Callable[..., MigrationStagePlan] = DuckDbAdapter.render_migration_stage
    failing: dict[str, MigrationStagePlan] = {
        origin: MigrationStagePlan(
            transfer=MigrationTransfer.COPY,
            statements=("SELECT * FROM main.simulated_missing_relation",),
        )
    }
    monkeypatch.setattr(
        DuckDbAdapter,
        "render_migration_stage",
        lambda self, *, origin, **kwargs: (
            failing.get(origin) or original(self, origin=origin, **kwargs)
        ),
    )


def disable_transactional_ddl(monkeypatch: pytest.MonkeyPatch) -> None:
    """Promote without a transaction, as Snowflake, BigQuery, and Databricks do."""

    monkeypatch.setattr(DuckDbAdapter, "supports_transactional_ddl", lambda self: False)


def fail_promotion_rename(monkeypatch: pytest.MonkeyPatch) -> None:
    """Fail when the stage is renamed into place, after any live destination moved aside."""

    original: Callable[..., None] = DuckDbAdapter.rename
    failing: dict[bool, Callable[..., None]] = {True: _raise_interruption}
    monkeypatch.setattr(
        DuckDbAdapter,
        "rename",
        lambda self, *, origin, **kwargs: failing.get("migration_stage" in origin, original)(
            self, origin=origin, **kwargs
        ),
    )


def fail_non_transactional_promotion_rename(monkeypatch: pytest.MonkeyPatch) -> None:
    """Leave the destination moved aside and missing, as a non-transactional crash would."""

    disable_transactional_ddl(monkeypatch)
    fail_promotion_rename(monkeypatch)


def fail_non_transactional_record(monkeypatch: pytest.MonkeyPatch) -> None:
    """Crash after a committed promotion but before the event, as non-transactional adapters can."""

    disable_transactional_ddl(monkeypatch)
    fail_record(monkeypatch)


def fail_model_build(*, monkeypatch: pytest.MonkeyPatch, model_name: str) -> None:
    """Make one incremental model's build raise after migrations have run."""

    failing: dict[str, Callable[..., Any]] = {model_name: _raise_interruption}
    incremental: Callable[..., Any] = scheduler_module.execute_incremental_entry
    table: Callable[..., Any] = scheduler_module.execute_table_entry
    monkeypatch.setattr(
        scheduler_module,
        "execute_incremental_entry",
        lambda **kwargs: failing.get(kwargs["context"].entry.name, incremental)(**kwargs),
    )
    monkeypatch.setattr(
        scheduler_module,
        "execute_table_entry",
        lambda **kwargs: failing.get(kwargs["context"].entry.name, table)(**kwargs),
    )


def fail_record(monkeypatch: pytest.MonkeyPatch) -> None:
    """Crash after the clone but before the migration event is recorded."""

    monkeypatch.setattr(
        "sqlbuild.executor.migrations._helpers.promotion.write_migration_state_event",
        _raise_interruption,
    )


def fail_after_migrations(monkeypatch: pytest.MonkeyPatch) -> None:
    """Crash after migrations are recorded but before any model builds."""

    monkeypatch.setattr(
        "sqlbuild.executor.build.main._execute.apply_retention_phase", _raise_interruption
    )


def enriched_view_sql(*, upstream: str, alias: str, materialized: str = "view") -> str:
    """Return a view or table that selects every order column through a table alias."""

    return (
        f"MODEL (description 'Test model.', materialized {materialized});\n\n"
        f"SELECT {alias}.order_id, {alias}.order_date, {alias}.amount_cents "
        f'FROM __ref("{upstream}") AS {alias}\n'
    )


def daily_totals_sql(*, upstream: str, cte: str, comment: str = "", extra_config: str = "") -> str:
    """Return an incremental daily revenue model that imports its upstream through a CTE."""

    return (
        f"{_DAILY_TOTALS_HEADER.format(extra_config=extra_config)}"
        f"{comment}"
        f'WITH {cte} AS (SELECT * FROM __ref("{upstream}"))\n'
        f"SELECT\n    {cte}.order_date,\n    SUM({cte}.amount_cents) AS total_cents\n"
        f"FROM {cte}\nGROUP BY {cte}.order_date\n"
    )


def original_order_models() -> dict[str, str]:
    """Return the connected staging, view, and daily totals models before renaming."""

    return {
        "stg_orders": incremental_orders_sql(),
        "orders_enriched": enriched_view_sql(upstream="stg_orders", alias="o"),
        "daily_order_totals": daily_totals_sql(upstream="orders_enriched", cte="orders_enriched"),
    }


def renamed_order_models() -> dict[str, str]:
    """Return the same connected models renamed, reformatted, and with new storage settings."""

    return {
        "stg_customer_orders": incremental_orders_sql(extra_config="  lookback 1d,\n"),
        "customer_orders_enriched": enriched_view_sql(
            upstream="stg_customer_orders", alias="customer_order"
        ),
        "customer_daily_order_totals": daily_totals_sql(
            upstream="customer_orders_enriched",
            cte="customer_orders_enriched",
            comment="-- Daily revenue per order date.\n",
        ),
    }


def build_original_order_models(*, project_dir: Path, capsys: pytest.CaptureFixture[str]) -> None:
    """Build the connected models over five days of raw orders."""

    write_project(
        project_dir=project_dir,
        models=original_order_models(),
    )
    load_raw_orders(project_dir=project_dir, first_day=1, last_day=5)
    _ = build_ok(project_dir=project_dir, capsys=capsys)


def warehouse_state(*, project_dir: Path) -> tuple[tuple[Any, ...], ...]:
    """Return (schema, relation, row count) for every relation in the dev and prod schemas."""

    tables: list[tuple[Any, ...]] = query(
        project_dir=project_dir,
        sql=(
            "SELECT table_schema, table_name FROM information_schema.tables "
            "WHERE table_schema IN ('dev', 'prod') ORDER BY 1, 2"
        ),
    )
    return tuple(
        (schema, name, row_count(project_dir=project_dir, relation=f"{schema}.{name}"))
        for schema, name in tables
    )


def prepare_prod_rename(*, project_dir: Path, capsys: pytest.CaptureFixture[str]) -> None:
    """Build the origin on prod, then rename it with migrate_from in the project."""

    write_project(
        project_dir=project_dir,
        models={ORIGIN_MODEL: incremental_orders_sql()},
        project_toml=TARGETS_PROJECT_TOML,
    )
    load_raw_orders(project_dir=project_dir, first_day=1, last_day=4)
    built: CliRun = run_sqb(
        project_dir=project_dir, args=("build", "--target", "prod"), capsys=capsys
    )
    assert built.exit_code == 0, built.output
    write_project(
        project_dir=project_dir,
        models={DESTINATION_MODEL: incremental_orders_sql(migrate_from=ORIGIN_MODEL)},
        project_toml=TARGETS_PROJECT_TOML,
    )


def archived_relations(*, project_dir: Path, kind: str, schema: str = "main") -> tuple[str, ...]:
    """Return every janitor-archive relation of one migration kind in one schema."""

    return tuple(
        str(row[0])
        for row in query(
            project_dir=project_dir,
            sql=(
                "SELECT table_name FROM information_schema.tables "
                f"WHERE table_schema = '{schema}' "
                "AND starts_with(table_name, '_sqb_archive__') "
                f"AND contains(table_name, '__{kind}__') ORDER BY 1"
            ),
        )
    )


def archived_order_ids(*, project_dir: Path, kind: str) -> tuple[tuple[int, ...], ...]:
    """Return the order IDs held by each archive of one migration kind."""

    return tuple(
        order_ids(project_dir=project_dir, relation=f"main.{name}")
        for name in archived_relations(project_dir=project_dir, kind=kind)
    )


def prepare_forced_replace(
    *, project_dir: Path, capsys: pytest.CaptureFixture[str], project_toml: str = PROJECT_TOML
) -> None:
    """Build an origin and an independent destination, then force the origin over it."""

    write_project(
        project_dir=project_dir,
        models={
            ORIGIN_MODEL: incremental_orders_sql(),
            DESTINATION_MODEL: incremental_orders_sql(
                select_sql=(
                    'SELECT order_id, order_date, amount_cents FROM __source("raw_orders") '
                    "WHERE order_id <= 2"
                )
            ),
        },
        project_toml=project_toml,
    )
    load_raw_orders(project_dir=project_dir, first_day=1, last_day=5)
    _ = build_ok(project_dir=project_dir, capsys=capsys)
    write_project(
        project_dir=project_dir,
        models={
            DESTINATION_MODEL: incremental_orders_sql(migrate_from=ORIGIN_MODEL, migrate_force=True)
        },
        project_toml=project_toml,
    )


def no_failure(monkeypatch: pytest.MonkeyPatch) -> None:
    """Install no failure."""

    del monkeypatch


def live_relations(*, project_dir: Path, schema: str = "main") -> tuple[str, ...]:
    """Return relations in one schema that are neither state tables nor archives."""

    return tuple(
        str(row[0])
        for row in query(
            project_dir=project_dir,
            sql=(
                "SELECT table_name FROM information_schema.tables "
                f"WHERE table_schema = '{schema}' AND NOT starts_with(table_name, '_') ORDER BY 1"
            ),
        )
    )


_REPLAY_FULL: str = "  replay_on_change full,\n"


def original_replay_order_models(*, middle: str) -> dict[str, str]:
    """Return the staging, enriched, and daily totals chain with full replay on the totals."""

    return {
        "stg_orders": incremental_orders_sql(),
        "orders_enriched": enriched_view_sql(upstream="stg_orders", alias="o", materialized=middle),
        "daily_order_totals": daily_totals_sql(
            upstream="orders_enriched", cte="orders_enriched", extra_config=_REPLAY_FULL
        ),
    }


def renamed_replay_order_models(*, middle: str) -> dict[str, str]:
    """Return the same full-replay chain with every model renamed."""

    return {
        "stg_customer_orders": incremental_orders_sql(),
        "customer_orders_enriched": enriched_view_sql(
            upstream="stg_customer_orders", alias="customer_order", materialized=middle
        ),
        "customer_daily_order_totals": daily_totals_sql(
            upstream="customer_orders_enriched",
            cte="customer_orders_enriched",
            extra_config=_REPLAY_FULL,
        ),
    }


def fail_nothing_in_build(*, monkeypatch: pytest.MonkeyPatch, model_name: str) -> None:
    """Install no build failure."""

    del monkeypatch, model_name


def state_tables(*, project_dir: Path, schema: str) -> tuple[str, ...]:
    """Return the SQLBuild state tables present in one schema."""

    return tuple(
        str(row[0])
        for row in query(
            project_dir=project_dir,
            sql=(
                "SELECT table_name FROM information_schema.tables "
                f"WHERE table_schema = '{schema}' AND starts_with(table_name, '_sqlbuild_') "
                "ORDER BY 1"
            ),
        )
    )


def fingerprint_work() -> tuple[int, int]:
    """Return migration fingerprints computed and distinct requests since the last reset."""

    return migration_fingerprint_work_info()


def reset_fingerprint_work() -> None:
    """Forget migration fingerprint work recorded so far in this process."""

    migration_fingerprint_work_info(reset=True)


def models_with_stored_migration_fingerprints(project_dir: Path) -> tuple[str, ...]:
    """Return model names whose stored fingerprint rows carry a migration fingerprint."""

    return tuple(
        str(row[0])
        for row in query(
            project_dir=project_dir,
            sql=(
                "SELECT DISTINCT node_name FROM main._sqlbuild_fingerprints "
                "WHERE json_extract_string(decode(from_base64(metadata_json_b64)), "
                "'$.migration_fingerprint') IS NOT NULL ORDER BY 1"
            ),
        )
    )


def build_each(
    *,
    project_dir: Path,
    model_sets: tuple[Callable[[], dict[str, str]], ...],
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Build each model set in turn over the current raw orders."""

    models: Callable[[], dict[str, str]]
    for models in model_sets:
        write_project(project_dir=project_dir, models=models())
        _ = build_ok(project_dir=project_dir, capsys=capsys)


def _is_latest_fingerprint_read(sql: str) -> bool:
    return FINGERPRINT_TABLE_NAME in sql and "__sqlbuild_latest_rank" in sql


def record_fingerprint_state_reads(monkeypatch: pytest.MonkeyPatch) -> list[int]:
    """Record the row count of every latest-fingerprint read the DuckDB adapter executes."""

    rows_read: list[int] = []
    original: Callable[..., Any] = DuckDbAdapter._execute

    def recording(self: DuckDbAdapter, *, connection: Any, sql: str) -> Any:
        rows_read.extend(
            int(connection.execute(f"SELECT count(*) FROM ({read_sql})").fetchone()[0])
            for read_sql in filter(_is_latest_fingerprint_read, (sql,))
        )
        return original(self, connection=connection, sql=sql)

    monkeypatch.setattr(DuckDbAdapter, "_execute", recording)
    return rows_read


def fingerprint_history(*, project_dir: Path) -> tuple[int, int]:
    """Return (stored rows, distinct node identities) of the fingerprint state table."""

    row: tuple[Any, ...] = query(
        project_dir=project_dir,
        sql=(
            "SELECT count(*), count(DISTINCT (node_type, node_name)) "
            f"FROM main.{FINGERPRINT_TABLE_NAME}"
        ),
    )[0]
    return int(row[0]), int(row[1])


def edited_original_order_models() -> dict[str, str]:
    """Return the original connected models with a comment edit that rebuilds each one."""

    return {name: f"{sql}-- revised\n" for name, sql in original_order_models().items()}


def declared_rename_models() -> dict[str, str]:
    """Return the original models with the staging model renamed through migrate_from."""

    return {
        DESTINATION_MODEL: incremental_orders_sql(migrate_from=ORIGIN_MODEL),
        "orders_enriched": enriched_view_sql(upstream=DESTINATION_MODEL, alias="o"),
        "daily_order_totals": daily_totals_sql(upstream="orders_enriched", cte="orders_enriched"),
    }


def model_fingerprint_rows(*, project_dir: Path) -> int:
    """Count the model fingerprint rows recorded in the main schema."""

    return int(
        query(
            project_dir=project_dir,
            sql="SELECT COUNT(*) FROM main._sqlbuild_fingerprints WHERE node_type = 'model'",
        )[0][0]
    )
