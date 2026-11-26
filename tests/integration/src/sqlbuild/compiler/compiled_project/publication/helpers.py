"""A standalone native API and real SQLite store, with test-only credential provisioning."""

from __future__ import annotations

import base64
import fcntl
import gzip
import hashlib
import json
import os
import pty
import random
import re
import secrets
import select
import signal
import socket
import sqlite3
import subprocess
import sys
import termios
import threading
import time
from collections.abc import Callable, Iterator
from contextlib import (
    AbstractContextManager,
    ExitStack,
    closing,
    contextmanager,
    nullcontext,
    suppress,
)
from dataclasses import replace
from datetime import UTC, datetime, timedelta
from functools import partial
from io import StringIO
from pathlib import Path
from typing import Any, Literal, LiteralString, cast
from urllib.error import HTTPError
from urllib.parse import quote
from urllib.request import Request, urlopen
from uuid import uuid4

import duckdb
import psycopg
import pytest
from testcontainers.postgres import PostgresContainer

from scripts.native_tests.constants import POSTGRES_IMAGE
from sqlbuild._native import (
    ColumnMigrationDecision,
    ColumnMigrationEvent,
    Fingerprint,
    MigrationDecision,
    MigrationDiscovery,
    MigrationEvent,
    MigrationRelation,
    OldNameViewEvent,
    OldNameViewEventType,
    StateSqlValueType,
    build_consumption_create_table_sql,
    build_observations_create_table_sql,
    render_state_sql_literal,
    write_fingerprint,
    write_migration_state_event,
)
from sqlbuild.adapter.discovery.main.resolve_adapter import resolve_adapter
from sqlbuild.executor.audit_results.main._write import write_audit_result_records
from sqlbuild.executor.audit_results.models import AuditResultRecord
from sqlbuild.executor.janitor_events.classes.janitor_event_writer import JanitorEventWriter
from sqlbuild.executor.janitor_events.main.create_table_sql import (
    build_janitor_events_create_table_sql,
)
from sqlbuild.executor.janitor_events.models import JanitorEventRecord
from sqlbuild.executor.janitor_events.types import JanitorEventType
from sqlbuild.executor.node_results.classes.direct_store import DirectNodeResultStore
from sqlbuild.executor.node_results.models import NodeResultRecord
from sqlbuild.microbatches.classes.direct_store import DirectMicrobatchEventStore
from sqlbuild.microbatches.classes.event_codec import MicrobatchEventCodec
from sqlbuild.microbatches.constants import MICROBATCH_COLUMNS
from sqlbuild.microbatches.models import MicrobatchEvent, MicrobatchWriteResult
from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import (
    PUBLISHABLE_FILES,
    duckshop_orchestration,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    ClaimedWorker,
    ScheduledWorker,
    SnapshotApi,
    WorkerDeployment,
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.catalog.helpers import (
    request_plan,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.docker.helpers import (
    docker_deployment,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.person.helpers import (
    person_login,
    person_request,
    provision_person,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files
from tests.integration.src.sqlbuild.executor.audit_results.main.helpers import build_record
from tests.unit.src.sqlbuild.python_nodes.helpers import node_module_files


class RejectedPublicationOutput(StringIO):
    """A caller loses only the result stream, after the real API accepts a publication."""

    def write(self, text: str) -> int:
        raise OSError("synthetic publication result delivery failure")


# A provider whose settings only the project's .env sets; the worker must see them.
REGION_PROVIDER_FILES: dict[str, str] = {
    "python/providers/region.py": (
        "from pydantic import Field, SecretStr\n"
        "from sqlbuild.providers import Provider\n\n\n"
        "class Region(Provider):\n"
        '    """Region settings from the environment."""\n'
        "    token: SecretStr = Field(validation_alias='WORKER_ENV_TOKEN')\n"
        "    code: int = Field(validation_alias='WORKER_ENV_REGION_CODE')\n"
    ),
    "sql/sources/regions.yml": (
        "sources:\n"
        "  - name: raw_regions\n"
        "    description: The region code the provider reads.\n"
        "    managed: true\n"
        "    columns:\n"
        "      - name: code\n"
        "        type: BIGINT\n"
    ),
    "python/nodes/loaders/regions/loader.py": (
        "from python.providers.region import Region\n"
        "from sqlbuild.loaders import loader\n\n\n"
        "@loader(write_strategy='replace')\n"
        "def raw_regions(ctx, region: Region):\n"
        '    """Load the provider\'s region code."""\n'
        "    return [{'code': region.code}]\n"
    ),
}


@contextmanager
def location_process(
    arguments: list[str], log: Path, publications: Path, *, env: dict[str, str] | None = None
) -> Iterator[subprocess.Popen[bytes]]:
    """Own and reap the supervisor even when an assertion interrupts a reload."""
    with log.open("ab") as stderr, publications.open("ab") as stdout:
        process: subprocess.Popen[bytes] = subprocess.Popen(
            arguments, stderr=stderr, stdout=stdout, env=env
        )
        try:
            yield process
        finally:
            process.terminate()
            try:
                _ = process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                _ = process.wait(timeout=5)


def stop_location(
    process: subprocess.Popen[bytes],
    interpreter_pid: int,
    log: Path,
    *,
    expected_exit_code: int = 128 + signal.SIGTERM,
) -> None:
    """Stop `sqb dev` (143) or `sqb location serve` (0) with SIGTERM; its interpreter is reaped."""
    process.send_signal(signal.SIGTERM)
    assert process.wait(timeout=15) == expected_exit_code, log.read_text()
    with pytest.raises(ProcessLookupError):
        os.kill(interpreter_pid, 0)


def open_legacy_warehouse(project: Path, adapter_name: str, connection_key: str) -> tuple[Any, Any]:
    """The caller owns the returned real connection; declarations must never run during reads."""
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": (
                f'name = "orders"\nadapter = "{adapter_name}"\n'
                f'[connections.local]\n{connection_key} = "warehouse.db"\n'
                '[targets.development]\nschema = "main"\n'
            ),
            "orchestration/calendars/unexpected_calendar.py": (
                "from sqlbuild.orchestration import calendar\n"
                'raise AssertionError("unexpected declaration import")\n'
                "@calendar(name='unexpected_calendar', description='Never imported.', "
                "horizon='1d')\n"
                "def unexpected(window):\n    return []\n"
            ),
        },
    )
    adapter: Any = resolve_adapter(adapter_name=adapter_name)
    connection: Any = adapter.connect({connection_key: str(project / "warehouse.db")})
    legacy_sql: str = Path(
        "benchmarks/duckshop/edge_cases/orchestration/legacy_warehouse_state.sql"
    ).read_text()
    for statement in legacy_sql.rstrip().removesuffix(";").split(";"):
        _ = adapter.execute(connection=connection, sql=statement)
    return adapter, connection


def append_legacy_history(
    adapter: Any, connection: Any, rows: list[dict[str, str]], statements: list[str]
) -> None:
    """Append synthetic arrivals with independent observation times to real history tables."""
    for row in rows:
        for statement in statements:
            _ = adapter.execute(connection=connection, sql=statement.format(**row))


def append_audit_history(adapter: Any, connection: Any, fixture: dict[str, Any]) -> None:
    """Use the public warehouse writer, including duplicate IDs and an external CLI run."""
    record: AuditResultRecord = replace(
        build_record(),
        run_id="",
        evidence_json=fixture["record"]["evidence_json"],
        evidence_count=1,
        evidence_truncated=True,
        audit_meta_json=fixture["record"]["audit_meta_json"],
    )
    records: list[AuditResultRecord] = [
        replace(record, result_id=f"result-{index:03}") for index in range(fixture["source_rows"])
    ]
    records.extend([records[0]] * fixture["duplicate_rows"])
    records.append(replace(records[0], occurred_at=datetime.fromisoformat(fixture["later_time"])))
    write_audit_result_records(
        connection=connection,
        execute=adapter.execute,
        database=None,
        schema="main",
        records=records,
        render_qualified_name=adapter.render_qualified_name,
        render_framework_type=adapter.render_framework_type,
        render_create_table_sql=adapter.render_create_audit_result_table_sql,
        render_create_index_sqls=adapter.render_create_audit_result_index_sqls,
    )


def append_janitor_history(adapter: Any, connection: Any, fixture: dict[str, Any]) -> None:
    """Real chunked archive/delete writes, including repeated write attempts."""
    data: dict[str, Any] = fixture["record"].copy()
    del data["state_database"]
    del data["state_schema"]
    for key in ("occurred_at", "archived_at"):
        data[key] = datetime.fromisoformat(data[key])
    data["event_type"] = JanitorEventType(data["event_type"])
    record: JanitorEventRecord = JanitorEventRecord(**data)
    records: list[JanitorEventRecord] = [
        replace(record, event_id=f"event-{index:03}") for index in range(fixture["source_rows"])
    ]
    records[-1] = replace(
        records[-1],
        event_type=JanitorEventType.DELETE,
        occurred_at=datetime.fromisoformat(fixture["later_time"]),
    )
    _ = adapter.execute(
        connection=connection,
        sql=build_janitor_events_create_table_sql(
            database=None,
            schema="main",
            render_qualified_name=adapter.render_qualified_name,
            render_framework_type=adapter.render_framework_type,
        ),
    )
    writer: JanitorEventWriter = JanitorEventWriter(
        execute=adapter.execute,
        render_qualified_name=adapter.render_qualified_name,
        run_id=record.run_id,
    )
    writer.write(connection=connection, records=tuple(records))
    writer.write(connection=connection, records=(records[0],) * fixture["duplicate_rows"])


def append_microbatch_history(adapter: Any, connection: Any, fixture: dict[str, Any]) -> None:
    """Real append-only coordination writer; reading never enters the executor's state path."""
    rows: list[dict[str, Any]] = [
        fixture["record"]
        | {"event_id": f"{fixture['id_prefix']}-{index:03}"}
        | fixture["record_overrides"].get(f"{fixture['id_prefix']}-{index:03}", {})
        for index in range(fixture["source_rows"])
    ]
    records: list[MicrobatchEvent] = []
    for row in rows:
        values: tuple[Any, ...] = tuple(row[column] for column in MICROBATCH_COLUMNS)
        records.append(MicrobatchEventCodec.from_row(values))
    writer: DirectMicrobatchEventStore = DirectMicrobatchEventStore(
        adapter=adapter, connection=connection
    )
    first: MicrobatchWriteResult = writer.write_many(tuple(records))
    assert first.inserted == fixture["source_rows"]
    repeated: MicrobatchWriteResult = writer.write_many((records[0],) * fixture["duplicate_rows"])
    assert repeated.inserted == 0


def append_migration_history(adapter: Any, connection: Any, fixture: dict[str, Any]) -> None:
    """Real model-move facts, including idempotent duplicate attempts."""
    data: dict[str, Any] = migration_history_data(fixture, ("origin", "destination"))
    data["discovery"] = MigrationDiscovery(data["discovery"])
    data["decision"] = MigrationDecision(data["decision"])
    record: MigrationEvent = MigrationEvent(**data)
    write_migration_history(
        adapter,
        connection,
        fixture,
        record,
        "model",
        adapter.render_create_migration_state_table_sql(database=None, schema="main"),
        {},
    )


def append_column_migration_history(adapter: Any, connection: Any, fixture: dict[str, Any]) -> None:
    """Real column-rename facts, including idempotent duplicate attempts."""
    data: dict[str, Any] = migration_history_data(fixture, ("relation",))
    data["discovery"] = MigrationDiscovery(data["discovery"])
    data["decision"] = ColumnMigrationDecision(data["decision"])
    record: ColumnMigrationEvent = ColumnMigrationEvent(**data)
    write_migration_history(
        adapter,
        connection,
        fixture,
        record,
        "column",
        adapter.render_create_column_migration_state_table_sql(database=None, schema="main"),
        {},
    )


def append_node_result_history(adapter: Any, connection: Any, fixture: dict[str, Any]) -> None:
    """Real node-result writer; retried writes repeat rows, which carry no event identity."""
    store: DirectNodeResultStore = DirectNodeResultStore(
        adapter=adapter, connection=connection, database=None, schema="main"
    )
    rows: list[dict[str, Any]] = identified_rows(fixture)
    for row in rows + [rows[0]] * fixture["duplicate_rows"]:
        store.write(
            NodeResultRecord(
                node_type=row["node_type"],
                node_name=row["node_name"],
                target_database=row["target_database"],
                target_schema=row["target_schema"],
                target_name=row["target_name"],
                run_id=row["run_id"],
                status=row["status"],
                payload=json.loads(base64.b64decode(row["payload_json_b64"])),
                metadata=json.loads(base64.b64decode(row["metadata_json_b64"])),
                error_message=row["error_message"],
                materialized=row["materialized"] == "true",
                ts=datetime.fromisoformat(row["ts"]),
            )
        )


def append_consumption_history(adapter: Any, connection: Any, fixture: dict[str, Any]) -> None:
    """Consumption rows in the real table layout; retried appends repeat identical rows."""
    _ = adapter.execute(
        connection=connection,
        sql=build_consumption_create_table_sql(
            database=None,
            schema="main",
            render_qualified_name=adapter.render_qualified_name,
            render_framework_type=adapter.render_framework_type,
        ),
    )
    columns: list[str] = list(fixture["record"])[2:]
    types: dict[str, StateSqlValueType] = dict.fromkeys(columns, StateSqlValueType.STRING) | {
        "input_count": StateSqlValueType.INTEGER,
        "recorded_at": StateSqlValueType.TEXT_TIMESTAMP,
    }
    rows: list[dict[str, Any]] = [
        row | {"recorded_at": datetime.fromisoformat(row["recorded_at"])}
        for row in identified_rows(fixture)
    ]
    for row in rows + [rows[0]] * fixture["duplicate_rows"]:
        values: list[str] = [
            render_state_sql_literal(
                value=row[column], declared_type=types[column], timestamp_type="TIMESTAMP"
            )
            for column in columns
        ]
        _ = adapter.execute(
            connection=connection,
            sql=f"INSERT INTO main._sqlbuild_consumption ({', '.join(columns)}) "
            f"VALUES ({', '.join(values)})",
        )


def append_observation_history(adapter: Any, connection: Any, fixture: dict[str, Any]) -> None:
    """Observation rows in the real table layout; retried appends repeat identical rows."""
    _ = adapter.execute(
        connection=connection,
        sql=build_observations_create_table_sql(
            database=None,
            schema="main",
            render_qualified_name=adapter.render_qualified_name,
            render_framework_type=adapter.render_framework_type,
        ),
    )
    columns: list[str] = list(fixture["record"])[2:]
    types: dict[str, StateSqlValueType] = dict.fromkeys(columns, StateSqlValueType.STRING) | {
        "observed_at": StateSqlValueType.TEXT_TIMESTAMP
    }
    rows: list[dict[str, Any]] = [
        row | {"observed_at": datetime.fromisoformat(row["observed_at"])}
        for row in identified_rows(fixture)
    ]
    for row in rows + [rows[0]] * fixture["duplicate_rows"]:
        values: list[str] = [
            render_state_sql_literal(
                value=row[column], declared_type=types[column], timestamp_type="TIMESTAMP"
            )
            for column in columns
        ]
        _ = adapter.execute(
            connection=connection,
            sql=f"INSERT INTO main._sqlbuild_observations ({', '.join(columns)}) "
            f"VALUES ({', '.join(values)})",
        )


def identified_rows(fixture: dict[str, Any]) -> list[dict[str, Any]]:
    """The fixture record once per source identity, with its per-identity overrides."""
    return [
        fixture["record"]
        | {fixture["source_key"]: f"{fixture['id_prefix']}-{index:03}"}
        | fixture["record_overrides"].get(f"{fixture['id_prefix']}-{index:03}", {})
        for index in range(fixture["source_rows"])
    ]


def append_old_name_history(adapter: Any, connection: Any, fixture: dict[str, Any]) -> None:
    """Real compatibility-view facts, including nullable and populated expiry."""
    data: dict[str, Any] = migration_history_data(fixture, ("old", "new"))
    data["event_type"] = OldNameViewEventType(data["event_type"])
    data["column_aliases"] = ()
    record: OldNameViewEvent = OldNameViewEvent(**data)
    write_migration_history(
        adapter,
        connection,
        fixture,
        record,
        "old_name",
        adapter.render_create_old_name_view_state_table_sql(database=None, schema="main"),
        {
            "column_aliases": tuple(json.loads(fixture["last_record"]["column_aliases"]).items()),
            "expires_at": datetime.fromisoformat(fixture["last_record"]["expires_at"]),
            "view_sql": fixture["last_record"]["view_sql"],
        },
    )


def migration_history_data(fixture: dict[str, Any], relations: tuple[str, ...]) -> dict[str, Any]:
    """Reconstruct native relation values from the warehouse-shaped fixture."""
    data: dict[str, Any] = fixture["record"].copy()
    del data["state_database"]
    del data["state_schema"]
    data["created_at"] = datetime.fromisoformat(data["created_at"])
    for relation in relations:
        data[relation] = MigrationRelation(
            database=data.pop(f"{relation}_database"),
            schema=data.pop(f"{relation}_schema"),
            name=data.pop(f"{relation}_name"),
        )
    return data


def write_migration_history(
    adapter: Any,
    connection: Any,
    fixture: dict[str, Any],
    record: MigrationEvent | ColumnMigrationEvent | OldNameViewEvent,
    kind: Literal["model", "column", "old_name"],
    create_sql: str,
    last_values: dict[str, Any],
) -> None:
    """Exercise the shared immutable writer with paginated, retried source facts."""
    records: list[Any] = [
        replace(record, event_id=f"{fixture['id_prefix']}-{index:03}")
        for index in range(fixture["source_rows"])
    ]
    records[-1] = replace(
        records[-1], created_at=datetime.fromisoformat(fixture["later_time"]), **last_values
    )
    records.extend([records[0]] * fixture["duplicate_rows"])
    for record in records:
        write_migration_state_event(
            kind=kind,
            connection=connection,
            execute=adapter.execute,
            event=record,
            render_qualified_name=adapter.render_qualified_name,
            create_table_sql=create_sql,
        )


def warehouse_history_page(
    project: Path,
    table: str,
    after: list[str | None] | None = None,
    observed_since: str | None = None,
    **options: int | None,
) -> Any:
    """Exercise the real isolated reader without a server, state database or declaration import."""
    result: subprocess.CompletedProcess[str] = warehouse_history_read(
        project, table, after, observed_since, **options
    )
    assert result.returncode == 0, result.stderr
    return json.loads(result.stdout)


def warehouse_history_read(
    project: Path,
    table: str,
    after: list[str | None] | None = None,
    observed_since: str | None = None,
    **options: int | None,
) -> subprocess.CompletedProcess[str]:
    """One isolated reader exchange, with the reader's own report of a failed read."""
    read: dict[str, Any] = {
        "location": {"database": None, "schema": "main"},
        "after": after,
        "observed_since": observed_since,
    } | options
    return subprocess.run(
        [
            str(Path(sys.executable).with_name("sqb")),
            "--project-dir",
            str(project),
            "location",
            "runtime",
            "--server",
            "http://127.0.0.1:1",
            "--location-id",
            "00000000-0000-0000-0000-000000000001",
            "--target",
            "development",
        ],
        input=json.dumps({table: read}) + "\n",
        text=True,
        capture_output=True,
        timeout=20,
        env=os.environ | {"SQLBUILD_CONTROL_DATABASE_URL": "unsupported://must-not-open"},
        check=False,
    )


def warehouse_history_pages(
    project: Path, table: str, observed_since: str | None = None, **options: int | None
) -> list[dict[str, Any]]:
    """Follow bounded source pages, including empty filtered pages with a continuation."""
    pages: list[dict[str, Any]] = []
    after: list[str | None] | None = None
    more: bool = True
    while more:
        page: dict[str, Any] = warehouse_history_page(
            project, table, after, observed_since, **options
        )
        assert "failed" not in page, page
        pages.append(page)
        after = page["next_after"]
        more = after is not None
        assert len(pages) <= 4, "history cursor must advance past duplicate physical rows"
    return pages


def sync_warehouse_history(api: SnapshotApi, batch: dict[str, Any]) -> dict[str, Any]:
    """Deliver one metadata page through the public location transport."""
    request: Request = Request(
        f"{api.url}/api/locations/{api.location_id}/targets/development/metadata",
        data=json.dumps(batch).encode(),
        headers={"Authorization": f"Bearer {api.bearer}", "Content-Type": "application/json"},
    )
    with urlopen(request, timeout=5) as response:
        result: dict[str, Any] = json.load(response)
    return result


def wait_for_generation(
    api: SnapshotApi, target: str, generation: int, process: subprocess.Popen[bytes], log: Path
) -> dict[str, Any]:
    deadline: float = time.monotonic() + 15
    current: dict[str, Any] = {"record": {"generation": 0}}
    while current["record"]["generation"] < generation:
        assert process.poll() is None, log.read_text()
        assert time.monotonic() < deadline, (
            f"Generation {generation} was not published: {log.read_text()}"
        )
        try:
            current = current_snapshot(api, target)
        except HTTPError as error:
            assert error.code == 404
        time.sleep(0.02)
    assert current["record"]["generation"] == generation, log.read_text()
    return current


def wait_for_log(
    process: subprocess.Popen[bytes], log: Path, text: str, *, timeout: float = 15
) -> None:
    deadline: float = time.monotonic() + timeout
    while text not in log.read_text():
        assert process.poll() is None, log.read_text()
        assert time.monotonic() < deadline, log.read_text()
        time.sleep(0.02)


def wait_for_log_count(
    process: subprocess.Popen[bytes], log: Path, text: str, count: int, *, timeout: float = 45
) -> str:
    """Wait until `text` has been logged `count` times; returns the log."""
    deadline: float = time.monotonic() + timeout
    while (logged := log.read_text()).count(text) < count:
        assert process.poll() is None, logged
        assert time.monotonic() < deadline, logged
        time.sleep(0.05)
    return logged


def wait_for_development_ready(
    process: subprocess.Popen[bytes], log: Path, *, timeout: float = 60
) -> tuple[str, str]:
    """The UI URL and location ID from `sqb dev`'s plain `ready` block."""
    wait_for_log(process, log, "  location   ", timeout=timeout)
    text: str = log.read_text()
    url: str = text.partition("\u2713 ready  ")[2].splitlines()[0]
    identity: str = text.partition("  location   ")[2].split()[0]
    return url, identity


def wait_for_connection_health(
    api: SnapshotApi, process: subprocess.Popen[bytes], log: Path
) -> dict[str, Any]:
    """The location's first connection report, sent once its own connection check finished."""
    deadline: float = time.monotonic() + 45
    reports: list[dict[str, Any]] = []
    while not reports:
        assert process.poll() is None, log.read_text()
        assert time.monotonic() < deadline, log.read_text()
        time.sleep(0.05)
        reports = list(
            filter(
                lambda health: health["kind"] == "connection",
                (report["report"]["health"] for report in location_health_reports(api)),
            )
        )
    return reports[0]


def read_terminal_until(
    terminal: int,
    process: subprocess.Popen[bytes],
    done: Callable[[str], bool],
    *,
    timeout: float = 60,
) -> str:
    """Everything the process wrote to its terminal until `done` holds, read as it arrives."""
    deadline: float = time.monotonic() + timeout
    written: bytes = b""
    while not done(written.decode(errors="replace")):
        assert process.poll() is None, written.decode(errors="replace")
        assert time.monotonic() < deadline, written.decode(errors="replace")
        readable, _, _ = select.select([terminal], [], [], 0.1)
        written += b"".join(os.read(ready, 65536) for ready in readable)
    return written.decode(errors="replace")


def wait_for_idle_sync(process: subprocess.Popen[bytes], log: Path) -> None:
    """Wait until the latest synchronization has completed, so a held lock meets the next one."""

    deadline: float = time.monotonic() + 45
    text: str = log.read_text()
    while text.rfind("synchronization completed") < text.rfind("Reading warehouse history"):
        assert process.poll() is None, text
        assert time.monotonic() < deadline, text
        time.sleep(0.05)
        text = log.read_text()


def location_health_reports(api: SnapshotApi) -> list[dict[str, Any]]:
    request: Request = Request(
        f"{api.url}/api/locations/{api.location_id}/health",
        headers={"Authorization": f"Bearer {api.bearer}"},
    )
    with urlopen(request, timeout=2) as response:
        return json.load(response)


def wait_for_warehouse_sync_health(
    api: SnapshotApi,
    expected: dict[str, Any],
    process: subprocess.Popen[bytes],
    log: Path,
    *,
    after: datetime,
) -> list[dict[str, Any]]:
    deadline: float = time.monotonic() + 45
    reports: list[dict[str, Any]] = []
    while {report["report"]["health"]["table"] for report in reports} != set(expected["tables"]):
        assert process.poll() is None, log.read_text()
        by_outcome: dict[tuple[str, str | None, bool], list[dict[str, Any]]] = {}
        for report in location_health_reports(api):
            health: dict[str, Any] = report["report"]["health"]
            key: tuple[str, str | None, bool] = (
                health["kind"],
                health.get("failure"),
                datetime.fromisoformat(report["received_at"]) > after,
            )
            by_outcome.setdefault(key, []).append(report)
        reports = list(
            filter(
                lambda report: report["report"]["health"]["table"] in expected["tables"],
                by_outcome.get(("warehouse_sync", expected["failure"], True), []),
            )
        )
        assert time.monotonic() < deadline, (reports, log.read_text())
        time.sleep(0.05)
    for report in reports:
        assert report["report"]["target"] == "development"
        health = report["report"]["health"]
        assert health["generation"] == 1
        assert health["location"] == {"database": None, "schema": "main"}
        assert 0 <= health["records_sent"] <= 2000
        assert health["elapsed_millis"] > 0
        assert bool(health.get("error")) == (expected["failure"] == "read"), health
    return reports


def wait_for_location_health(
    api: SnapshotApi,
    target: str,
    health: dict[str, Any],
    process: subprocess.Popen[bytes],
    log: Path,
    *,
    minimum_reports: int = 1,
) -> dict[str, Any]:
    """Wait for reports matching every field of `health`; fields it leaves out may differ."""
    deadline: float = time.monotonic() + 15 + 30 * (minimum_reports - 1)
    key: tuple[str, str] = (target, json.dumps(health, sort_keys=True))
    observations: dict[tuple[str, str], dict[str, dict[str, Any]]] = {}
    while len(observations.get(key, ())) < minimum_reports:
        assert process.poll() is None, log.read_text()
        assert time.monotonic() < deadline, log.read_text()
        for report in reversed(location_health_reports(api)):
            event_id: str = report["report"]["event_id"]
            reported: dict[str, Any] = report["report"]["health"]
            observed_key: tuple[str, str] = (
                report["report"]["target"],
                json.dumps({name: reported.get(name) for name in health}, sort_keys=True),
            )
            observations.setdefault(observed_key, {})[event_id] = report
        time.sleep(0.02)
    return list(observations[key].values())[-1]


def published_connection_health(
    api: SnapshotApi,
    expected: tuple[str, bool],
    published: tuple[dict[str, Any], str],
    process: subprocess.Popen[bytes],
    log: Path,
) -> dict[str, Any]:
    """The timed connection check after a snapshot published, on a local DuckDB connection."""
    target, healthy = expected
    snapshot, trigger = published
    health: dict[str, Any] = wait_for_location_health(
        api,
        target,
        {
            "kind": "connection",
            "generation": snapshot["record"]["generation"],
            "healthy": healthy,
            "trigger": trigger,
        },
        process,
        log,
    )
    assert isinstance(health["report"]["health"]["elapsed_millis"], int)
    connection: dict[str, Any] = snapshot["documents"]["connection"]
    assert (connection["adapter"], connection["auth_method"]) == ("duckdb", "none")
    return health


def wait_for_interpreter(process: subprocess.Popen[bytes], pidfile: Path, log: Path) -> int:
    deadline: float = time.monotonic() + 15
    while not pidfile.exists():
        assert process.poll() is None, log.read_text()
        assert time.monotonic() < deadline, log.read_text()
        time.sleep(0.02)
    return int(pidfile.read_text())


def wait_for_location_work(
    api: SnapshotApi, request_id: str, process: subprocess.Popen[bytes], log: Path
) -> dict[str, Any]:
    """Observe completion through the scoped API rather than private coordination tables."""
    request: Request = Request(
        f"{api.url}/api/locations/{api.location_id}/work/{request_id}",
        headers={"Authorization": f"Bearer {api.bearer}"},
    )
    deadline: float = time.monotonic() + 15
    result: dict[str, Any] = {"result_json": None}
    while result["result_json"] is None:
        assert process.poll() is None, log.read_text()
        assert time.monotonic() < deadline, log.read_text()
        with urlopen(request, timeout=2) as response:
            result = json.load(response)
        time.sleep(0.02)
    return result


def network_address() -> str:
    """The IPv4 address other hosts reach this machine on; connecting UDP sends nothing."""
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as route:
        route.connect(("192.0.2.1", 9))
        address: str = route.getsockname()[0]
    return address


def api_address(process: subprocess.Popen[bytes], log: Path) -> str:
    """Wait for bounded native startup, never publishing an unready address."""
    deadline: float = time.monotonic() + 10
    while "Control API ready: " not in log.read_text():
        assert process.poll() is None, log.read_text()
        assert time.monotonic() < deadline, "Native API did not become ready"
        time.sleep(0.02)
    return "http://" + log.read_text().partition("Control API ready: ")[2].splitlines()[0]


def control_query(
    database: str, statement: LiteralString, *parameters: object
) -> list[tuple[Any, ...]]:
    """One committed statement's rows, with `?` placeholders, from either kind of control store."""

    def on_sqlite() -> list[tuple[Any, ...]]:
        with closing(sqlite3.connect(database.removeprefix("sqlite://"))) as control, control:
            return control.execute(statement, parameters).fetchall()

    def on_postgres() -> list[tuple[Any, ...]]:
        with psycopg.connect(database) as control:
            return control.execute(statement.replace("?", "%s"), parameters).fetchall()

    return {True: on_sqlite, False: on_postgres}[database.startswith("sqlite://")]()


@contextmanager
def postgres_database() -> Iterator[str]:
    """A throwaway PostgreSQL control store."""
    with PostgresContainer(POSTGRES_IMAGE, driver=None) as postgres:
        yield postgres.get_connection_url()


def control_database(backend: str) -> AbstractContextManager[str | None]:
    """The store's URL, or None for the SQLite one each server creates."""
    databases: dict[str, Callable[[], AbstractContextManager[str | None]]] = {
        "sqlite": partial(nullcontext, None),
        "postgres": postgres_database,
    }
    return databases[backend]()


@contextmanager
def snapshot_api(
    directory: Path, *, server_arguments: tuple[str, ...] = (), database_url: str | None = None
) -> Iterator[SnapshotApi]:
    """Only this fixture seeds the control database; the code location uses HTTP exclusively."""
    directory.mkdir(parents=True)
    log: Path = directory / "server.log"
    environment: dict[str, str] = os.environ.copy()
    environment.pop("SQLBUILD_CONTROL_DATABASE_URL", None)
    environment.pop("SQLBUILD_PLATFORM_WEBHOOK_URL", None)
    environment.pop("SQLBUILD_SECURITY_AUDIT_WEBHOOK_URL", None)
    environment["XDG_STATE_HOME"] = str(directory / "state")
    environment |= {
        True: {},
        False: {"SQLBUILD_CONTROL_DATABASE_URL": str(database_url)},
    }[database_url is None]
    database: str = database_url or f"sqlite://{directory}/.sqlbuild/control.sqlite3"
    with log.open("wb") as stderr:
        process: subprocess.Popen[bytes] = subprocess.Popen(
            [
                str(Path(sys.executable).with_name("sqb")),
                "server",
                "--port",
                "0",
                "--no-scheduler",
                *server_arguments,
            ],
            cwd=directory,
            env=environment,
            stdout=subprocess.DEVNULL,
            stderr=stderr,
        )
        try:
            url: str = api_address(process, log)
            location_id: str = str(uuid4())
            token_id: str = str(uuid4())
            bearer: str = f"{token_id}.{secrets.token_hex(32)}"
            now: datetime = datetime.now(UTC)
            _ = control_query(
                database,
                "INSERT INTO sqb_locations (location_id,name,created_by,created_at) "
                "VALUES (?, 'orders', 'test-admin', ?) RETURNING location_id",
                location_id,
                now.isoformat(),
            )
            _ = control_query(
                database,
                "INSERT INTO sqb_access_tokens "
                "(token_id,secret_hash,location_id,scope_json,created_at,expires_at) "
                "VALUES (?,?,?,?,?,?) RETURNING token_id",
                token_id,
                hashlib.sha256(bearer.encode()).hexdigest(),
                location_id,
                json.dumps({"kind": "location", "location_id": location_id}),
                now.isoformat(),
                (now + timedelta(hours=1)).isoformat(),
            )
            yield SnapshotApi(url=url, location_id=location_id, bearer=bearer)
        finally:
            process.terminate()
            try:
                _ = process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                _ = process.wait(timeout=5)


def current_snapshot(api: SnapshotApi, target: str) -> dict[str, Any]:
    """Observe the public API instead of inspecting stored snapshot implementation details."""
    request: Request = Request(
        f"{api.url}/api/locations/{api.location_id}/targets/{quote(target, safe='')}/snapshots",
        headers={"Authorization": f"Bearer {api.bearer}"},
    )
    with urlopen(request, timeout=5) as response:
        result: dict[str, Any] = json.load(response)
    return result


def toml_table(value: dict[str, Any]) -> str:
    """Inline TOML table; every key is quoted so any name survives."""
    return (
        "{"
        + ", ".join(f"{json.dumps(str(key))} = {toml_value(item)}" for key, item in value.items())
        + "}"
    )


def toml_array(value: list[Any]) -> str:
    """Inline TOML array."""
    return "[" + ", ".join(toml_value(item) for item in value) + "]"


def toml_value(value: Any) -> str:
    """One-line TOML for JSON-shaped settings; scalars share JSON's spelling."""
    writers: dict[type, Callable[[Any], str]] = {dict: toml_table, list: toml_array}
    return writers.get(type(value), json.dumps)(value)


def write_server_config(path: Path, sections: dict[str, Any]) -> None:
    """Write `server.toml` with each section as an inline table."""
    _ = path.write_text(
        "".join(f"{name} = {toml_value(value)}\n" for name, value in sections.items())
    )


def native_launcher(directory: Path, executable: str) -> AbstractContextManager[WorkerDeployment]:
    """Use the installed native launcher without a synthetic child."""
    return nullcontext(
        WorkerDeployment(
            "local", {"project_dir": str(directory / "project"), "executable": executable}
        )
    )


@contextmanager
def uncooperative_launcher(directory: Path, executable: str) -> Iterator[WorkerDeployment]:
    """Own a synthetic process that ignores TERM, including cleanup after a failed assertion."""
    del executable
    script: Path = directory / "worker"
    _ = script.write_text(
        f"#!{sys.executable}\nimport os, signal, time\nfrom pathlib import Path\n"
        "signal.signal(signal.SIGTERM, signal.SIG_IGN)\n"
        "Path('worker.pid').write_text(str(os.getpid()))\n"
        "time.sleep(90)\n"
    )
    script.chmod(0o700)
    try:
        yield WorkerDeployment(
            "local", {"project_dir": str(directory / "project"), "executable": str(script)}
        )
    finally:
        with suppress(FileNotFoundError, ProcessLookupError):
            os.kill(int((directory / "project/worker.pid").read_text()), signal.SIGKILL)


@contextmanager
def scheduled_worker(
    directory: Path,
    monkeypatch: pytest.MonkeyPatch,
    test_case: WorkerExecutionTestCase,
    *,
    server_arguments: tuple[str, ...] = (),
    serve_sensors: bool = True,
    scheduler_settings: dict[str, Any] | None = None,
    database_url: str | None = None,
) -> Iterator[ScheduledWorker]:
    """Declare, publish and schedule isolated DuckDB work without provisioning claims."""
    project: Path = directory / "project"
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": (
                'name = "orders"\nadapter = "duckdb"\n'
                '[connections.local]\ndatabase = "warehouse.duckdb"\n'
                '[targets.development]\nschema = "main"\n'
                "[pools]\n"
                + "".join(f"{name} = {limit}\n" for name, limit in test_case.pools.items())
            ),
            "sql/models/orders.sql": (
                "MODEL (description 'Orders.', materialized table);\n" + test_case.model_sql + "\n"
            ),
            "python/nodes/environment_guard.py": (
                "import os\n"
                "if 'SQLBUILD_RUN_TOKEN' in os.environ:\n"
                "    assert 'SQLBUILD_PLATFORM_WEBHOOK_URL' not in os.environ\n"
                "    assert 'SQLBUILD_SECURITY_AUDIT_WEBHOOK_URL' not in os.environ\n"
            ),
        }
        | test_case.default_job_files
        | test_case.files,
    )
    test_case.warehouse_setup(project / "warehouse.duckdb")
    executable: str = str(Path(sys.executable).with_name("sqb"))
    launchers: dict[str, Callable[[Path, str], AbstractContextManager[WorkerDeployment]]] = {
        "native": native_launcher,
        "docker": docker_deployment,
        "uncooperative": uncooperative_launcher,
    }
    with (
        snapshot_api(
            directory / "control", server_arguments=server_arguments, database_url=database_url
        ) as api,
        launchers[test_case.launcher](directory, executable) as deployment,
    ):
        published: subprocess.CompletedProcess[str] = subprocess.run(
            [
                executable,
                "location",
                "publish",
                "--project-dir",
                str(project),
                "--location-id",
                api.location_id,
                "--target",
                "development",
                "--server",
                api.url,
                "--generation",
                "1",
                *deployment.publication_arguments,
            ],
            env=os.environ | {"SQLBUILD_LOCATION_TOKEN": api.bearer},
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )
        assert published.returncode == 0, published.stderr
        write_files(project_dir=project, files=test_case.files_after_publication)
        configuration: Path = directory / "server.toml"
        launcher: dict[str, Any] = (
            {
                "location": "orders",
                "target": "development",
                "server_url": api.url,
                "max_workers": 1,
            }
            | deployment.configuration
            | test_case.launcher_settings
        )
        write_server_config(
            configuration,
            {"launchers": {deployment.kind: [launcher]}} | (scheduler_settings or {}),
        )
        control_store: Path = directory / "control/.sqlbuild/control.sqlite3"

        def edit_sqlite_store() -> None:
            with sqlite3.connect(control_store) as control:
                _ = control.executescript(test_case.store_setup)
                test_case.store_edit(control)

        # A case's store edits are SQLite's.
        {True: edit_sqlite_store, False: lambda: None}[database_url is None]()
        monkeypatch.setenv(
            "SQLBUILD_SCHEDULER_TEST_STORE", database_url or f"sqlite://{control_store}"
        )
        monkeypatch.setenv(
            "SQLBUILD_PLATFORM_WEBHOOK_URL",
            "http://127.0.0.1:1/events?token=synthetic-delivery-credential",
        )
        monkeypatch.setenv(
            "SQLBUILD_SECURITY_AUDIT_WEBHOOK_URL",
            "http://127.0.0.1:1/audit?token=synthetic-audit-credential",
        )
        log: Path = directory / "scheduler.log"
        serves: bool = serve_sensors and any(
            path.startswith("orchestration/sensors/") for path in test_case.files
        )
        locations: dict[bool, Callable[[], AbstractContextManager[object]]] = {
            True: partial(
                location_process,
                [
                    executable,
                    "--project-dir",
                    str(project),
                    "location",
                    "serve",
                    "--server",
                    api.url,
                    "--location-id",
                    api.location_id,
                    "--target",
                    "development",
                ],
                directory / "location.log",
                directory / "location.stdout",
                env=os.environ | {"SQLBUILD_LOCATION_TOKEN": api.bearer},
            ),
            False: nullcontext,
        }
        with ExitStack() as serving:
            # A project with sensors gets a serving location, which runs their functions.
            _ = serving.enter_context(locations[serves]())
            process: subprocess.Popen[bytes] = serving.enter_context(
                location_process(
                    [
                        executable,
                        "scheduler",
                        "--database-env",
                        "SQLBUILD_SCHEDULER_TEST_STORE",
                        "--config",
                        str(configuration),
                    ],
                    log,
                    directory / "scheduler.stdout",
                )
            )
            yield ScheduledWorker(api, project, control_store, process, log, deployment.docker)


