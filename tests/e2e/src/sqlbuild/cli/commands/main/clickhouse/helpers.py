"""Helpers for ClickHouse CLI E2E tests."""

from __future__ import annotations

import functools
import re
import subprocess
import uuid
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path
from textwrap import dedent
from typing import Any, cast

from sqlbuild.adapters.clickhouse.classes.clickhouse_adapter import ClickHouseAdapter
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    REPO_ROOT,
    prepare_inline_project,
    prepare_waffle_shop,
    run_sqb,
)
from tests.integration.src.sqlbuild.cli.recall.helpers import (
    killed_build,
    killed_during_discovery,
    run,
    start,
    wait_for,
    write_project,
)

CLICKHOUSE_IMAGE: str = "clickhouse/clickhouse-server:26.1"
_RECALL_SCHEMA_CONSTANTS: Path = REPO_ROOT / "crates/sqlbuild-recall/src/schema/constants.rs"
_RECALL_SCHEMA_VERSION_PATTERN: re.Pattern[str] = re.compile(
    r"pub const RECALL_SCHEMA_VERSION: i64 = (\d+);"
)


def _recall_schema_version() -> int:
    """The Recall schema version this build writes, as `RECALL_SCHEMA_VERSION` declares it."""

    (version,) = _RECALL_SCHEMA_VERSION_PATTERN.findall(
        _RECALL_SCHEMA_CONSTANTS.read_text(encoding="utf-8")
    )
    return int(version)


RECALL_SCHEMA_VERSION: int = _recall_schema_version()


def build_unique_schema_name(*, prefix: str) -> str:
    return f"{prefix}_{uuid.uuid4().hex[:10]}"


def build_clickhouse_project_toml(
    *,
    project_name: str,
    schema_name: str,
    config: dict[str, object],
    extra: str = "",
) -> str:
    return (
        f'name = "{project_name}"\n'
        'adapter = "clickhouse"\n'
        'default_target = "dev"\n\n'
        "[connection]\n"
        f'host = "{config["host"]}"\n'
        f"port = {config['port']}\n"
        f'user = "{config["user"]}"\n'
        f'password = "{config["password"]}"\n\n'
        "[targets.dev]\n"
        f'schema = "{schema_name}"\n\n'
        "[defaults]\n"
        'materialized = "table"\n' + extra
    )


def execute_clickhouse_sql(*, config: dict[str, object], statements: tuple[str, ...]) -> None:
    adapter: ClickHouseAdapter = ClickHouseAdapter()
    connection: Any = adapter.connect(dict(config))
    try:
        for statement in statements:
            adapter.execute(connection=connection, sql=statement)
    finally:
        adapter.close(connection)


def fetch_clickhouse_rows(*, config: dict[str, object], sql: str) -> tuple[tuple[object, ...], ...]:
    adapter: ClickHouseAdapter = ClickHouseAdapter()
    connection: Any = adapter.connect(dict(config))
    try:
        cursor: Any = adapter.execute(connection=connection, sql=sql)
        return tuple(tuple(row) for row in cursor.fetchall())
    finally:
        adapter.close(connection)


def create_clickhouse_schema(*, config: dict[str, object], schema_name: str) -> None:
    execute_clickhouse_sql(
        config=config, statements=(f"CREATE DATABASE IF NOT EXISTS {schema_name}",)
    )


def cleanup_clickhouse_schema(*, config: dict[str, object], schema_name: str) -> None:
    rows: tuple[tuple[object, ...], ...] = fetch_clickhouse_rows(
        config=config,
        sql=(
            "SELECT name FROM system.functions WHERE origin = 'SQLUserDefined' "
            f"AND startsWith(name, '{schema_name}__')"
        ),
    )
    execute_clickhouse_sql(
        config=config,
        statements=(
            *(f"DROP FUNCTION IF EXISTS {row[0]}" for row in rows),
            f"DROP DATABASE IF EXISTS {schema_name} SYNC",
        ),
    )


def build_two_target_toml(
    *, project_name: str, prod_schema: str, dev_schema: str, config: dict[str, object]
) -> str:
    return (
        f'name = "{project_name}"\n'
        'adapter = "clickhouse"\n'
        'default_target = "dev"\n\n'
        "[connection]\n"
        f'host = "{config["host"]}"\n'
        f"port = {config['port']}\n"
        f'user = "{config["user"]}"\n'
        f'password = "{config["password"]}"\n\n'
        "[targets.prod]\n"
        f'schema = "{prod_schema}"\n\n'
        "[targets.prod.clone]\n"
        "allow_as_clone_destination = true\n\n"
        "[targets.dev]\n"
        f'schema = "{dev_schema}"\n\n'
        "[targets.dev.clone]\n"
        "allow_as_clone_origin = true\n"
    )


