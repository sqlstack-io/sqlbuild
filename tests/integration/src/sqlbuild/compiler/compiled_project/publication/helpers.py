"""A standalone native API and real SQLite store, with test-only credential provisioning."""

from __future__ import annotations

import hashlib
import itertools
import json
import os
import random
import secrets
import signal
import sqlite3
import subprocess
import sys
import time
from collections.abc import Callable, Iterator
from contextlib import AbstractContextManager, closing, contextmanager, nullcontext, suppress
from dataclasses import replace
from datetime import UTC, datetime, timedelta
from io import StringIO
from pathlib import Path
from typing import Any, Literal, cast
from urllib.error import HTTPError
from urllib.parse import quote
from urllib.request import Request, urlopen
from uuid import uuid4

import duckdb
import pytest

from sqlbuild._native import (
    Fingerprint,
    MigrationDecision,
    MigrationDiscovery,
    MigrationEvent,
    MigrationRelation,
    OldNameViewEvent,
    OldNameViewEventType,
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
from sqlbuild.microbatches.classes.direct_store import DirectMicrobatchEventStore
from sqlbuild.microbatches.classes.event_codec import MicrobatchEventCodec
from sqlbuild.microbatches.constants import MICROBATCH_COLUMNS
from sqlbuild.microbatches.models import MicrobatchEvent, MicrobatchWriteResult
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    ClaimedWorker,
    ScheduledWorker,
    SnapshotApi,
    WorkerDeployment,
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.docker.helpers import (
    docker_deployment,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files
from tests.integration.src.sqlbuild.executor.audit_results.main.helpers import build_record


class RejectedPublicationOutput(StringIO):
    """A caller loses only the result stream, after the real API accepts a publication."""

    def write(self, text: str) -> int:
        raise OSError("synthetic publication result delivery failure")


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


def stop_location(process: subprocess.Popen[bytes], interpreter_pid: int, log: Path) -> None:
    """Exercise graceful supervisor shutdown and prove its interpreter was reaped."""
    process.send_signal(signal.SIGTERM)
    assert process.wait(timeout=15) == 0, log.read_text()
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
            "orchestration/orders.py": 'raise AssertionError("unexpected declaration import")\n',
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


def migration_history_data(fixture: dict[str, Any], relations: tuple[str, str]) -> dict[str, Any]:
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
    record: MigrationEvent | OldNameViewEvent,
    kind: Literal["model", "old_name"],
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
) -> Any:
    """Exercise the real isolated reader without a server, state database or declaration import."""
    result: subprocess.CompletedProcess[str] = subprocess.run(
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
        input=json.dumps(
            {
                table: {
                    "location": {"database": None, "schema": "main"},
                    "after": after,
                    "observed_since": observed_since,
                }
            }
        )
        + "\n",
        text=True,
        capture_output=True,
        timeout=20,
        env=os.environ | {"SQLBUILD_CONTROL_DATABASE_URL": "unsupported://must-not-open"},
    )
    assert result.returncode == 0, result.stderr
    return json.loads(result.stdout)


def warehouse_history_pages(
    project: Path, table: str, observed_since: str | None = None
) -> list[dict[str, Any]]:
    """Follow bounded source pages, including empty filtered pages with a continuation."""
    pages: list[dict[str, Any]] = []
    after: list[str | None] | None = None
    more: bool = True
    while more:
        page: dict[str, Any] = warehouse_history_page(project, table, after, observed_since)
        assert page is not None
        pages.append(page)
        after = page["next_after"]
        more = after is not None
        assert len(pages) <= 4, "history cursor must advance past duplicate physical rows"
    return pages


def sync_warehouse_history(api: SnapshotApi, table: str, batch: dict[str, Any]) -> dict[str, Any]:
    """Use the public location transport and return its bounded reconciliation continuation."""
    request: Request = Request(
        f"{api.url}/api/locations/{api.location_id}/targets/development/metadata/{table}",
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
        assert 0 <= health["records_sent"] <= 100
        assert health["elapsed_millis"] > 0
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
    deadline: float = time.monotonic() + 15 + 30 * (minimum_reports - 1)
    key: tuple[str, str] = (target, json.dumps(health, sort_keys=True))
    observations: dict[tuple[str, str], dict[str, dict[str, Any]]] = {}
    while len(observations.get(key, ())) < minimum_reports:
        assert process.poll() is None, log.read_text()
        assert time.monotonic() < deadline, log.read_text()
        for report in reversed(location_health_reports(api)):
            event_id: str = report["report"]["event_id"]
            observed_key: tuple[str, str] = (
                report["report"]["target"],
                json.dumps(report["report"]["health"], sort_keys=True),
            )
            observations.setdefault(observed_key, {})[event_id] = report
        time.sleep(0.02)
    return list(observations[key].values())[-1]


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


def api_address(process: subprocess.Popen[bytes], log: Path) -> str:
    """Wait for bounded native startup, never publishing an unready address."""
    deadline: float = time.monotonic() + 10
    while "Control API ready: " not in log.read_text():
        assert process.poll() is None, log.read_text()
        assert time.monotonic() < deadline, "Native API did not become ready"
        time.sleep(0.02)
    return "http://" + log.read_text().partition("Control API ready: ")[2].splitlines()[0]


@contextmanager
def snapshot_api(
    directory: Path, *, server_arguments: tuple[str, ...] = ()
) -> Iterator[SnapshotApi]:
    """Only this fixture seeds the control database; the code location uses HTTP exclusively."""
    directory.mkdir()
    log: Path = directory / "server.log"
    environment: dict[str, str] = os.environ.copy()
    environment.pop("SQLBUILD_CONTROL_DATABASE_URL", None)
    environment.pop("SQLBUILD_PLATFORM_WEBHOOK_URL", None)
    environment.pop("SQLBUILD_SECURITY_AUDIT_WEBHOOK_URL", None)
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
            with sqlite3.connect(directory / ".sqlbuild/control.sqlite3") as connection:
                _ = connection.execute(
                    "INSERT INTO sqb_locations (location_id,name,created_by,created_at) "
                    "VALUES (?, 'orders', 'test-admin', ?)",
                    (location_id, now.isoformat()),
                )
                _ = connection.execute(
                    "INSERT INTO sqb_access_tokens "
                    "(token_id,secret_hash,location_id,scope_json,created_at,expires_at) "
                    "VALUES (?,?,?,?,?,?)",
                    (
                        token_id,
                        hashlib.sha256(bearer.encode()).hexdigest(),
                        location_id,
                        json.dumps({"kind": "location", "location_id": location_id}),
                        now.isoformat(),
                        (now + timedelta(hours=1)).isoformat(),
                    ),
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
            ),
            "models/orders.sql": (
                "MODEL (description 'Orders.', materialized table);\n" + test_case.model_sql + "\n"
            ),
            "orchestration/orders.py": (
                "import os\nfrom sqlbuild.orchestration import job, schedule\n"
                "if 'SQLBUILD_RUN_TOKEN' in os.environ:\n"
                "    assert 'SQLBUILD_PLATFORM_WEBHOOK_URL' not in os.environ\n"
                "    assert 'SQLBUILD_SECURITY_AUDIT_WEBHOOK_URL' not in os.environ\n"
                'annual_orders = job("annual_orders", select="orders", '
                'description="Build orders.", '
                'schedule=schedule(cron="0 0 * * *", timezone="UTC", '
                'starts_at="2000-01-01T00:00:00Z"))\n'
            ),
        }
        | test_case.files,
    )
    executable: str = str(Path(sys.executable).with_name("sqb"))
    launchers: dict[str, Callable[[Path, str], AbstractContextManager[WorkerDeployment]]] = {
        "native": native_launcher,
        "docker": docker_deployment,
        "uncooperative": uncooperative_launcher,
    }
    with (
        snapshot_api(directory / "control", server_arguments=server_arguments) as api,
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
        configuration: Path = directory / "launchers.json"
        launcher: dict[str, Any] = {
            "location_id": api.location_id,
            "target": "development",
            "server_url": api.url,
            "max_workers": 1,
        } | deployment.configuration
        _ = configuration.write_text(
            json.dumps(
                {
                    deployment.kind: [launcher],
                    "concurrency_limits": test_case.concurrency_limits,
                }
            )
        )
        control_store: Path = directory / "control/.sqlbuild/control.sqlite3"
        with sqlite3.connect(control_store) as control:
            _ = control.executescript(test_case.store_setup)
        monkeypatch.setenv("SQLBUILD_SCHEDULER_TEST_STORE", f"sqlite://{control_store}")
        monkeypatch.setenv(
            "SQLBUILD_PLATFORM_WEBHOOK_URL",
            "http://127.0.0.1:1/events?token=synthetic-delivery-credential",
        )
        monkeypatch.setenv(
            "SQLBUILD_SECURITY_AUDIT_WEBHOOK_URL",
            "http://127.0.0.1:1/audit?token=synthetic-audit-credential",
        )
        log: Path = directory / "scheduler.log"
        with location_process(
            [
                executable,
                "scheduler",
                "--database-env",
                "SQLBUILD_SCHEDULER_TEST_STORE",
                "--launchers",
                str(configuration),
            ],
            log,
            directory / "scheduler.stdout",
        ) as process:
            yield ScheduledWorker(api, project, control_store, process, log, deployment.docker)


def worker_result(worker: ScheduledWorker, test_case: WorkerExecutionTestCase) -> dict[str, Any]:
    """Run/node outcomes, Recall, claims and the physical warehouse agree."""
    api, project, control_store, process, log, _ = worker
    run: dict[str, Any] = wait_for_run(api, process, log)
    assert run["node_counts_json"] == {test_case.expected_status: test_case.expected_node_count}, (
        log.read_text()
    )
    assert run["job_key"] == "annual_orders"
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
        "concurrency_groups": [],
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
            "INSERT INTO sqb_run_nodes (run_id,node_key,attempt,status,writes) "
            "VALUES (?,'model:orders',1,'pending',TRUE)",
            (run_id,),
        )
        _ = connection.execute(
            "INSERT INTO sqb_write_claims (target,node_key,run_id,claimed_at) VALUES (?,?,?,?)",
            (publication["target"], "model:orders", run_id, now.isoformat()),
        )
        _ = connection.execute(
            "UPDATE sqb_runs SET worker_instance=?, claimed_at=?, last_heartbeat_at=? "
            "WHERE run_id=?",
            (claim_id, now.isoformat(), now.isoformat(), run_id),
        )
        _ = connection.execute(
            "INSERT INTO sqb_access_tokens "
            "(token_id,secret_hash,run_id,claim_id,scope_json,created_at,expires_at) "
            "VALUES (?,?,?,?,?,?,?)",
            (
                token_id,
                hashlib.sha256(bearer.encode()).hexdigest(),
                run_id,
                claim_id,
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
                "ORDER BY storage_id DESC LIMIT 1",
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
    """SIGKILL real schedulers while they drain a backlog; return breaches and the kill count."""
    project: Path = directory / "project"
    start: datetime = datetime.now(UTC).replace(second=0, microsecond=0) - timedelta(
        minutes=fixture["backlog_minutes"]
    )
    declarations: str = "".join(
        f'{name} = job("{name}", select="orders", description="Build {name}.", '
        f'schedule=schedule(cron="* * * * *", timezone="UTC"))\n'
        for name in fixture["jobs"]
    )
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": (
                'name = "orders"\nadapter = "duckdb"\n'
                '[connections.local]\ndatabase = "warehouse.duckdb"\n'
                '[targets.development]\nschema = "main"\n'
            ),
            "models/orders.sql": (
                "MODEL (description 'Orders.', materialized table);\nSELECT 1 AS order_id\n"
            ),
            "orchestration/orders.py": "from sqlbuild.orchestration import job, schedule\n"
            + declarations,
        },
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
    minutes: list[datetime] = [
        start + timedelta(minutes=offset) for offset in range(fixture["backlog_minutes"] + 1)
    ]
    expected: set[tuple[str, datetime]] = set(
        itertools.product({tick[0] for tick in ticks}, minutes)
    )
    observed: list[tuple[str, datetime]] = [
        (trigger, datetime.fromisoformat(scheduled)) for trigger, scheduled, _, _ in ticks
    ]
    requesting: list[str] = list(filter(None, (run for _, _, run, _ in ticks)))
    covering: set[str] = set(filter(None, (covered for _, _, _, covered in ticks)))
    return {
        "triggers_without_ticks": triggers - len({tick[0] for tick in ticks}),
        "duplicate_ticks": len(observed) - len(set(observed)),
        "missing_ticks": len(expected - set(observed)),
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