def worker_result(worker: ScheduledWorker, test_case: WorkerExecutionTestCase) -> dict[str, Any]:
    """Run/node outcomes, Recall, claims and the physical warehouse agree."""
    api, project, control_store, process, log, _ = worker
    run: dict[str, Any] = wait_for_run(api, process, log)
    assert run["node_counts_json"] == (
        test_case.expected_node_counts or {test_case.expected_status: test_case.expected_node_count}
    ), log.read_text()
    assert run["job_key"] == "annual_orders_job"
    runs_url: str = f"{api.url}/api/locations/{api.location_id}/targets/development/runs"
    headers: dict[str, str] = {"Authorization": f"Bearer {api.bearer}"}
    with urlopen(
        Request(f"{runs_url}/{run['run_id']}/nodes", headers=headers), timeout=5
    ) as response:
        nodes: list[dict[str, Any]] = json.load(response)
    assert nodes[0]["node_key"] == "model:orders"
    assert nodes[0]["started_at"] is not None
    assert nodes[0]["ended_at"] is not None
    with sqlite3.connect(control_store) as control:
        assert control.execute(
            "SELECT origin FROM sqb_runs WHERE run_id=?", (run["run_id"],)
        ).fetchone() == (test_case.expected_origin,)
        assert control.execute(
            "SELECT count(*) FROM recall_events "
            "WHERE invocation_id=? AND event_type='run_launched'",
            (f"scheduler/{run['run_id']}",),
        ).fetchone() == (1,)
        contexts: list[tuple[str | None]] = control.execute(
            "SELECT external_context FROM recall_invocations WHERE command = 'location'"
        ).fetchall()
        assert contexts, log.read_text()
        assert all(context is not None for (context,) in contexts)
        printed: str = "".join(
            str(data)
            for (data,) in control.execute(
                "SELECT data FROM recall_output WHERE invocation_id LIKE 'worker/%' "
                "ORDER BY occurred_at, sequence"
            ).fetchall()
        )
        assert ("Planning from the published compiled project." not in printed) == (
            test_case.expected_compiles_again
        ), printed
        assert ("Compiling project..." in printed) == test_case.expected_compiles_again, printed
        expected_run_ids: set[str] = {
            run_id
            for (run_id,) in control.execute(
                "SELECT run_id FROM sqb_runs WHERE kind<>'group' AND ended_at IS NOT NULL"
            ).fetchall()
        }
        assert {
            json.loads(cast(str, context))["run_id"] for (context,) in contexts
        } == expected_run_ids
        assert control.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (0,)
    with duckdb.connect(str(project / "warehouse.duckdb"), read_only=True) as warehouse:
        assert warehouse.sql(test_case.warehouse_query).fetchall() == test_case.expected_rows
    return run