def build_orders_project_files(*, schema_name: str, config: dict[str, object]) -> dict[str, str]:
    return {
        "sqlbuild_project.toml": build_clickhouse_project_toml(
            project_name="clickhouse_orders",
            schema_name=schema_name,
            config=config,
            extra="\n[settings]\nconcurrency = 4\nmicrobatch_concurrency = true\n",
        ),
        "sources/raw.yml": dedent(
            f"""
            sources:
              - name: raw_events
                description: Synthetic order events.
                schema: {schema_name}
                table: raw_events
              - name: raw_customers
                description: Customer versions kept by a replacing engine.
                schema: {schema_name}
                table: raw_customers
                final: true
            """
        ).lstrip(),
        "seeds/regions.csv": "customer_id,region_name\n1,north\n2,south\n3,east\n",
        "seeds/regions.yml": dedent(
            """
            seeds:
              - name: regions
                description: Customer regions.
                columns:
                  - name: customer_id
                    type: Int64
                  - name: region_name
                    type: String
            """
        ).lstrip(),
        "functions/sql/half_amount.sql": (
            'FUNCTION (\n  description "Half of an amount",\n'
            "  arguments (amount Int64),\n  returns Float64,\n);\n\namount / 2\n"
        ),
        "models/stg_events.sql": (
            "MODEL (description 'Staged events.', materialized view);\n\n"
            "SELECT event_id, customer_id, amount, event_time\n"
            'FROM __source("raw_events")\n'
        ),
        "models/customer_totals.sql": (
            "MODEL (description 'One row per customer.');\n\n"
            "SELECT e.customer_id AS customer_id, r.region_name AS region_name,\n"
            "  sum(e.amount) AS total_amount,\n"
            '  __udf("half_amount")(sum(e.amount)) AS half_total\n'
            'FROM __ref("stg_events") AS e\n'
            'JOIN __seed("regions") AS r ON e.customer_id = r.customer_id\n'
            "GROUP BY e.customer_id, r.region_name\n"
        ),
        "models/customer_names.sql": (
            "MODEL (description 'One row per customer name.');\n\n"
            'SELECT customer_id, customer_name FROM __source("raw_customers")\n'
        ),
        "models/customer_latest.sql": (
            "MODEL (description 'Latest activity per customer.',\n"
            "  materialized incremental,\n"
            "  incremental_strategy merge,\n"
            "  unique_key [customer_id],\n"
            ");\n\n"
            "SELECT customer_id, count() AS event_count, max(event_time) AS last_event_time\n"
            'FROM __ref("stg_events")\n'
            "GROUP BY customer_id\n"
        ),
        "models/daily_events.sql": (
            "MODEL (description 'Events by day.',\n"
            "  materialized incremental,\n"
            "  incremental_strategy delete_insert,\n"
            "  incremental_mode microbatch,\n"
            "  microbatch_strategy watermark,\n"
            "  cursor_watermark_mode all,\n"
            "  cursor event_time,\n"
            "  cursor_type timestamp,\n"
            "  cursor_grain hour,\n"
            "  cursor_inputs (\n"
            "    raw_events (column event_time, roles [filter, watermark]),\n"
            "  ),\n"
            "  batch_size 1d,\n"
            "  batch_concurrency 4,\n"
            ");\n\n"
            "SELECT event_id, customer_id, amount, event_time\n"
            'FROM __source("raw_events")\n'
            "WHERE event_time >= __cursor_start() AND event_time < __cursor_end()\n"
        ),
    }


def build_partitioned_events_project_files(
    *, schema_name: str, config: dict[str, object], partition_by: str = "toYYYYMM(event_time)"
) -> dict[str, str]:
    return {
        "sqlbuild_project.toml": build_clickhouse_project_toml(
            project_name="clickhouse_layout", schema_name=schema_name, config=config
        ),
        "sources/raw.yml": dedent(
            f"""
            sources:
              - name: raw_events
                description: Synthetic order events.
                schema: {schema_name}
                table: raw_events
            """
        ).lstrip(),
        "models/events.sql": (
            "MODEL (description 'One row per event.',\n"
            "  materialized incremental,\n"
            "  incremental_strategy delete_insert,\n"
            "  unique_key [event_id],\n"
            "  cursor event_time,\n"
            "  cursor_type timestamp,\n"
            "  cursor_grain hour,\n"
            "  cursor_inputs (\n"
            "    raw_events event_time,\n"
            "  ),\n"
            "  engine 'ReplacingMergeTree(version)',\n"
            "  order_by [event_id, event_time],\n"
            "  partition_by 'toYYYYMM(event_time)',\n"
            "  primary_key [event_id],\n"
            "  table_settings (index_granularity 4096),\n"
            ");\n\n"
            "SELECT event_id, amount, event_time, toInt64(1) AS version\n"
            'FROM __source("raw_events")\n'
            "WHERE event_time >= __cursor_start() AND event_time < __cursor_end()\n"
        ),
    }


def active_parts_by_partition(
    *, config: dict[str, object], schema_name: str, table: str
) -> dict[str, tuple[str, ...]]:
    rows: tuple[tuple[object, ...], ...] = fetch_clickhouse_rows(
        config=config,
        sql=(
            "SELECT partition, arraySort(groupArray(name)) FROM system.parts "
            f"WHERE active AND database = '{schema_name}' AND table = '{table}' "
            "GROUP BY partition ORDER BY partition"
        ),
    )
    return {str(row[0]): tuple(map(str, cast(list[object], row[1]))) for row in rows}


def prepare_clickhouse_waffle_shop(
    *, tmp_path: Path, config: dict[str, object]
) -> tuple[Path, str]:
    """Copy waffle shop to tmp dir and wire it to a unique ClickHouse database."""

    schema_name: str = build_unique_schema_name(prefix="sqb_waffle")
    project_dir: Path = prepare_waffle_shop(tmp_path)
    (project_dir / "functions" / "sql" / "customer_orders.sql").unlink()
    (project_dir / "tests" / "unit" / "test_customer_orders_table_fn.sql").unlink()
    (project_dir / "functions" / "python" / "is_completed_order_py.py").unlink()
    fact_orders_path: Path = project_dir / "models" / "marts" / "fact_orders.sql"
    fact_orders_path.write_text(
        fact_orders_path.read_text(encoding="utf-8").replace(
            '__udf("is_completed_order_py")(o.status) AS is_completed_order_py,',
            '__udf("is_completed_order")(o.status) AS is_completed_order_py,',
        ),
        encoding="utf-8",
    )
    for model_name in ("dim_customers", "fact_orders", "scenario_order_prices"):
        _alias_qualified_columns(project_dir / "models" / "marts" / f"{model_name}.sql")
    materialization_path: Path = project_dir / "materializations" / "partition_tracked.py"
    materialization_path.write_text(
        materialization_path.read_text(encoding="utf-8").replace(
            "CURRENT_TIMESTAMP", "CURRENT_TIMESTAMP()"
        ),
        encoding="utf-8",
    )
    time_macros_path: Path = project_dir / "models" / "marts" / "_macros" / "python" / "time.py"
    time_macros_path.write_text(
        time_macros_path.read_text(encoding="utf-8").replace(
            "return f\"DATE_TRUNC('{grain}', {expression})\"",
            "return f\"CAST(DATE_TRUNC('{grain}', {expression}) AS TIMESTAMP)\"",
        ),
        encoding="utf-8",
    )
    (project_dir / "sqlbuild_project.toml").write_text(
        build_clickhouse_project_toml(
            project_name="waffle_shop",
            schema_name=schema_name,
            config=config,
            extra='\n[settings]\ndefault_audit_severity = "warn"\n\n'
            '[path_defaults.staging]\nmaterialized = "view"\n',
        ),
        encoding="utf-8",
    )
    return project_dir, schema_name


