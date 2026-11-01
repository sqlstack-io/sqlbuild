"""Local helpers for audit command E2E tests."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from textwrap import dedent
from typing import Any, cast

import duckdb

from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.audit_results.constants import COLUMN_AUDIT_META_JSON
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

ORDER_COLUMNS: tuple[str, ...] = tuple(f"order_attribute_{index:02d}" for index in range(12))


def delivery_project_toml(*, lifecycle_config: str) -> str:
    """Return a DuckDB project config with optional lifecycle sink settings."""

    return (
        dedent(
            """
            name = "audit_event_delivery"
            adapter = "duckdb"

            [connection]
            database = "audit_event_delivery.duckdb"
            """
        ).lstrip()
        + lifecycle_config
    )


def delivery_model_files() -> dict[str, str]:
    """Return a customers -> orders chain carrying one plus twelve column audits."""

    columns: str = "\n".join(f"    {name} (audits [not_null])," for name in ORDER_COLUMNS)
    selected: str = ",\n  ".join(f"1 AS {name}" for name in ORDER_COLUMNS)
    return {
        "models/customers.sql": dedent(
            """
            MODEL (description "Test model customers.",
              materialized table,
              columns (
                customer_id (audits [not_null]),
              ),
            );

            SELECT 1 AS customer_id
            """
        ).lstrip(),
        "models/orders.sql": (
            "MODEL (description 'Test model orders.',\n  materialized table,\n  columns (\n"
            f"{columns}\n"
            "  ),\n);\n\n"
            f"SELECT\n  customer_id,\n  {selected}\n"
            'FROM __ref("customers")\n'
        ),
    }


def recording_sink_source(*, event_kinds: str, delay_seconds: float) -> str:
    """Return a lifecycle sink that appends each event to ORDERS_EVENT_PATH after a delay."""

    return dedent(
        f"""
        import os
        import time
        from pathlib import Path

        from sqlbuild.sinks import (
            LifecycleEvent,
            LifecycleEventKind,
            lifecycle_event_sink,
            lifecycle_event_to_json,
        )


        @lifecycle_event_sink(name="slow_orders_events", event_kinds={event_kinds})
        def record_event(event: LifecycleEvent) -> None:
            time.sleep({delay_seconds})
            path = Path(os.environ["ORDERS_EVENT_PATH"])
            with path.open("a", encoding="utf-8") as stream:
                stream.write(lifecycle_event_to_json(event) + "\\n")
        """
    ).lstrip()


def slow_once_sink_source(*, slow_call: int) -> str:
    """Return a lifecycle sink whose call number ``slow_call`` sleeps past the 1s call timeout."""

    return dedent(
        f"""
        import os
        import time
        from pathlib import Path

        from sqlbuild.sinks import LifecycleEvent, lifecycle_event_sink, lifecycle_event_to_json

        CALLS: list[int] = []


        @lifecycle_event_sink(name="slow_once_events")
        def record_event(event: LifecycleEvent) -> None:
            CALLS.append(1)
            if len(CALLS) == {slow_call}:
                time.sleep(1.5)
            path = Path(os.environ["ORDERS_EVENT_PATH"])
            with path.open("a", encoding="utf-8") as stream:
                stream.write(lifecycle_event_to_json(event) + "\\n")
        """
    ).lstrip()


def read_sequenced_events(path: Path) -> list[dict[str, object]]:
    """Return recorded events ordered by canonical invocation sequence."""

    events: list[dict[str, object]] = [
        json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()
    ]
    return sorted(events, key=lambda event: int(str(event["invocation_sequence"])))


def events_of_type(events: list[dict[str, object]], event_type: str) -> list[dict[str, object]]:
    """Return events with one lifecycle type in their existing order."""

    return list(filter(lambda event: event["event_type"] == event_type, events))


AUDIT_META_ORDERS_HEADER: str = (
    'description "Valid order rate",\n'
    '      meta (region "emea", tiers ["gold", "silver"], weight 1.5, flags (late false)),'
)


def audit_meta_project_files(
    *, measurement_options: str = AUDIT_META_ORDERS_HEADER
) -> dict[str, str]:
    """Return a DuckDB project whose SQL, column and factory audits each declare meta."""

    return {
        "sqlbuild_project.toml": dedent(
            """
            name = "audit_meta"
            adapter = "duckdb"

            [connection]
            database = "audit_meta.duckdb"
            """
        ).lstrip(),
        "audits/generic/valid_order_rate.sql": dedent(
            """
            AUDIT (
              evaluation measurement,
              value valid_rate,
              sample_count total_rows,
              sample_unit rows
            );

            MEASURE (
              SELECT
                COUNT(*) AS total_rows,
                100.0 * AVG(CASE WHEN @condition THEN 1 ELSE 0 END) AS valid_rate
              FROM @relation
            );
            """
        ).lstrip(),
        "audits/generic/expression_is_true.sql": (
            "AUDIT ();\n\nSELECT * FROM @relation WHERE NOT (@expression)\n"
        ),
        "python/factories/order_quality.py": dedent(
            """
            from sqlbuild.audits import AuditCase, AuditSeverity, audit_factory


            @audit_factory
            def order_quality():
                return [
                    AuditCase(
                        name="positive_amount",
                        definition="expression_is_true",
                        arguments={"expression": "amount > 0"},
                        severity=AuditSeverity.ERROR,
                        meta={"segment": "core", "reference": "orders-quality", "rank": 1},
                    )
                ]
            """
        ).lstrip(),
        "models/orders.sql": (
            "MODEL (\n"
            '  description "Test model orders.",\n'
            "  materialized table,\n"
            "  audit_factories [order_quality],\n"
            "  columns (\n"
            '    order_id (audits [not_null (meta (owner "fulfillment"))]),\n'
            "  ),\n"
            "  audits [\n"
            "    valid_order_rate (\n"
            "      condition \"region = 'emea'\",\n"
            "      thresholds (error (below 50)),\n"
            f"      {measurement_options}\n"
            "    ),\n"
            '    expression_is_true (name "known_region", expression "region IS NOT NULL"),\n'
            "  ],\n"
            ");\n\n"
            "WITH orders(order_id, region, amount) AS (\n"
            "  VALUES (1, 'emea', 10), (2, 'emea', 20), (3, 'apac', 30)\n"
            ")\n"
            "SELECT * FROM orders\n"
        ),
    }


AUDIT_META_DATABASE: str = "audit_meta.duckdb"


def prepare_audit_meta_project(*, tmp_path: Path, measurement_options: str) -> Path:
    """Write the audit meta project with a sink that records audit lifecycle events."""

    return prepare_inline_project(
        tmp_path=tmp_path,
        project_name="audit_meta",
        repo_files={
            **audit_meta_project_files(measurement_options=measurement_options),
            "sinks/orders_events.py": recording_sink_source(
                event_kinds="{LifecycleEventKind.AUDIT}", delay_seconds=0
            ),
        },
    )


def create_legacy_audit_results_table(project_dir: Path) -> None:
    """Create the audit results table as it was before the meta column existed."""

    create_sql: str = DuckDbAdapter().render_create_audit_result_table_sql(
        database=None, schema="main"
    )
    with duckdb.connect(str(project_dir / AUDIT_META_DATABASE)) as connection:
        connection.execute(create_sql.replace(f", {COLUMN_AUDIT_META_JSON} VARCHAR", ""))


def leave_audit_results_table_absent(project_dir: Path) -> None:
    """Leave the audit results table for the build to create."""

    del project_dir


def stored_audit_meta(project_dir: Path) -> dict[str, object]:
    """Map each stored audit name to its decoded meta, `None` when absent."""

    rows: list[tuple[Any, ...]] = query_duckdb(
        db_path=project_dir / AUDIT_META_DATABASE,
        sql=(
            "SELECT audit_name, COALESCE(audit_meta_json, 'null') "
            "FROM main._sqlbuild_audit_results ORDER BY occurred_at"
        ),
    )
    return {str(name): json.loads(str(meta)) for name, meta in rows}


def stored_audit_identities(project_dir: Path) -> dict[str, tuple[object, ...]]:
    """Map each audit's latest stored result to its identity, outcome and measurement."""

    rows: list[tuple[Any, ...]] = query_duckdb(
        db_path=project_dir / AUDIT_META_DATABASE,
        sql=(
            "SELECT audit_name, binding_key, definition_fingerprint, execution_fingerprint, "
            "outcome, measured_value, sample_count "
            "FROM main._sqlbuild_audit_results ORDER BY occurred_at"
        ),
    )
    return {str(row[0]): tuple(row[1:]) for row in rows}