def running_statement(worker: ScheduledWorker, *, job_key: str | None = None) -> dict[str, Any]:
    """The worker has started the real long query, not merely entered its Python interpreter."""
    active: dict[str, Any] = wait_for_run(
        worker.api,
        worker.process,
        worker.log,
        pending=frozenset({"queued", "launched"}),
        job_key=job_key,
    )
    assert active["status"] == "running", worker.log.read_text()
    deadline: float = time.monotonic() + 15
    with sqlite3.connect(worker.control_store) as control:
        while not control.execute(
            "SELECT 1 FROM recall_artifacts WHERE CAST(content AS TEXT) LIKE ?",
            ("%range(10000000000)%",),
        ).fetchone():
            assert worker.process.poll() is None, worker.log.read_text()
            assert time.monotonic() < deadline, worker.log.read_text()
            time.sleep(0.02)
    return active


def cancel_running_statement(
    worker: ScheduledWorker, *, job_key: str | None = None, run_id: str | None = None
) -> None:
    """Wait for the real long-running query before exercising the administrative cancel command."""
    active: dict[str, Any] = running_statement(worker, job_key=job_key)
    cancelled: subprocess.CompletedProcess[str] = subprocess.run(
        [
            str(Path(sys.executable).with_name("sqb")),
            "admin",
            "--database-env",
            "SQLBUILD_SCHEDULER_TEST_STORE",
            "cancel-run",
            run_id or active["run_id"],
            "--event-id",
            str(uuid4()),
            "--reason",
            "Cancel the active test statement.",
        ],
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )
    assert cancelled.returncode == 0, cancelled.stderr