def _alias_qualified_columns(model_path: Path) -> None:
    """Alias bare qualified columns; ClickHouse names them ``alias.column`` in multi-joins."""

    model_path.write_text(
        re.sub(
            r"^(\s+)(\w+)\.(\w+),$",
            r"\1\2.\3 AS \3,",
            model_path.read_text(encoding="utf-8"),
            flags=re.MULTILINE,
        ),
        encoding="utf-8",
    )


RECALL_TABLES: tuple[str, ...] = (
    "recall_meta",
    "recall_invocations",
    "recall_output",
    "recall_events",
    "recall_diagnostics",
    "recall_artifacts",
    "recall_artifact_refs",
    "recall_heartbeats",
    "recall_delivery",
    "recall_conflicts",
)
_TTL_DAYS: re.Pattern[str] = re.compile(r" TTL \w+ \+ toIntervalDay\((\d+)\)")
_AGED_TABLES: tuple[tuple[str, str], ...] = (
    ("recall_invocations", ""),
    ("recall_output", ""),
    ("recall_events", ", concat('aged-', event_id) AS event_id"),
    ("recall_diagnostics", ""),
    ("recall_artifact_refs", ""),
    ("recall_heartbeats", ""),
    ("recall_delivery", ""),
    ("recall_conflicts", ""),
)


def recall_store_toml(
    *,
    config: dict[str, object],
    database: str,
    recall_settings: str = "",
    connection_settings: str = "",
) -> str:
    """Project configuration sending Recall history to a ClickHouse database."""

    return (
        "\n[connections.history]\n"
        'adapter = "clickhouse"\n'
        f'host = "{config["host"]}"\n'
        f"port = {config['port']}\n"
        f'user = "{config["user"]}"\n'
        f'password = "{config["password"]}"\n'
        f'database = "{database}"\n'
        f"{connection_settings}"
        "\n[recall]\n"
        'write_connection = "history"\n'
        'read_connection = "history"\n'
        f"{recall_settings}"
    )


def stored_ttl_days(*, config: dict[str, object], database: str) -> tuple[tuple[str, int], ...]:
    """Each Recall table's TTL in days, by table name."""

    rows: tuple[tuple[object, ...], ...] = fetch_clickhouse_rows(
        config=config,
        sql=(
            "SELECT name, create_table_query FROM system.tables "
            f"WHERE database = '{database}' AND startsWith(name, 'recall_') ORDER BY name"
        ),
    )
    return tuple(
        (str(name), int(next(iter(_TTL_DAYS.findall(str(query))), -1))) for name, query in rows
    )


def physical_rows(*, config: dict[str, object], database: str, where: str = "1") -> int:
    """Rows stored across every Recall table, before any deduplication or retention filter."""

    total: int = 0
    for table in RECALL_TABLES:
        rows: tuple[tuple[object, ...], ...] = fetch_clickhouse_rows(
            config=config, sql=f"SELECT count() FROM {database}.{table} WHERE {where}"
        )
        total += int(cast(int, rows[0][0]))
    return total


def age_recorded_invocations(*, config: dict[str, object], database: str, days: int) -> None:
    """Store a copy of every invocation's rows started `days` earlier, under `aged-` IDs."""

    statements: list[str] = []
    for table, extra in _AGED_TABLES:
        replaced: str = (
            "concat('aged-', invocation_id) AS invocation_id, "
            f"invocation_started_at - INTERVAL {days} DAY AS invocation_started_at{extra}"
        )
        statements.append(
            f"INSERT INTO {database}.{table} SELECT * REPLACE ({replaced}) FROM "
            f"(SELECT * FROM {database}.{table} WHERE NOT startsWith(invocation_id, 'aged-'))"
        )
    execute_clickhouse_sql(config=config, statements=tuple(statements))


def age_stored_artifacts(*, config: dict[str, object], database: str, days: int) -> None:
    """Move every stored SQL text's `seen_at` `days` earlier, as if not refreshed since."""

    table: str = f"{database}.recall_artifacts"
    execute_clickhouse_sql(
        config=config,
        statements=(
            f"INSERT INTO {table} SELECT * REPLACE (seen_at - INTERVAL {days} DAY AS seen_at) "
            f"FROM {table}",
            f"ALTER TABLE {table} DELETE WHERE seen_at > now64(6) - INTERVAL {days - 1} DAY "
            "SETTINGS mutations_sync = 2",
        ),
    )


def expire_recall_parts(*, config: dict[str, object], database: str) -> None:
    """Apply every Recall table's TTL now, as background TTL merges eventually would."""

    execute_clickhouse_sql(
        config=config,
        statements=tuple(
            f"ALTER TABLE {database}.{table} MATERIALIZE TTL SETTINGS mutations_sync = 2"
            for table in RECALL_TABLES
        ),
    )