def audit_completed_meta(event_path: Path) -> dict[str, object]:
    """Map each audit_completed event's audit name to its `audit_meta` payload field."""

    events: list[dict[str, object]] = events_of_type(
        read_sequenced_events(event_path), "audit_completed"
    )
    payloads: list[dict[str, object]] = [
        cast(dict[str, object], event["payload"]) for event in events
    ]
    return {str(payload["audit_name"]): payload.get("audit_meta") for payload in payloads}


def json_check_meta(stdout: str) -> dict[str, object]:
    """Map each JSON-output check name to its `audit_meta` field."""

    return {str(check["name"]): check.get("audit_meta") for check in json.loads(stdout)["checks"]}


def json_asset_statuses(stdout: str) -> dict[str, str]:
    """Map each JSON-output asset name to its status."""

    return {str(asset["name"]): str(asset["status"]) for asset in json.loads(stdout)["assets"]}


def plan_reasons(project_dir: Path) -> dict[str, str]:
    """Map each planned model to its plan reason from `sqb plan --json`."""

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan", "--json"), project_dir=project_dir
    )
    assert result.returncode == 0, result.stdout + result.stderr
    return {
        str(model["name"]): str(model["reason"]) for model in json.loads(result.stdout)["models"]
    }


GENERIC_AUDIT_PATH: str = "audits/generic/unique_within_group.sql"


def generic_audit_project_files(*, arguments: str) -> dict[str, str]:
    """Return a project whose orders model attaches unique_within_group with ``arguments``."""

    return {
        "sqlbuild_project.toml": (
            'name = "audit_arguments"\nadapter = "duckdb"\n\n'
            '[connection]\ndatabase = "audit_arguments.duckdb"\n'
        ),
        GENERIC_AUDIT_PATH: (
            "AUDIT ();\n\n"
            "SELECT @group_columns, COUNT(DISTINCT @column) AS distinct_count\n"
            "FROM @relation\nWHERE @column IS NOT NULL\nGROUP BY @group_columns\n"
            "HAVING COUNT(DISTINCT @column) > 1\n"
        ),
        "models/orders.sql": (
            "MODEL (description 'Test model orders.',\n  materialized table,\n  columns (\n"
            f"    status (audits [unique_within_group ({arguments})]),\n  ),\n);\n\n"
            "SELECT 1 AS order_id, 'open' AS status, 'web' AS channel\n"
        ),
    }