def wait_for_run(
    api: SnapshotApi,
    process: subprocess.Popen[bytes],
    log: Path,
    *,
    pending: frozenset[str] = frozenset({"queued", "launched", "running", "unresponsive"}),
    job_key: str | None = None,
    run_id: str | None = None,
    target: str = "development",
) -> dict[str, Any]:
    """A terminal API projection must follow the scheduler's actual isolated warehouse execution."""
    request: Request = Request(
        f"{api.url}/api/locations/{api.location_id}/targets/{quote(target, safe='')}/runs",
        headers={"Authorization": f"Bearer {api.bearer}"},
    )
    deadline: float = time.monotonic() + 30
    current: dict[str, Any] = {"status": "queued"}
    while current["status"] in pending:
        assert process.poll() is None, log.read_text()
        assert time.monotonic() < deadline, log.read_text()
        with urlopen(request, timeout=2) as response:
            runs: list[dict[str, Any]] = json.load(response)
            by_job: dict[str | None, dict[str, Any]] = {run["job_key"]: run for run in runs}
            by_job[None] = next(iter(runs), current)
            by_id: dict[str | None, dict[str, Any]] = {run["run_id"]: run for run in runs}
            by_id[None] = by_job.get(job_key, current)
            current = by_id.get(run_id, current)
        time.sleep(0.02)
    return current


def claimed_worker(directory: Path, api: SnapshotApi, publication: dict[str, Any]) -> ClaimedWorker:
    """Provision only the launch boundary; verification and lifecycle reporting use real HTTP."""
    run_id: str = str(uuid4())
    claim_id: str = str(uuid4())
    token_id: str = str(uuid4())
    bearer: str = f"{token_id}.{secrets.token_hex(32)}"
    now: datetime = datetime.now(UTC)
    selection: list[str] = ["model:orders"]
    request: dict[str, Any] = {
        "run_id": run_id,
        "location_id": api.location_id,
        "target": publication["target"],
        "snapshot_id": publication["snapshot_id"],
        "image_id": None,
        "kind": "job",
        "origin": "manual",
        "job_key": None,
        "parent_run_id": None,
        "root_run_id": run_id,
        "trigger_id": None,
        "tick_id": None,
        "rerun_of_run_id": None,
        "requested_by": "test-admin",
        "priority": 0,
        "selection": selection,
        "write_set": selection,
        "settings": {"full_refresh": True},
    }
    with sqlite3.connect(directory / ".sqlbuild/control.sqlite3") as connection:
        _ = connection.execute(
            "INSERT INTO sqb_runs (run_id,location_id,target,snapshot_id,kind,origin,root_run_id,"
            "requested_by,selection_json,request_json,write_set_digest,status,queued_at,"
            "node_counts_json,last_event_storage_id) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            (
                run_id,
                api.location_id,
                publication["target"],
                publication["snapshot_id"],
                "job",
                "manual",
                run_id,
                "test-admin",
                json.dumps(selection),
                json.dumps(request),
                hashlib.sha256(json.dumps(selection).encode()).hexdigest(),
                "queued",
                now.isoformat(),
                json.dumps({"pending": 1}),
                0,
            ),
        )
        _ = connection.execute(
            "INSERT INTO sqb_run_nodes (run_id,node_key,status,writes) "
            "VALUES (?,'model:orders','pending',TRUE)",
            (run_id,),
        )
        _ = connection.execute(
            "INSERT INTO sqb_write_claims (target,node_key,run_id) VALUES (?,?,?)",
            (publication["target"], "model:orders", run_id),
        )
        _ = connection.execute(
            "UPDATE sqb_runs SET worker_instance=?, claimed_at=?, last_heartbeat_at=? "
            "WHERE run_id=?",
            (claim_id, now.isoformat(), now.isoformat(), run_id),
        )
        _ = connection.execute(
            "INSERT INTO sqb_access_tokens "
            "(token_id,secret_hash,run_id,scope_json,created_at,expires_at) "
            "VALUES (?,?,?,?,?,?)",
            (
                token_id,
                hashlib.sha256(bearer.encode()).hexdigest(),
                run_id,
                json.dumps({"kind": "run", "run_id": run_id, "claim_id": claim_id}),
                now.isoformat(),
                (now + timedelta(hours=1)).isoformat(),
            ),
        )
    return ClaimedWorker(run_id=run_id, claim_id=claim_id, bearer=bearer)