def apply_printed_retention_fix(*, config: dict[str, object], stderr: str) -> int:
    """Run the statements a retention notice printed without expiring stored parts."""

    script: str = stderr.split("configured retention with:\n\n", 1)[1].split("\nsqb ", 1)[0]
    statements: list[str] = [
        statement.strip().removesuffix(";") for statement in script.split(";\n\n")
    ]
    import clickhouse_connect

    client: Any = clickhouse_connect.get_client(
        host=str(config["host"]),
        port=int(cast(int, config["port"])),
        username=str(config["user"]),
        password=str(config["password"]),
    )
    try:
        for statement in statements:
            client.command(statement, settings={"materialize_ttl_after_modify": 0})
    finally:
        client.close()
    return len(statements)


FIRST_RELEASE_EDITS: tuple[tuple[str, str], ...] = (
    (
        "    `pid` Nullable(Int64),\n    `command` Nullable(String),\n"
        "    `subcommand` Nullable(String),",
        "    `pid` Nullable(Int64),",
    ),
    (
        "CAST(command AS Nullable(String)) AS beat_command",
        "CAST(NULL AS Nullable(String)) AS beat_command",
    ),
    (
        "CAST(subcommand AS Nullable(String)) AS beat_subcommand",
        "CAST(NULL AS Nullable(String)) AS beat_subcommand",
    ),
)


def apply_script(*, config: dict[str, object], script: str) -> None:
    """Run, as an administrator, a script of `;`-terminated statements."""

    execute_clickhouse_sql(
        config=config,
        statements=tuple(
            statement.strip().removesuffix(";") for statement in script.split(";\n\n")
        ),
    )


def apply_printed_ddl(
    *, config: dict[str, object], stderr: str, edits: tuple[tuple[str, str], ...] = ()
) -> None:
    """Run, as an administrator, the script a permission notice printed, with `edits` applied."""

    printed: str = stderr.split(" with:\n\n", 1)[1].split("\nsqb ", 1)[0].strip()
    script: str = functools.reduce(lambda text, edit: text.replace(*edit), edits, printed)
    execute_clickhouse_sql(
        config=config,
        statements=tuple(
            statement.strip().removesuffix(";") for statement in script.split(";\n\n")
        ),
    )


@contextmanager
def limited_user(
    *,
    config: dict[str, object],
    database: str,
    revoked_inserts: tuple[str, ...] = (),
    privileges: str = "SELECT, INSERT",
) -> Iterator[dict[str, object]]:
    """A user with only `privileges` on `database`, less `revoked_inserts`."""

    name: str = build_unique_schema_name(prefix="recall_writer")
    password: str = uuid.uuid4().hex
    execute_clickhouse_sql(
        config=config,
        statements=(
            f"CREATE USER {name} IDENTIFIED BY '{password}'",
            f"GRANT {privileges} ON {database}.* TO {name}",
            *(f"REVOKE INSERT ON {database}.{table} FROM {name}" for table in revoked_inserts),
        ),
    )
    try:
        yield {**config, "user": name, "password": password}
    finally:
        execute_clickhouse_sql(config=config, statements=(f"DROP USER IF EXISTS {name}",))


def replicate_invocations(
    *, config: dict[str, object], database: str, copies: int, spacing_minutes: int
) -> None:
    """Store `copies` more invocations shaped like each recorded one, `spacing_minutes` apart."""

    shift: str = f"toIntervalMinute(n * {spacing_minutes})"
    shifted: dict[str, str] = {
        "recall_events": (
            ", if(statement_id IS NULL, NULL, concat(statement_id, '-', toString(n))) "
            "AS statement_id, concat(event_id, '-', toString(n)) AS event_id, "
            f"occurred_at - {shift} AS occurred_at"
        ),
        "recall_invocations": "",
        "recall_heartbeats": f", beat_at - {shift} AS beat_at, stale_at - {shift} AS stale_at",
    }
    execute_clickhouse_sql(
        config=config,
        statements=tuple(
            f"INSERT INTO {database}.{table} SELECT * EXCEPT (n) REPLACE ("
            "concat(invocation_id, '-', toString(n)) AS invocation_id, "
            f"invocation_started_at - {shift} AS invocation_started_at{extra}) "
            f"FROM (SELECT * FROM {database}.{table}) AS source "
            f"CROSS JOIN (SELECT number + 1 AS n FROM numbers({copies})) AS copies "
            "SETTINGS insert_deduplicate = 0, max_partitions_per_insert_block = 1000"
            for table, extra in shifted.items()
        ),
    )


def duplicate_recall_rows(*, config: dict[str, object], database: str) -> None:
    """Deliver every stored Recall row a second time, as a retried insert would."""

    execute_clickhouse_sql(
        config=config,
        statements=tuple(
            f"INSERT INTO {database}.{table} SETTINGS insert_deduplicate = 0 "
            f"SELECT * FROM {database}.{table}"
            for table in RECALL_TABLES
        ),
    )


def write_recall_project(
    *,
    parent: Path,
    config: dict[str, object],
    database: str,
    settings: tuple[str, str] = ("", ""),
    targets: int = 1,
) -> Path:
    """Write the local Recall test project, sending its history to `database`."""

    recall_settings, connection_settings = settings
    return write_project(
        parent / "orders_shop",
        extra_toml=recall_store_toml(
            config=config,
            database=database,
            recall_settings=recall_settings,
            connection_settings=connection_settings,
        ),
        targets=targets,
    )


_PREVIOUS_ORDER: str = "tuple(invocation_sequence, occurred_at)"
_PREVIOUS_STATEMENT_TERMINAL: str = "event_type IN ('statement_completed', 'statement_failed')"


def _previous_read(*, database: str, table: str, expiry: str, days: int) -> str:
    return (
        f"(SELECT * FROM `{database}`.`{table}` FINAL "
        f"WHERE {expiry} >= now64(6) - INTERVAL {days} DAY)"
    )


def previous_view_bodies(*, database: str, retention_days: int = 30) -> dict[str, str]:
    """The bodies of the views the previous schema version created, which this one replaces."""

    def read(table: str, expiry: str = "invocation_started_at", extra_days: int = 0) -> str:
        return _previous_read(
            database=database, table=table, expiry=expiry, days=retention_days + extra_days
        )

    context: str = read("recall_invocations")
    events: str = read("recall_events")
    beats: str = read("recall_heartbeats")
    output: str = read("recall_output")
    delivery: str = read("recall_delivery")
    refs: str = read("recall_artifact_refs")
    stored: str = read("recall_artifacts", expiry="seen_at", extra_days=7)
    order: str = _PREVIOUS_ORDER
    terminal: str = _PREVIOUS_STATEMENT_TERMINAL
    return {
        "invocations": f"""WITH
ids AS (
    SELECT invocation_id, invocation_started_at
    FROM (
        SELECT invocation_id, invocation_started_at FROM {context}
        UNION ALL SELECT invocation_id, invocation_started_at FROM {beats}
        UNION ALL SELECT invocation_id, invocation_started_at FROM {events}
            WHERE event_type IN ('invocation_started', 'invocation_completed', 'invocation_failed')
    )
    GROUP BY invocation_id, invocation_started_at
),
started AS (
    SELECT invocation_id, invocation_started_at, min(occurred_at) AS started_at,
        any(command) AS command
    FROM {events} WHERE event_type = 'invocation_started'
    GROUP BY invocation_id, invocation_started_at
),
terminals AS (
    SELECT invocation_id, invocation_started_at,
        argMax(event_type, {order}) AS terminal_type,
        argMax(occurred_at, {order}) AS terminal_at,
        argMax(exit_code, {order}) AS terminal_exit_code,
        argMax(duration_ms, {order}) AS terminal_duration_ms,
        argMax(error_type, {order}) AS terminal_error_type,
        argMax(error_code, {order}) AS terminal_error_code,
        argMax(JSONExtract(payload, 'error_message', 'Nullable(String)'),
            {order}) AS terminal_error_message
    FROM {events}
    WHERE event_type IN ('invocation_completed', 'invocation_failed')
    GROUP BY invocation_id, invocation_started_at
),
beats AS (
    SELECT invocation_id, invocation_started_at, max(beat_at) AS last_heartbeat_at,
        max(stale_at) AS stale_at, min(command) AS command, min(subcommand) AS subcommand
    FROM {beats} GROUP BY invocation_id, invocation_started_at
)
SELECT
    ids.invocation_id AS invocation_id,
    ids.invocation_started_at AS invocation_started_at,
    coalesce(started.started_at, ids.invocation_started_at) AS started_at,
    multiIf(
        terminals.terminal_type = 'invocation_completed', 'completed',
        terminals.terminal_type = 'invocation_failed', 'failed',
        beats.stale_at IS NULL, 'unknown',
        beats.stale_at >= now64(6), 'running',
        'abandoned'
    ) AS status,
    if(terminals.terminal_type IS NOT NULL, terminals.terminal_at,
        if(beats.stale_at < now64(6), beats.last_heartbeat_at, NULL)) AS ended_at,
    if(terminals.terminal_type IS NOT NULL, terminals.terminal_duration_ms,
        if(beats.stale_at < now64(6),
            toFloat64(dateDiff('millisecond', started_at, beats.last_heartbeat_at)), NULL))
        AS duration_ms,
    terminals.terminal_exit_code AS exit_code,
    terminals.terminal_error_type AS error_type,
    terminals.terminal_error_code AS error_code,
    terminals.terminal_error_message AS error_message,
    coalesce(context.command, started.command, beats.command) AS command,
    coalesce(context.subcommand, beats.subcommand) AS subcommand,
    context.target AS target,
    context.adapter AS adapter,
    context.project_name AS project_name,
    context.selection AS selection,
    context.exclusion AS exclusion,
    context.sqlbuild_version AS sqlbuild_version,
    context.git_revision AS git_revision,
    context.git_dirty AS git_dirty,
    context.host_token AS host_token,
    context.pid AS pid,
    context.external_context AS external_context,
    context.recall_filter_active AS recall_filter_active,
    beats.last_heartbeat_at AS last_heartbeat_at
FROM ids
LEFT JOIN {context} AS context ON context.invocation_id = ids.invocation_id
    AND context.invocation_started_at = ids.invocation_started_at
LEFT JOIN started ON started.invocation_id = ids.invocation_id
    AND started.invocation_started_at = ids.invocation_started_at
LEFT JOIN terminals ON terminals.invocation_id = ids.invocation_id
    AND terminals.invocation_started_at = ids.invocation_started_at
LEFT JOIN beats ON beats.invocation_id = ids.invocation_id
    AND beats.invocation_started_at = ids.invocation_started_at
SETTINGS join_use_nulls = 1""",
        "statements": f"""WITH
facts AS (
    SELECT * FROM {events} WHERE statement_id IS NOT NULL AND startsWith(event_type, 'statement_')
),
folded AS (
    SELECT invocation_id, invocation_started_at, statement_id,
        argMin(run_id, {order}) AS first_run_id,
        argMin(resource_id, {order}) AS first_resource_id,
        argMin(resource_attempt_id, {order}) AS first_resource_attempt_id,
        argMin(operation_id, {order}) AS first_operation_id,
        argMin(intent, {order}) AS first_intent,
        argMin(statement_kind, {order}) AS first_statement_kind,
        argMin(purpose, {order}) AS first_purpose,
        argMin(sql_digest, {order}) AS first_sql_digest,
        coalesce(min(started_at), minIf(occurred_at, event_type = 'statement_started'))
            AS statement_started_at,
        minIf(occurred_at, event_type = 'statement_submitted') AS submitted_at,
        max(query_id) AS any_query_id,
        countIf({terminal}) AS terminal_count,
        argMaxIf(event_type, {order}, {terminal}) AS last_event_type,
        argMaxIf(occurred_at, {order}, {terminal}) AS last_ended_at,
        argMaxIf(query_id, {order}, {terminal}) AS last_query_id,
        argMaxIf(error_type, {order}, {terminal}) AS last_error_type,
        argMaxIf(error_code, {order}, {terminal}) AS last_error_code,
        argMaxIf(duration_ms, {order}, {terminal}) AS last_duration_ms,
        argMaxIf(row_count, {order}, {terminal}) AS last_row_count,
        argMaxIf(affected_rows, {order}, {terminal}) AS last_affected_rows
    FROM facts GROUP BY invocation_id, invocation_started_at, statement_id
),
resources AS (
    SELECT invocation_id, invocation_started_at, resource_id,
        min(resource_name) AS first_resource_name, min(resource_kind) AS first_resource_kind
    FROM {events}
    WHERE resource_id IS NOT NULL AND resource_name IS NOT NULL
    GROUP BY invocation_id, invocation_started_at, resource_id
)
SELECT
    folded.invocation_id AS invocation_id,
    folded.invocation_started_at AS invocation_started_at,
    folded.statement_id AS statement_id,
    folded.first_run_id AS run_id,
    folded.first_resource_id AS resource_id,
    resources.first_resource_name AS resource_name,
    resources.first_resource_kind AS resource_kind,
    folded.first_resource_attempt_id AS resource_attempt_id,
    folded.first_operation_id AS operation_id,
    folded.first_intent AS intent,
    folded.first_statement_kind AS statement_kind,
    folded.first_purpose AS purpose,
    if(folded.terminal_count > 0, coalesce(folded.last_query_id, folded.any_query_id),
        folded.any_query_id) AS query_id,
    folded.statement_started_at AS started_at,
    folded.submitted_at AS submitted_at,
    if(folded.terminal_count > 0, folded.last_ended_at, NULL) AS ended_at,
    multiIf(
        folded.terminal_count = 0, 'no_terminal',
        folded.last_event_type = 'statement_completed', 'completed',
        'failed'
    ) AS outcome,
    if(folded.terminal_count > 0, folded.last_error_type, NULL) AS error_type,
    if(folded.terminal_count > 0, folded.last_error_code, NULL) AS error_code,
    if(folded.terminal_count > 0, folded.last_duration_ms, NULL) AS duration_ms,
    if(folded.terminal_count > 0, folded.last_row_count, NULL) AS row_count,
    if(folded.terminal_count > 0, folded.last_affected_rows, NULL) AS affected_rows,
    folded.first_sql_digest AS sql_digest,
    concat('sha256:', folded.first_sql_digest)
        IN (SELECT digest FROM `{database}`.`recall_artifacts`) AS sql_available
FROM folded
LEFT JOIN resources
    ON resources.invocation_id = folded.invocation_id
    AND resources.invocation_started_at = folded.invocation_started_at
    AND resources.resource_id = folded.first_resource_id
SETTINGS join_use_nulls = 1""",
        "output_invocations": f"""WITH
received AS (
    SELECT invocation_id,
        min(invocation_started_at) AS invocation_started_at,
        count() AS chunks,
        sum(byte_count) AS bytes,
        sumIf(byte_count, stream = 'stdout') AS stdout_bytes,
        sumIf(byte_count, stream = 'stderr') AS stderr_bytes,
        max(sequence) AS max_sequence,
        sum(dropped_before) AS queue_drops,
        sum(coalesce(omitted_sequences, 1)) AS spans,
        sum(coalesce(omitted_bytes, 0)) AS truncated_bytes
    FROM {output} GROUP BY invocation_id
),
delivery AS (SELECT * FROM {delivery} WHERE stream = 'output'),
ids AS (
    SELECT DISTINCT invocation_id, invocation_started_at
    FROM (
        SELECT invocation_id, invocation_started_at FROM received
        UNION ALL SELECT invocation_id, invocation_started_at FROM delivery
    )
),
counted AS (
    SELECT
        ids.invocation_id AS invocation_id,
        ids.invocation_started_at AS invocation_started_at,
        coalesce(received.chunks, 0) AS chunks,
        coalesce(received.bytes, 0) AS bytes,
        coalesce(received.stdout_bytes, 0) AS stdout_bytes,
        coalesce(received.stderr_bytes, 0) AS stderr_bytes,
        coalesce(received.truncated_bytes, 0) AS truncated_bytes,
        received.max_sequence AS max_sequence,
        greatest(
            coalesce(toInt64(received.max_sequence) + 1 - toInt64(received.spans), 0),
            coalesce(toInt64(delivery.accepted), 0) - coalesce(toInt64(received.chunks), 0)
        ) AS missing_sequences,
        greatest(coalesce(received.queue_drops, 0), coalesce(delivery.dropped, 0))
            AS reported_drops,
        coalesce(delivery.filtered, 0) AS filtered,
        coalesce(delivery.failed, 0) AS failed,
        coalesce(context.recall_filter_active, 0) AS filter_active,
        coalesce(delivery.flush_complete, 0) AS flush_complete,
        delivery.invocation_id IS NOT NULL AS delivery_reported,
        coalesce(delivery.delivered, 0) AS delivered
    FROM ids
    LEFT JOIN received ON received.invocation_id = ids.invocation_id
    LEFT JOIN delivery ON delivery.invocation_id = ids.invocation_id
    LEFT JOIN {context} AS context ON context.invocation_id = ids.invocation_id
    SETTINGS join_use_nulls = 1
)
SELECT
    invocation_id, invocation_started_at, chunks, bytes, stdout_bytes, stderr_bytes,
    truncated_bytes, max_sequence, missing_sequences, reported_drops, filtered, failed,
    filter_active, flush_complete,
    multiIf(
        missing_sequences = 0 AND truncated_bytes > 0, 'truncated',
        missing_sequences = 0, NULL,
        filtered >= missing_sequences, 'filtered',
        filtered + reported_drops + failed >= missing_sequences, 'dropped',
        'unexplained'
    ) AS gap_reason,
    toUInt8(delivery_reported AND flush_complete = 1 AND missing_sequences = 0
        AND reported_drops = 0 AND filtered = 0 AND failed = 0 AND truncated_bytes = 0
        AND chunks >= delivered)
        AS complete
FROM counted""",
        "artifacts": f"""SELECT
    refs.invocation_id AS invocation_id,
    refs.invocation_started_at AS invocation_started_at,
    refs.digest AS digest,
    refs.ref_id AS ref_id,
    refs.role AS role,
    refs.statement_id AS statement_id,
    refs.resource_id AS resource_id,
    artifact.kind AS kind,
    artifact.media_type AS media_type,
    artifact.size AS size,
    artifact.digest IS NOT NULL AS available,
    if(artifact.digest IS NULL, 'expired_or_not_captured', NULL) AS unavailable_reason,
    if(startsWith(artifact.media_type, 'text/'), toValidUTF8(artifact.content), NULL) AS text,
    artifact.content AS content
FROM {refs} AS refs
LEFT JOIN {stored} AS artifact ON artifact.digest = refs.digest
SETTINGS join_use_nulls = 1""",
    }