def wait_for_sensor_phase(
    store: Path, phase: str, *, empty_runs: bool = False, timeout: float = 45
) -> dict[str, Any]:
    """Wait for real scheduler facts, optionally requiring a completed skip."""
    deadline: float = time.monotonic() + timeout
    row: tuple[str] | None = None
    while row is None:
        assert time.monotonic() < deadline, f"Sensor did not record {phase}"
        with sqlite3.connect(store) as control:
            row = control.execute(
                "SELECT fact_json FROM sqb_sensor_evaluation_events WHERE phase=? "
                "AND (? = FALSE OR fact_json->'outcome'->'run_ids'='[]') "
                "ORDER BY occurred_at DESC, evaluation_id DESC LIMIT 1",
                (phase, empty_runs),
            ).fetchone()
        time.sleep(0.05)
    return json.loads(row[0])


def sensor_calls(path: Path, fields: dict[str, Any]) -> list[dict[str, Any]]:
    """Select recorded callback invocations without inferring execution from declarations."""
    records: list[dict[str, Any]] = [json.loads(line) for line in path.read_text().splitlines()]
    return list(filter(lambda record: fields.items() <= record.items(), records))


def append_retried_fingerprints(
    *,
    adapter: Any,
    connection: Any,
    database_path: Path,
    fingerprint: Fingerprint,
    read_columns: Callable[..., Any] | None = None,
) -> None:
    """Retry the same observation without substituting its identity or live catalog."""
    del database_path
    for _attempt in range(2):
        _ = write_fingerprint(
            connection=connection,
            execute=adapter.execute,
            database=None,
            schema="main",
            fingerprint=fingerprint,
            ensure_table=False,
            render_qualified_name=adapter.render_qualified_name,
            render_framework_type=adapter.render_framework_type,
            read_columns=read_columns or adapter.get_columns,
            render_add_columns=adapter.render_add_columns,
            render_create_table_sql=adapter.render_create_fingerprint_table_sql,
        )


def append_contended_fingerprints(
    *, adapter: Any, connection: Any, database_path: Path, fingerprint: Fingerprint
) -> None:
    """Release a real SQLite write lock only after the first ALTER has failed."""
    with closing(sqlite3.connect(database_path)) as blocker:
        _ = adapter.execute(connection=connection, sql="PRAGMA busy_timeout=1")
        catalog_actions: Iterator[Callable[[], Any]] = iter(
            (lambda: blocker.execute("BEGIN IMMEDIATE"), blocker.commit)
        )

        def read_columns(**scope: Any) -> Any:
            columns: Any = adapter.get_columns(**scope)
            next(catalog_actions, lambda: None)()
            return columns

        try:
            append_retried_fingerprints(
                adapter=adapter,
                connection=connection,
                database_path=database_path,
                fingerprint=fingerprint,
                read_columns=read_columns,
            )
        finally:
            blocker.rollback()


def run_statistics_retention(directory: Path, arguments: tuple[str, ...]) -> str:
    """Run one installed server until its first retention pass, returning its stderr."""
    return run_server_until(
        directory,
        ("--no-scheduler", *arguments),
        lambda _: "sentinel_expired" not in statistics_relations(directory),
    )


def run_server_until(
    directory: Path,
    arguments: tuple[str, ...],
    finished: Callable[[str], bool],
    environment_overrides: dict[str, str] | None = None,
) -> str:
    """Run one installed server until its stderr or store satisfies `finished`, returning stderr."""
    log: Path = directory / "server.log"
    environment: dict[str, str] = os.environ.copy()
    environment.pop("SQLBUILD_CONTROL_DATABASE_URL", None)
    environment.update(environment_overrides or {})
    with log.open("wb") as stderr:
        process: subprocess.Popen[bytes] = subprocess.Popen(
            [str(Path(sys.executable).with_name("sqb")), "server", "--port", "0", *arguments],
            cwd=directory,
            env=environment,
            stdout=subprocess.DEVNULL,
            stderr=stderr,
        )
        try:
            _ = api_address(process, log)
            deadline: float = time.monotonic() + 10
            while not finished(log.read_text()):
                assert process.poll() is None, log.read_text()
                assert time.monotonic() < deadline, "server did not reach the expected state"
                time.sleep(0.05)
        finally:
            process.terminate()
            _ = process.wait(timeout=10)
    return log.read_text()


def audit_actions(directory: Path) -> tuple[str, ...]:
    """Read the synthetic retention fixture's audit actions directly, in storage order."""
    with sqlite3.connect(directory / ".sqlbuild/control.sqlite3") as connection:
        rows: list[tuple[str]] = connection.execute(
            "SELECT action FROM sqb_security_audit WHERE action LIKE 'orders.%' ORDER BY storage_id"
        ).fetchall()
    return tuple(action for (action,) in rows)


def statistics_relations(directory: Path) -> tuple[str, ...]:
    """Read the retained sample relations directly, oldest first."""
    with sqlite3.connect(directory / ".sqlbuild/control.sqlite3") as connection:
        rows: list[tuple[str]] = connection.execute(
            "SELECT relation_name FROM sqb_warehouse_statistics ORDER BY observed_at"
        ).fetchall()
    return tuple(name for (name,) in rows)


def scheduler_crash_history(directory: Path, fixture: dict[str, Any]) -> dict[str, int]:
    """SIGKILL real schedulers during catch-up; return breaches and the kill count."""
    project: Path = directory / "project"
    start: datetime = datetime.now(UTC).replace(second=0, microsecond=0) - timedelta(
        minutes=fixture["backlog_minutes"]
    )
    jobs: dict[str, str] = {
        f"orchestration/jobs/{name}.toml": (
            f'name = "{name}"\ndescription = "Build {name}."\nselect = "orders"\n'
            'schedule = { cron = "* * * * *", timezone = "UTC" }\n'
        )
        for name in fixture["jobs"]
    }
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": (
                'name = "orders"\nadapter = "duckdb"\n'
                '[connections.local]\ndatabase = "warehouse.duckdb"\n'
                '[targets.development]\nschema = "main"\n'
            ),
            "sql/models/orders.sql": (
                "MODEL (description 'Orders.', materialized table);\nSELECT 1 AS order_id\n"
            ),
        }
        | jobs,
    )
    executable: str = str(Path(sys.executable).with_name("sqb"))
    control_store: Path = directory / "control/.sqlbuild/control.sqlite3"
    environment: dict[str, str] = os.environ | {
        "SQLBUILD_SCHEDULER_TEST_STORE": f"sqlite://{control_store}"
    }
    for name in ("SQLBUILD_PLATFORM_WEBHOOK_URL", "SQLBUILD_SECURITY_AUDIT_WEBHOOK_URL"):
        _ = environment.pop(name, None)
    with snapshot_api(directory / "control") as api:
        published: subprocess.CompletedProcess[str] = subprocess.run(
            [
                executable,
                "location",
                "publish",
                "--project-dir",
                str(project),
                "--location-id",
                api.location_id,
                "--target",
                "development",
                "--server",
                api.url,
                "--generation",
                "1",
            ],
            env=os.environ | {"SQLBUILD_LOCATION_TOKEN": api.bearer},
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
        )
        assert published.returncode == 0, published.stderr
    # The triggers were published hours ago, so a backlog of minutes is due when schedulers start.
    with closing(sqlite3.connect(control_store)) as control, control:
        _ = control.execute(
            "UPDATE sqb_trigger_state SET started_at=?, next_due_at=?, last_tick_for=NULL",
            (start.strftime("%Y-%m-%dT%H:%M:%S.%fZ"),) * 2,
        )
    choices: random.Random = random.Random(fixture["seed"])
    command: list[str] = [
        executable,
        "scheduler",
        "--database-env",
        "SQLBUILD_SCHEDULER_TEST_STORE",
    ]
    kills: int = 0
    for round_index in range(fixture["rounds"]):
        processes: list[subprocess.Popen[bytes]] = [
            subprocess.Popen(
                command,
                env=environment,
                stdout=subprocess.DEVNULL,
                stderr=(directory / f"scheduler-{round_index}-{index}.log").open("wb"),
            )
            for index in range(2)
        ]
        time.sleep(choices.uniform(0.05, fixture["max_kill_delay_ms"] / 1000))
        for process in processes:
            process.kill()
            kills += int(process.wait(timeout=10) == -signal.SIGKILL)
        # A killed holder's lease would lapse after its TTL; lapse it now instead of waiting.
        with closing(sqlite3.connect(control_store, timeout=30)) as control, control:
            _ = control.execute(
                "UPDATE sqb_scheduler_lease SET expires_at='2000-01-01T00:00:00.000000Z'"
            )
    settled: datetime = datetime.now(UTC)
    with closing(sqlite3.connect(control_store, timeout=30)) as control:
        (interrupted,) = control.execute("SELECT count(*) FROM sqb_ticks").fetchone()
    with location_process(
        command, directory / "survivor.log", directory / "survivor.stdout", env=environment
    ):
        deadline: float = time.monotonic() + 60
        pending: int = 1
        while pending and time.monotonic() < deadline:
            time.sleep(0.5)
            with closing(sqlite3.connect(control_store, timeout=30)) as control:
                (pending,) = control.execute(
                    "SELECT count(*) FROM sqb_trigger_state "
                    "WHERE last_tick_for IS NULL OR last_tick_for<?",
                    (settled.replace(second=0, microsecond=0).strftime("%Y-%m-%dT%H:%M:%S.%fZ"),),
                ).fetchone()
    with closing(sqlite3.connect(control_store)) as control:
        ticks: list[tuple[str, str, str | None, str | None]] = control.execute(
            "SELECT trigger_id, scheduled_for, run_id, covered_by_run_id FROM sqb_ticks"
        ).fetchall()
        runs: set[str] = {run for (run,) in control.execute("SELECT run_id FROM sqb_runs")}
        (requested,) = control.execute(
            "SELECT count(*) FROM recall_events WHERE event_type='run_requested'"
        ).fetchone()
        (triggers,) = control.execute("SELECT count(*) FROM sqb_trigger_state").fetchone()
        duplicate_rejected: bool = True
        with suppress(sqlite3.IntegrityError):
            _ = control.execute(
                "INSERT INTO sqb_ticks (tick_id, trigger_id, scheduled_for, evaluated_at, epoch, "
                "outcome, run_id, covered_by_run_id) "
                "SELECT ?, trigger_id, scheduled_for, evaluated_at, epoch, 'skipped', NULL, NULL "
                "FROM sqb_ticks LIMIT 1",
                (str(uuid4()),),
            )
            duplicate_rejected = False
    # A minute can fall due while the survivor runs; the one due at `settled` or a later one ticks.
    latest: datetime = settled.replace(second=0, microsecond=0)
    observed: list[tuple[str, datetime]] = [
        (trigger, datetime.fromisoformat(scheduled)) for trigger, scheduled, _, _ in ticks
    ]
    newest: dict[str, datetime] = dict(sorted(observed, key=lambda tick: tick[1]))
    requesting: list[str] = list(filter(None, (run for _, _, run, _ in ticks)))
    covering: set[str] = set(filter(None, (covered for _, _, _, covered in ticks)))
    return {
        "triggers_without_ticks": triggers - len({tick[0] for tick in ticks}),
        "duplicate_ticks": len(observed) - len(set(observed)),
        "missing_latest_ticks": sum(minute < latest for minute in newest.values()),
        "off_schedule_ticks": sum(
            minute.second != 0 or minute.microsecond != 0 or minute < start
            for _, minute in observed
        ),
        "runs_without_exactly_one_tick": len(runs ^ set(requesting))
        + len(requesting)
        - len(set(requesting))
        + len(covering - runs)
        + sum((run is None) == (covered is None) for _, _, run, covered in ticks),
        "request_event_mismatch": abs(requested - len(requesting)),
        "accepted_duplicate_ticks": int(not duplicate_rejected),
        "kills": kills,
        "ticks_before_survivor": interrupted,
        "ticks": len(ticks),
    }