_PLAN_READ: re.Pattern[str] = re.compile(r"ReadFromMergeTree \(([^)]+)\)")
_PRIMARY_KEY_INDEX: re.Pattern[str] = re.compile(
    r"^\s*PrimaryKey\n\s*Keys:\n(?:\s*\w+\n)*?\s*invocation_id\n", re.MULTILINE
)
RECALL_VIEWS: tuple[str, ...] = (
    "invocations",
    "statements",
    "audit_results",
    "output_invocations",
    "output_tail",
    "output_transcripts",
    "events",
    "diagnostics",
    "artifacts",
    "conflicts",
)


def fetch_with_settings(
    *, config: dict[str, object], sql: str, settings: dict[str, object]
) -> tuple[tuple[object, ...], ...]:
    """Rows of `sql` from a plain client sending only `settings`, none of SQLBuild's."""

    import clickhouse_connect

    client: Any = clickhouse_connect.get_client(
        host=str(config["host"]),
        port=int(cast(int, config["port"])),
        username=str(config["user"]),
        password=str(config["password"]),
        settings=settings,
    )
    try:
        return tuple(tuple(row) for row in client.query(sql).result_rows)
    finally:
        client.close()


def differing_rows(
    *,
    config: dict[str, object],
    left: str,
    right: str,
    settings: dict[str, object],
    columns: tuple[str, ...] = (),
) -> int:
    """Rows of `left` missing from `right` plus rows of `right` missing from `left`, over `columns`
    when given, else over every column."""

    selected: str = ", ".join(f"`{column}`" for column in columns) or "*"
    rows: tuple[tuple[object, ...], ...] = fetch_with_settings(
        config=config,
        sql=(
            f"SELECT (SELECT count() FROM (SELECT {selected} FROM {left} EXCEPT DISTINCT "
            f"SELECT {selected} FROM {right})) + (SELECT count() FROM (SELECT {selected} "
            f"FROM {right} EXCEPT DISTINCT SELECT {selected} FROM {left}))"
        ),
        settings=settings,
    )
    return int(cast(int, rows[0][0]))


def described(*, config: dict[str, object], relation: str) -> tuple[tuple[object, ...], ...]:
    """Each column's name and type, in order."""

    return tuple(
        (row[0], row[1])
        for row in fetch_clickhouse_rows(config=config, sql=f"DESCRIBE TABLE {relation}")
    )


def column_names(*, config: dict[str, object], relation: str) -> tuple[str, ...]:
    """Each column's name, in order."""

    return tuple(str(name) for name, _ in described(config=config, relation=relation))


def described_except(
    *, config: dict[str, object], relation: str, previous: str
) -> tuple[tuple[tuple[object, ...], ...], tuple[tuple[object, ...], ...]]:
    """`relation`'s columns and types that `previous` also has, and those it adds, in order."""

    columns: dict[object, object] = {
        column[0]: column[1] for column in described(config=config, relation=relation)
    }
    kept: tuple[str, ...] = column_names(config=config, relation=previous)
    added: dict[object, object] = dict(columns)
    for name in kept:
        del added[name]
    return (
        tuple((name, columns[name]) for name in kept),
        tuple(added.items()),
    )


def create_previous_views(
    *, config: dict[str, object], database: str, names: tuple[str, ...], prefix: str
) -> None:
    """Create the previous version's bodies of `names`, each under `prefix` + its name."""

    bodies: dict[str, str] = previous_view_bodies(database=database)
    execute_clickhouse_sql(
        config=config,
        statements=tuple(
            f"CREATE OR REPLACE VIEW `{database}`.`{prefix}{name}` AS\n{bodies[name]}"
            for name in names
        ),
    )


def snapshot_views(*, config: dict[str, object], database: str, names: tuple[str, ...]) -> None:
    """Copy each view's current rows into a `snapshot_` memory table."""

    execute_clickhouse_sql(
        config=config,
        statements=tuple(
            f"CREATE TABLE `{database}`.`snapshot_{name}` ENGINE = Memory AS "
            f"SELECT * FROM `{database}`.`{name}`"
            for name in names
        ),
    )