def publish_generation(
    project: Path, api: SnapshotApi, generation: int, target: str = "development"
) -> subprocess.CompletedProcess[str]:
    """Publish one generation of a project through the installed location command."""
    return subprocess.run(
        [
            str(Path(sys.executable).with_name("sqb")),
            "location",
            "publish",
            "--project-dir",
            str(project),
            "--location-id",
            api.location_id,
            "--target",
            target,
            "--server",
            api.url,
            "--generation",
            str(generation),
        ],
        env=os.environ | {"SQLBUILD_LOCATION_TOKEN": api.bearer},
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )


def read_person_sql(api: SnapshotApi, session: tuple[str, str], node: str) -> tuple[int, str]:
    """A person's view.sql read of one node: the node key on success, else the denial."""
    path: str = f"/api/control/locations/{api.location_id}/targets/development/nodes/{node}/sql"
    headers: dict[str, str] = {
        "Host": "control.example.com",
        "Origin": "https://control.example.com",
        "Cookie": session[0],
        "x-sqb-csrf": session[1],
    }
    try:
        with urlopen(Request(api.url + path, headers=headers), timeout=5) as response:
            return 200, json.load(response)["node_key"]
    except HTTPError as denied:
        return denied.code, json.load(denied)["error"].split("; decision ")[0]


def tagged_model(name: str, tags: str) -> str:
    """A table model with these tags."""
    return (
        f"MODEL (description 'Synthetic {name}.', materialized table, tags [{tags}]);\n"
        "select 1 as id\n"
    )


# `--changed` compares a relation with each of its executions this close to its newest, so a
# rebuild sooner than this after a build still reports that build's edits and inputs.
ORDER_SKEW_SECONDS: float = 5.2
CHANGED_FIXTURE: Path = Path("benchmarks/duckshop/edge_cases/changed_builds")
CHANGED_JOB: str = (
    'name = "changed_orders_job"\n'
    'description = "Build what changed."\n'
    'select = "stg_orders stg_customers customer_orders"\n'
    "\n[settings]\nchanged = true\n"
)


def duckdb_rows(path: Path, sql: str) -> list[tuple[Any, ...]]:
    """Run `sql` on a DuckDB warehouse file and return its rows."""
    with duckdb.connect(str(path)) as connection:
        return connection.execute(sql).fetchall()


def sqlite_rows(path: Path, sql: str) -> list[tuple[Any, ...]]:
    """Run and commit `sql` on a SQLite warehouse file and return its rows."""
    with closing(sqlite3.connect(path)) as connection, connection:
        return connection.execute(sql).fetchall()


def changed_job_files(project_toml: str) -> dict[str, str]:
    """The DuckShop changed-builds edge case with one scheduled `changed` job."""
    return {
        path.relative_to(CHANGED_FIXTURE).as_posix(): path.read_text()
        for path in sorted(CHANGED_FIXTURE.glob("sql/*/*"))
    } | {
        "orchestration/jobs/changed_orders_job.toml": CHANGED_JOB,
        "sqlbuild_project.toml": project_toml,
    }


def request_job_run(worker: ScheduledWorker, job: str) -> dict[str, Any]:
    """Queue `job` through privileged administration and wait for its terminal run."""
    run_id: str = str(uuid4())
    requested: subprocess.CompletedProcess[str] = subprocess.run(
        [
            str(Path(sys.executable).with_name("sqb")),
            "admin",
            "--database-env",
            "SQLBUILD_SCHEDULER_TEST_STORE",
            "request-run",
            worker.api.location_id,
            job,
            "--target",
            "development",
            "--run-id",
            run_id,
            "--reason",
            "Build what changed",
        ],
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )
    assert requested.returncode == 0, requested.stderr
    return wait_for_run(worker.api, worker.process, worker.log, run_id=run_id)


SHARED_AUDITS: Path = Path("benchmarks/duckshop/edge_cases/orchestration/shared_inline_audits.json")