def table_reads(*, config: dict[str, object], sql: str) -> tuple[str, ...]:
    """The tables `sql` reads, once per read, from its plan."""

    plan: tuple[tuple[object, ...], ...] = fetch_clickhouse_rows(
        config=config, sql=f"EXPLAIN {sql}"
    )
    return tuple(sorted(_PLAN_READ.findall("\n".join(str(row[0]) for row in plan))))


def primary_key_reads(*, config: dict[str, object], sql: str) -> int:
    """How many of `sql`'s reads use the primary key by `invocation_id`."""

    plan: tuple[tuple[object, ...], ...] = fetch_clickhouse_rows(
        config=config, sql=f"EXPLAIN indexes = 1 {sql}"
    )
    return len(_PRIMARY_KEY_INDEX.findall("\n".join(str(row[0]) for row in plan)))


@contextmanager
def readonly_user(
    *, config: dict[str, object], database: str, settings: str = "readonly = 1"
) -> Iterator[dict[str, object]]:
    """A user with `settings`, by default the standard `readonly = 1`, and `SELECT` on it."""

    name: str = build_unique_schema_name(prefix="recall_reader")
    password: str = uuid.uuid4().hex
    execute_clickhouse_sql(
        config=config,
        statements=(
            f"CREATE USER {name} IDENTIFIED BY '{password}' SETTINGS {settings}",
            f"GRANT SELECT ON {database}.* TO {name}",
        ),
    )
    try:
        yield {**config, "user": name, "password": password}
    finally:
        execute_clickhouse_sql(config=config, statements=(f"DROP USER IF EXISTS {name}",))


_FAILING_MODEL: str = (
    "MODEL (\n  description 'An order whose amount does not parse',\n  materialized table,\n);\n\n"
    "SELECT CAST('not a number' AS INTEGER) AS order_amount\n"
)


def representative_store(project_dir: Path) -> subprocess.Popen[bytes]:
    """A completed, a failed, an abandoned and a heartbeat-only invocation, and a running one."""

    assert run(project_dir, "build").returncode == 0
    failing: Path = project_dir / "models" / "unparsed_orders.sql"
    _ = failing.write_text(_FAILING_MODEL, encoding="utf-8")
    assert run(project_dir, "build").returncode != 0
    failing.unlink()
    _ = killed_build(project_dir, heartbeat_seconds="0.5")
    _ = killed_during_discovery(project_dir, heartbeat_seconds="0.5")
    running: subprocess.Popen[bytes] = start(project_dir, "build", ORDERS_TASK_SLEEP_SECONDS="600")
    _ = wait_for(
        "SELECT invocation_id FROM invocations WHERE status = 'running' "
        "AND invocation_id IN (SELECT invocation_id FROM recall_output)",
        project_dir,
        timeout=120,
    )
    return running


def drop_database(*, config: dict[str, object], database: str) -> None:
    """Drop `database`, so a writer finds it missing."""

    execute_clickhouse_sql(config=config, statements=(f"DROP DATABASE {database} SYNC",))


def stale_views(*, config: dict[str, object], database: str) -> tuple[tuple[object, ...], ...]:
    """How many views in `database` still depend on `join_use_nulls`, as the previous version's."""

    return fetch_clickhouse_rows(
        config=config,
        sql="SELECT countIf(position(create_table_query, 'join_use_nulls') > 0) "
        f"FROM system.tables WHERE database = '{database}' AND engine = 'View'",
    )


JANITOR_SETTINGS: str = (
    "\n[janitor]\nenabled = true\nretention_days = 0\ndelete_tracked_only = false\n"
)
STALE_RELATIONS: tuple[str, ...] = (
    "CREATE TABLE {schema}.old_orders ENGINE = MergeTree ORDER BY id "
    "AS SELECT toInt64(number) AS id FROM numbers(3)",
    "CREATE TABLE {schema}.old_customers ENGINE = MergeTree ORDER BY id AS SELECT toInt64(1) AS id",
    "CREATE VIEW {schema}.old_orders_v2 AS SELECT id FROM {schema}.old_orders",
    "CREATE VIEW {schema}.old_orders_v AS SELECT id FROM {schema}.old_orders_v2",
)


def prepare_janitor_usage_project(
    *, tmp_path: Path, config: dict[str, object], schema: str, connection_settings: str = ""
) -> Path:
    """A built project in `schema` beside a stale table, an unread table and a view chain on it."""

    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="clickhouse_janitor_usage",
        repo_files={
            "sqlbuild_project.toml": build_clickhouse_project_toml(
                project_name="clickhouse_janitor_usage",
                schema_name=schema,
                config=config,
                extra=JANITOR_SETTINGS,
            ).replace("\n[targets.dev]", connection_settings + "\n[targets.dev]"),
            "models/orders.sql": "MODEL (description 'Orders.');\n\nSELECT toInt64(1) AS id\n",
        },
    )
    built: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert built.returncode == 0, built.stdout + built.stderr
    execute_clickhouse_sql(
        config=config,
        statements=tuple(statement.format(schema=schema) for statement in STALE_RELATIONS),
    )
    return project_dir


def read_as(*, config: dict[str, object], sql: str) -> None:
    """Run one query as the user in `config`, as any client would, outside SQLBuild."""

    import clickhouse_connect

    client: Any = clickhouse_connect.get_client(
        host=str(config["host"]),
        port=int(str(config["port"])),
        username=str(config["user"]),
        password=str(config["password"]),
    )
    try:
        client.query(sql)
    finally:
        client.close()


def stale_relation_names(*, config: dict[str, object], schema: str) -> tuple[str, ...]:
    """The `old_*` relations still in `schema`."""

    return tuple(
        str(row[0])
        for row in fetch_clickhouse_rows(
            config=config,
            sql=f"SELECT name FROM system.tables WHERE database = '{schema}' "
            "AND startsWith(name, 'old_') ORDER BY name",
        )
    )