def create_playground(project: Path) -> None:
    """Create the default playground project with the installed CLI."""
    created: subprocess.CompletedProcess[str] = subprocess.run(
        [str(Path(sys.executable).with_name("sqb")), "playground", str(project)],
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    assert created.returncode == 0, created.stderr


def write_shared_audits(project: Path) -> None:
    """Write the DuckShop project whose models share inline and named audits."""
    write_files(project_dir=project, files=json.loads(SHARED_AUDITS.read_text())["files"])


def audit_node_keys(api: SnapshotApi, target: str) -> frozenset[str]:
    """Keys of the current snapshot's audit nodes."""
    nodes: dict[str, Any] = current_snapshot(api, target)["nodes"]
    return frozenset(filter(lambda key: nodes[key]["kind"] == "audit", nodes))


def worker_statements(control_store: Path, run_id: str) -> int:
    """Statements the run's worker invocations recorded, as the run statement read selects them."""
    with sqlite3.connect(control_store) as control:
        invocations: list[str] = [
            invocation
            for (invocation,) in control.execute(
                "SELECT invocation_id FROM invocations WHERE invocation_id LIKE 'worker/%' "
                "AND json_extract(external_context, '$.run_id') = ?",
                (run_id,),
            ).fetchall()
        ]
        assert invocations, f"run {run_id} has no worker invocation"
        return sum(
            control.execute(
                "SELECT count(*) FROM statements WHERE invocation_id=? OR invocation_id LIKE ?",
                (invocation, invocation + "/%"),
            ).fetchone()[0]
            for invocation in invocations
        )


def security_audit_rows(control_store: Path) -> int:
    """Rows in the security audit, so a test can show a request recorded nothing."""
    with sqlite3.connect(control_store) as control:
        return control.execute("SELECT count(*) FROM sqb_security_audit").fetchone()[0]


def run_sqb(project: Path, *arguments: str) -> subprocess.CompletedProcess[str]:
    """Run this environment's `sqb` on `project` without raising on failure."""
    return subprocess.run(
        [str(Path(sys.executable).with_name("sqb")), "--project-dir", str(project), *arguments],
        capture_output=True,
        text=True,
        check=False,
    )


def live_plan(project: Path, selection: list[str], *flags: str) -> subprocess.CompletedProcess[str]:
    """Run `sqb plan --json` with `flags` on the warehouse for node keys such as `model:orders`."""
    selected: list[str] = []
    for key in selection:
        selected.extend(("--select", key.split(":")[1]))
    return run_sqb(
        project, "plan", "--target", "development", "--json", "--no-color", *flags, *selected
    )


def hold_write_lock(database: Path) -> duckdb.DuckDBPyConnection:
    """Hold the warehouse's write lock, so any process that opens it fails."""
    deadline: float = time.monotonic() + 15
    while True:
        try:
            return duckdb.connect(database)
        except duckdb.IOException:
            assert time.monotonic() < deadline
            time.sleep(0.05)


LOADED_ORDER_FILES: dict[str, str] = {
    "sql/sources/loaded.yml": Path(
        "benchmarks/duckshop/edge_cases/orchestration/loaded_sources.yml"
    ).read_text(),
    **node_module_files(
        "python/nodes/loaders/loaded.py",
        Path("benchmarks/duckshop/edge_cases/orchestration/loaded_sources.py").read_text(),
    ),
    "sql/models/orders.sql": Path(
        "benchmarks/duckshop/edge_cases/orchestration/loaded_orders.sql"
    ).read_text(),
    **duckshop_orchestration("loaded_orders"),
}


INTERMEDIATE_LOADER_FILES: dict[str, str] = {
    "sql/sources/shipments.yml": Path(
        "benchmarks/duckshop/edge_cases/orchestration/intermediate_loader_sources.yml"
    ).read_text(),
    **node_module_files(
        "python/nodes/loaders/shipments.py",
        Path(
            "benchmarks/duckshop/edge_cases/orchestration/intermediate_loader_sources.py"
        ).read_text(),
    ),
    "sql/models/orders.sql": Path(
        "benchmarks/duckshop/edge_cases/orchestration/intermediate_loader_orders.sql"
    ).read_text(),
    **duckshop_orchestration("loaded_orders"),
}


def without_loads(control: sqlite3.Connection, node_key: str) -> None:
    """Drop a published node's `loads` in place; its content address and the worker's check stay."""
    for node_hash, body in control.execute(
        "SELECT d.node_hash, d.body FROM sqb_node_definitions d JOIN sqb_snapshot_nodes n "
        "ON n.node_hash = d.node_hash WHERE n.node_key = ?",
        (node_key,),
    ).fetchall():
        content: dict[str, Any] = json.loads(gzip.decompress(body))
        del content["definition"]["loads"]
        _ = control.execute(
            "UPDATE sqb_node_definitions SET body = ? WHERE node_hash = ?",
            (gzip.compress(json.dumps(content).encode()), node_hash),
        )


def run_write_set(control_store: Path, run_id: str) -> frozenset[str]:
    """The frozen write set a run's request recorded."""
    with closing(sqlite3.connect(control_store)) as control:
        (request,) = control.execute(
            "SELECT request_json FROM sqb_runs WHERE run_id=?", (run_id,)
        ).fetchone()
    return frozenset(json.loads(request)["write_set"])


def create_raw_orders(warehouse: Path) -> None:
    """The source table a changed job's model reads."""
    with duckdb.connect(str(warehouse)) as connection:
        _ = connection.execute(
            "CREATE TABLE main.raw_orders AS SELECT CAST(1 AS BIGINT) AS order_id"
        )


def plan_from_copy(
    tmp_path: Path, project: Path, selection: list[str], settings: dict[str, object]
) -> tuple[subprocess.CompletedProcess[str], dict[str, Any]]:
    """Sync `project` to a location, lock its warehouse, then plan `selection` from the copy.

    Returns the refused live plan run while the lock was held and the location's plan document.
    """
    control: Path = tmp_path / "control/.sqlbuild/control.sqlite3"
    log: Path = tmp_path / "location.log"
    started: datetime = datetime.now(UTC)
    with (
        snapshot_api(tmp_path / "control") as api,
        location_process(
            [
                str(Path(sys.executable).with_name("sqb")),
                "--project-dir",
                str(project),
                "location",
                "serve",
                "--server",
                api.url,
                "--location-id",
                api.location_id,
                "--target",
                "development",
            ],
            log,
            tmp_path / "location.stdout",
            env=os.environ | {"SQLBUILD_LOCATION_TOKEN": api.bearer},
        ) as location,
    ):
        _ = wait_for_generation(api, "development", 1, location, log)
        _ = wait_for_warehouse_sync_health(
            api,
            {"failure": None, "tables": ["catalog", "consumption", "fingerprints", "observations"]},
            location,
            log,
            after=started,
        )
        lock: duckdb.DuckDBPyConnection = hold_write_lock(project / "warehouse.db")
        try:
            refused: subprocess.CompletedProcess[str] = live_plan(project, selection)
            request_id: str = request_plan(control, selection, settings)
            result: dict[str, Any] = wait_for_location_work(api, request_id, location, log)
        finally:
            lock.close()
    outcome: dict[str, Any] = result["result_json"]["outcome"]
    assert outcome["kind"] == "plan", outcome
    return refused, outcome["document"]


def changed_build_and_plan_from_copy(
    tmp_path: Path, project: Path, selection: list[str]
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Serve `project` until its first sync completes.

    Returns the changed build the location published after that sync and the location's
    `--changed` plan of `selection` from the same synced copy.
    """
    control: Path = tmp_path / "control/.sqlbuild/control.sqlite3"
    log: Path = tmp_path / "location.log"
    started: datetime = datetime.now(UTC)
    with (
        snapshot_api(tmp_path / "control") as api,
        location_process(
            [
                str(Path(sys.executable).with_name("sqb")),
                "--project-dir",
                str(project),
                "location",
                "serve",
                "--server",
                api.url,
                "--location-id",
                api.location_id,
                "--target",
                "development",
            ],
            log,
            tmp_path / "location.stdout",
            env=os.environ | {"SQLBUILD_LOCATION_TOKEN": api.bearer},
        ) as location,
    ):
        _ = wait_for_generation(api, "development", 1, location, log)
        synced: datetime = max(
            datetime.fromisoformat(report["received_at"])
            for report in wait_for_warehouse_sync_health(
                api,
                {"failure": None, "tables": ["catalog", "consumption", "fingerprints"]},
                location,
                log,
                after=started,
            )
        )
        deadline: float = time.monotonic() + 45
        published: tuple[str, str] | None = None
        while published is None or datetime.fromisoformat(published[0]) <= synced:
            assert location.poll() is None, log.read_text()
            assert time.monotonic() < deadline, log.read_text()
            time.sleep(0.05)
            with closing(sqlite3.connect(control)) as database:
                published = database.execute(
                    "SELECT reported_at, report_json FROM sqb_changed_builds"
                ).fetchone()
        request_id: str = request_plan(control, selection, {"changed": True})
        result: dict[str, Any] = wait_for_location_work(api, request_id, location, log)
    outcome: dict[str, Any] = result["result_json"]["outcome"]
    assert outcome["kind"] == "plan", outcome
    return json.loads(published[1]), outcome["document"]


def plans_from_copy(
    tmp_path: Path,
    project: Path,
    requests: list[tuple[list[str], dict[str, object]]],
    fresh: frozenset[int] = frozenset(),
) -> list[dict[str, Any]]:
    """Serve `project` until its first sync completes, then plan each request from the copy.

    The requests at the `fresh` positions are Plan again, which plans anew instead of reusing.
    """
    control: Path = tmp_path / "control/.sqlbuild/control.sqlite3"
    log: Path = tmp_path / "location.log"
    started: datetime = datetime.now(UTC)
    documents: list[dict[str, Any]] = []
    with (
        snapshot_api(tmp_path / "control") as api,
        location_process(
            [
                str(Path(sys.executable).with_name("sqb")),
                "--project-dir",
                str(project),
                "location",
                "serve",
                "--server",
                api.url,
                "--location-id",
                api.location_id,
                "--target",
                "development",
            ],
            log,
            tmp_path / "location.stdout",
            env=os.environ | {"SQLBUILD_LOCATION_TOKEN": api.bearer},
        ) as location,
    ):
        _ = wait_for_generation(api, "development", 1, location, log)
        _ = wait_for_warehouse_sync_health(
            api,
            {"failure": None, "tables": ["catalog", "consumption", "fingerprints"]},
            location,
            log,
            after=started,
        )
        for position, (selection, settings) in enumerate(requests):
            request_id: str = request_plan(control, selection, settings, fresh=position in fresh)
            result: dict[str, Any] = wait_for_location_work(api, request_id, location, log)
            outcome: dict[str, Any] = result["result_json"]["outcome"]
            assert outcome["kind"] == "plan", outcome
            documents.append(outcome["document"])
    return documents


def changed_runs(plan: dict[str, Any]) -> tuple[str, ...]:
    """The nodes a changed plan runs."""
    return tuple(run["node"] for run in plan["changed"]["runs"])


def changed_build_report(plan: dict[str, Any]) -> dict[str, list[str]]:
    """What a changed build of `plan` reports: the nodes it runs, split by why they run."""
    runs: list[dict[str, Any]] = plan["changed"]["runs"]
    changed: list[dict[str, Any]] = list(filter(lambda run: run["downstream_of"] is None, runs))
    downstream: list[dict[str, Any]] = list(
        filter(lambda run: run["downstream_of"] is not None, runs)
    )
    return {
        "changed": [run["node"] for run in changed],
        "downstream": [run["node"] for run in downstream],
    }


def failed_load(tmp_path: Path, project: Path) -> tuple[dict[str, Any], int, str]:
    """Serve `project` until its first load fails: its health, snapshot API status and log."""
    log: Path = tmp_path / "location.log"
    with (
        snapshot_api(tmp_path / "control") as api,
        location_process(
            [
                str(Path(sys.executable).with_name("sqb")),
                "--project-dir",
                str(project),
                "location",
                "serve",
                "--server",
                api.url,
                "--location-id",
                api.location_id,
                "--target",
                "development",
            ],
            log,
            tmp_path / "location.stdout",
            env=os.environ | {"SQLBUILD_LOCATION_TOKEN": api.bearer},
        ) as location,
    ):
        report: dict[str, Any] = wait_for_location_health(
            api, "development", {"kind": "compile_failed"}, location, log
        )
        with pytest.raises(HTTPError) as missing:
            _ = current_snapshot(api, "development")
    return report["report"]["health"], missing.value.code, log.read_text()


def provider_secrets_health(tmp_path: Path, project: Path) -> tuple[dict[str, Any], str]:
    """Serve `project` until it reports which providers miss secrets: that health and the log."""
    log: Path = tmp_path / "location.log"
    with (
        snapshot_api(tmp_path / "control") as api,
        location_process(
            [
                str(Path(sys.executable).with_name("sqb")),
                "--project-dir",
                str(project),
                "location",
                "serve",
                "--server",
                api.url,
                "--location-id",
                api.location_id,
                "--target",
                "development",
            ],
            log,
            tmp_path / "location.stdout",
            env=os.environ | {"SQLBUILD_LOCATION_TOKEN": api.bearer},
        ) as location,
    ):
        report: dict[str, Any] = wait_for_location_health(
            api, "development", {"kind": "provider_secrets"}, location, log
        )
    return report["report"]["health"], log.read_text()


def plan_before_sync(
    tmp_path: Path,
    project: Path,
    settings: dict[str, object],
    selection: tuple[str, ...] = (),
) -> tuple[dict[str, Any], str]:
    """Serve `project` until it publishes, then plan from its copy without waiting for a sync."""
    control: Path = tmp_path / "control/.sqlbuild/control.sqlite3"
    log: Path = tmp_path / "location.log"
    with (
        snapshot_api(tmp_path / "control") as api,
        location_process(
            [
                str(Path(sys.executable).with_name("sqb")),
                "--project-dir",
                str(project),
                "location",
                "serve",
                "--server",
                api.url,
                "--location-id",
                api.location_id,
                "--target",
                "dev",
            ],
            log,
            tmp_path / "location.stdout",
            env=os.environ | {"SQLBUILD_LOCATION_TOKEN": api.bearer},
        ) as location,
    ):
        _ = wait_for_generation(api, "dev", 1, location, log)
        request_id: str = request_plan(control, list(selection), settings)
        result: dict[str, Any] = wait_for_location_work(api, request_id, location, log)
    return result["result_json"]["outcome"], log.read_text()


def development_environment(extra: dict[str, str], unset: frozenset[str]) -> dict[str, str]:
    """This environment with `extra` and `TERM=xterm`, less `unset` and the platform webhook."""
    environment: dict[str, str] = dict(os.environ) | {"TERM": "xterm"} | extra
    dropped: frozenset[str] = unset | {"SQLBUILD_PLATFORM_WEBHOOK_URL"}
    return dict(filter(lambda item: item[0] not in dropped, environment.items()))


def start_development_on_terminal(
    project: Path, arguments: list[str], environment: dict[str, str]
) -> tuple[int, subprocess.Popen[bytes]]:
    """`sqb dev` for `project` on a new 80x24 controlling terminal: its controller, the process."""
    terminal, attached = pty.openpty()
    termios.tcsetwinsize(attached, (24, 80))
    process: subprocess.Popen[bytes] = subprocess.Popen(
        [
            str(Path(sys.executable).with_name("sqb")),
            "dev",
            "--project-dir",
            str(project),
            "--port",
            "0",
            *arguments,
        ],
        stdin=attached,
        stdout=attached,
        stderr=attached,
        env=environment,
        start_new_session=True,
        preexec_fn=_take_terminal,
    )
    os.close(attached)
    return terminal, process


def _take_terminal() -> None:
    """In the child, after `setsid`: the terminal on stdin becomes the session's terminal."""
    fcntl.ioctl(0, termios.TIOCSCTTY, 0)


def drain_terminal(terminal: int) -> str:
    """What is left to read on `terminal` once its process has exited."""
    written: bytes = b""
    with suppress(OSError):
        while select.select([terminal], [], [], 0.2)[0] and (chunk := os.read(terminal, 65536)):
            written += chunk
    return written.decode(errors="replace")


def _command_line(process_directory: Path) -> str:
    """A `/proc/<pid>` directory's command line, empty once the process is gone."""
    with suppress(OSError):
        return (process_directory / "cmdline").read_bytes().replace(b"\0", b" ").decode()
    return ""


def processes_naming(project: Path) -> list[str]:
    """`<pid> <command line>` of every running process whose command line names `project`."""
    running: list[Path] = list(filter(lambda entry: entry.name.isdigit(), Path("/proc").iterdir()))
    commands: list[str] = [f"{entry.name} {_command_line(entry)}" for entry in running]
    return list(filter(lambda command: str(project) in command, commands))


def reap_development(process: subprocess.Popen[bytes], project: Path) -> list[str]:
    """Kill `process` if it still runs, then any process left naming `project`; return those."""
    process.kill()
    _ = process.wait(timeout=5)
    time.sleep(0.5)
    left: list[str] = processes_naming(project)
    for line in left:
        with suppress(ProcessLookupError):
            os.kill(int(line.split()[0]), signal.SIGKILL)
    return left


def statement_started(control: Path) -> bool:
    """Whether a worker has sent the endless `range(10000000000)` statement to the warehouse."""
    with sqlite3.connect(control) as store:
        return (
            store.execute(
                "SELECT 1 FROM recall_artifacts WHERE CAST(content AS TEXT) LIKE ?",
                ("%range(10000000000)%",),
            ).fetchone()
            is not None
        )


# DuckDB whose statement cancellation leaves a mark, so a test sees the cancel path run.
CANCEL_MARKING_ADAPTER: str = (
    "import os\nfrom pathlib import Path\n"
    "from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter\n\n\n"
    "class DuckDbCancelMarkingAdapter(DuckDbAdapter):\n"
    "    adapter_name = 'duckdb_cancel_marking'\n\n"
    "    def interrupt_connection(self, connection):\n"
    "        with Path(os.environ['ORDERS_CANCELS']).open('a') as cancels:\n"
    "            cancels.write(f'{os.getpid()}\\n')\n"
    "        return super().interrupt_connection(connection)\n"
)

# A model whose one statement runs until it is cancelled.
ENDLESS_ORDERS: str = (
    "MODEL (description 'Typed orders.',\n"
    "  columns (order_id (type INTEGER), amount (type BIGINT)),\n"
    ");\n\n"
    "SELECT CAST(1 AS INTEGER) AS order_id, CAST(sum(sin(value)) AS BIGINT) AS amount\n"
    "FROM range(10000000000) t(value)\n"
)


def endless_build_files(project_toml: str) -> dict[str, str]:
    """A dev project whose job runs one endless statement on a DuckDB that marks its cancels."""
    return PUBLISHABLE_FILES | {
        "sqlbuild_project.toml": project_toml.replace(
            'adapter = "duckdb"', 'adapter = "duckdb_cancel_marking"'
        ),
        "python/adapters/duckdb_cancel_marking.py": CANCEL_MARKING_ADAPTER,
        "sql/models/orders.sql": ENDLESS_ORDERS,
        **duckshop_orchestration("development_orders"),
    }


def request_development_run(project: Path, environment: dict[str, str]) -> tuple[Path, str]:
    """Queue `annual_orders_job` in `project`'s running `sqb dev`: its control store and the run."""
    control: Path = project / ".sqlbuild/control.sqlite3"
    run_id: str = str(uuid4())
    with closing(sqlite3.connect(control)) as store:
        (location_id,) = store.execute(
            "SELECT location_id FROM sqb_development_locations"
        ).fetchone()
    requested: subprocess.CompletedProcess[str] = subprocess.run(
        [
            str(Path(sys.executable).with_name("sqb")),
            "--project-dir",
            str(project),
            "admin",
            "request-run",
            str(location_id),
            "annual_orders_job",
            "--target",
            "development",
            "--run-id",
            run_id,
            "--reason",
            "A build to stop",
        ],
        env=environment | {"SQLBUILD_CONTROL_DATABASE_URL": str(control)},
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    assert requested.returncode == 0, requested.stderr
    return control, run_id


def development_run_state(control: Path, run_id: str) -> tuple[str, list[str]]:
    """The run's status and the nodes it still holds write claims on."""
    with closing(sqlite3.connect(control)) as store:
        (status,) = store.execute(
            "SELECT status FROM sqb_runs WHERE run_id = ?", (run_id,)
        ).fetchone()
        claims: list[str] = [
            node
            for (node,) in store.execute(
                "SELECT node_key FROM sqb_write_claims WHERE run_id = ?", (run_id,)
            ).fetchall()
        ]
    return status, claims


def uncooperative_worker_pid(worker: ScheduledWorker) -> int:
    """The process ID the uncooperative launcher's worker writes once it has started."""
    pidfile: Path = worker.project / "worker.pid"
    deadline: float = time.monotonic() + 15
    while not pidfile.exists():
        assert worker.process.poll() is None, worker.log.read_text()
        assert time.monotonic() < deadline, worker.log.read_text()
        time.sleep(0.02)
    return int(pidfile.read_text())


def request_identified_run(
    executable: str, project: Path, identity: str, environment: dict[str, str]
) -> str:
    """Request a run of `annual_orders_job` from the development store; its run ID."""
    run_id: str = str(uuid4())
    requested: subprocess.CompletedProcess[str] = subprocess.run(
        [
            executable,
            "--project-dir",
            str(project),
            "admin",
            "request-run",
            identity,
            "annual_orders_job",
            "--target",
            "development",
            "--run-id",
            run_id,
            "--reason",
            "Development secret reload",
        ],
        env=environment,
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )
    assert requested.returncode == 0, requested.stderr
    return run_id


def linked_project(project: Path) -> Path:
    """A symlink to the project, beside it."""
    link: Path = project.with_name(f"{project.name}_link")
    link.symlink_to(project, target_is_directory=True)
    return link


def latest_redraw(shown: str) -> str:
    """The rows the live screen drew last, uncoloured: everything after its last clear."""
    return re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", shown.rsplit("\x1b[J", 1)[-1])


def worker_printed(control_store: Path, run_id: str) -> tuple[str, frozenset[str]]:
    """What the run's workers printed, both streams in order, and their Recall event types."""
    invocations: str = (
        "SELECT invocation_id FROM recall_invocations WHERE invocation_id LIKE 'worker/%' "
        "AND json_extract(external_context, '$.run_id')=?"
    )
    with sqlite3.connect(control_store) as control:
        printed: str = "".join(
            data
            for (data,) in control.execute(
                "SELECT CAST(data AS TEXT) FROM recall_output "
                f"WHERE invocation_id IN ({invocations}) ORDER BY occurred_at, sequence",
                (run_id,),
            ).fetchall()
        )
        events: frozenset[str] = frozenset(
            event_type
            for (event_type,) in control.execute(
                "SELECT DISTINCT event_type FROM recall_events "
                f"WHERE invocation_id IN ({invocations})",
                (run_id,),
            ).fetchall()
        )
    return printed, events


def launch_adhoc(worker: ScheduledWorker, command: list[str]) -> dict[str, Any]:
    """Launch `command` as an admin's ad-hoc run on `worker`'s location: the run once it ends."""
    admin: dict[str, Any] = provision_person(
        worker.control_store.parent.parent, "create-user", "orders-admin"
    )
    session: tuple[str, str] = person_login(worker.api, admin["name"])
    run_id: str = str(uuid4())
    assert person_request(
        worker.api,
        "/api/control/runs/adhoc",
        body={
            "location_id": worker.api.location_id,
            "target": "development",
            "run_id": run_id,
            "command": command,
            "reason": "Check the worker's output",
            "priority": 0,
        },
        session=session,
    ) == {"applied": True}
    return wait_for_run(worker.api, worker.process, worker.log, run_id=run_id)


# A loader after the items tagged `fulfilment`, which a `+raw_steps` reload runs as ingress.
INGRESS_JOB_SETTINGS: str = "\n[settings]\nreload = true\n"
INGRESS_LOADER_FILES: dict[str, str] = {
    "python/nodes/ready/asset.py": (
        "from sqlbuild.assets import asset\n"
        "from sqlbuild.refs import tag\n\n\n"
        '@asset(depends_on=tag("fulfilment"))\n'
        "def ready(ctx):\n"
        '    """Wait for every step."""\n'
    ),
    "python/nodes/loads/fetch_steps/loader.py": (
        "from sqlbuild.loaders import loader\n"
        "from python.nodes.ready.asset import ready\n"
        "\n"
        "\n"
        "@loader(depends_on=[ready], write_strategy='replace', "
        "columns=[{'name': 'step', 'type': 'INTEGER'}])\n"
        "def fetch_steps(ctx):\n"
        '    """Fetch the steps."""\n'
        '    return [{"step": 1}]\n'
    ),
    "python/nodes/loads/raw_steps/loader.py": (
        "from sqlbuild.loaders import loader\n"
        "from python.nodes.ready.asset import ready\n"
        "from python.nodes.loads.fetch_steps.loader import fetch_steps\n"
        "\n"
        "\n"
        "@loader(write_strategy='replace', depends_on=[fetch_steps])\n"
        "def raw_steps(ctx):\n"
        '    """Load the steps."""\n'
        '    return [{"step": 1}]\n'
    ),
    "python/nodes/loads/__init__.py": (
        "from .fetch_steps.loader import fetch_steps\nfrom .raw_steps.loader import raw_steps\n"
    ),
    "sql/sources/raw.yml": (
        "sources:\n  - name: raw_steps\n    description: Steps.\n    managed: true\n"
    ),
}


@contextmanager
def outage_server(directory: Path, database_url: str) -> Iterator[tuple[str, Path]]:
    """`sqb server` over a control database it may not reach yet, from the moment it listens."""
    directory.mkdir(parents=True)
    log: Path = directory / "server.log"
    environment: dict[str, str] = os.environ.copy()
    environment.pop("SQLBUILD_PLATFORM_WEBHOOK_URL", None)
    environment.pop("SQLBUILD_SECURITY_AUDIT_WEBHOOK_URL", None)
    environment["XDG_STATE_HOME"] = str(directory / "state")
    environment["SQLBUILD_CONTROL_DATABASE_URL"] = database_url
    with log.open("wb") as stderr:
        process: subprocess.Popen[bytes] = subprocess.Popen(
            [str(Path(sys.executable).with_name("sqb")), "server", "--port", "0", "--no-scheduler"],
            cwd=directory,
            env=environment,
            stdout=subprocess.DEVNULL,
            stderr=stderr,
        )
        try:
            yield listening_address(process, log), log
        finally:
            process.terminate()
            try:
                _ = process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                _ = process.wait(timeout=5)


def listening_address(process: subprocess.Popen[bytes], log: Path) -> str:
    """The address a server answers on before its control database is open."""
    deadline: float = time.monotonic() + 10
    while "Control API listening: " not in log.read_text():
        assert process.poll() is None, log.read_text()
        assert time.monotonic() < deadline, "the server did not start listening"
        time.sleep(0.02)
    return "http://" + log.read_text().partition("Control API listening: ")[2].splitlines()[0]


def http_reply(url: str) -> tuple[int, dict[str, str], str]:
    """A GET's status, headers and body text, whatever the status."""
    try:
        with urlopen(url, timeout=10) as response:
            return (
                response.status,
                {key.lower(): value for key, value in response.headers.items()},
                response.read().decode(),
            )
    except HTTPError as error:
        return (
            error.code,
            {key.lower(): value for key, value in error.headers.items()},
            error.read().decode(),
        )


def health_report(url: str) -> dict[str, Any]:
    """The server's health report, which it answers with 200 or 503."""
    report: dict[str, Any] = json.loads(http_reply(f"{url}/api/health")[2])
    return report


def wait_for_health(
    url: str, wanted: Callable[[dict[str, Any]], bool], seconds: float
) -> dict[str, Any]:
    """The first health report `wanted` accepts, read every half second."""
    deadline: float = time.monotonic() + seconds
    report: dict[str, Any] = health_report(url)
    while not wanted(report) and time.monotonic() < deadline:
        time.sleep(0.5)
        report = health_report(url)
    assert wanted(report), f"health never became what was wanted: {report}"
    return report


def control_database_row(report: dict[str, Any]) -> dict[str, Any]:
    """The control database's row of a health report, by what anyone may see of it."""
    rows: dict[str, dict[str, Any]] = {row["component"]: row for row in report["components"]}
    row: dict[str, Any] = rows["control_database"]
    return {"status": row["status"], "reason": row["reason"], "case": row.get("case")}


def free_port() -> int:
    """A local port nothing listens on yet."""
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        port: int = probe.getsockname()[1]
    return port


@contextmanager
def forwarded_port(port: int, target: tuple[str, int]) -> Iterator[None]:
    """Accept on `port` and relay every connection to `target` until the block ends."""
    listener: socket.socket = socket.create_server(("127.0.0.1", port))
    stop: threading.Event = threading.Event()

    def relay(source: socket.socket, sink: socket.socket) -> None:
        with suppress(OSError):
            while chunk := source.recv(65536):
                sink.sendall(chunk)
        with suppress(OSError):
            sink.shutdown(socket.SHUT_WR)

    def accept() -> None:
        listener.settimeout(0.2)
        while not stop.is_set():
            try:
                client, _ = listener.accept()
            except TimeoutError:
                continue
            upstream: socket.socket = socket.create_connection(target)
            threading.Thread(target=relay, args=(client, upstream), daemon=True).start()
            threading.Thread(target=relay, args=(upstream, client), daemon=True).start()

    acceptor: threading.Thread = threading.Thread(target=accept, daemon=True)
    acceptor.start()
    try:
        yield
    finally:
        stop.set()
        acceptor.join(timeout=2)
        listener.close()


def runtime_sink_totals(
    project: Path, settled: Callable[[dict[str, Any]], bool], seconds: float
) -> dict[str, Any]:
    """The location interpreter's sink totals, asked every half second until `settled`."""
    child: subprocess.Popen[str] = subprocess.Popen(
        [
            str(Path(sys.executable).with_name("sqb")),
            "--project-dir",
            str(project),
            "location",
            "runtime",
            "--server",
            "http://127.0.0.1:9",
            "--location-id",
            str(uuid4()),
            "--target",
            "development",
        ],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        cwd=project,
    )
    stdin, stdout = cast(Any, child.stdin), cast(Any, child.stdout)

    def ask() -> dict[str, Any]:
        stdin.write('{"sink_deliveries": {}}\n')
        stdin.flush()
        reply: dict[str, Any] = json.loads(stdout.readline())
        return reply

    deadline: float = time.monotonic() + seconds
    totals: dict[str, Any] = ask()
    while not settled(totals) and time.monotonic() < deadline:
        time.sleep(0.5)
        totals = ask()
    stdin.close()
    child.wait(timeout=60)
    return totals
