"""A standalone native API and real SQLite store, with test-only credential provisioning."""

from __future__ import annotations

import hashlib
import json
import os
import secrets
import signal
import sqlite3
import subprocess
import sys
import time
from collections.abc import Callable, Iterator
from contextlib import AbstractContextManager, contextmanager, nullcontext, suppress
from datetime import UTC, datetime, timedelta
from io import StringIO
from pathlib import Path
from typing import Any
from urllib.error import HTTPError
from urllib.parse import quote
from urllib.request import Request, urlopen
from uuid import uuid4

import duckdb
import pytest

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


class RejectedPublicationOutput(StringIO):
    """A caller loses only the result stream, after the real API accepts a publication."""

    def write(self, text: str) -> int:
        raise OSError("synthetic publication result delivery failure")


@contextmanager
def location_process(
    arguments: list[str], log: Path, publications: Path
) -> Iterator[subprocess.Popen[bytes]]:
    """Own and reap the supervisor even when an assertion interrupts a reload."""
    with log.open("ab") as stderr, publications.open("ab") as stdout:
        process: subprocess.Popen[bytes] = subprocess.Popen(arguments, stderr=stderr, stdout=stdout)
        try:
            yield process
        finally:
            process.terminate()
            try:
                _ = process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                _ = process.wait(timeout=5)


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
    request: Request = Request(
        f"{api.url}/api/locations/{api.location_id}/health",
        headers={"Authorization": f"Bearer {api.bearer}"},
    )
    key: tuple[str, str] = (target, json.dumps(health, sort_keys=True))
    observations: dict[tuple[str, str], dict[str, dict[str, Any]]] = {}
    while len(observations.get(key, ())) < minimum_reports:
        assert process.poll() is None, log.read_text()
        assert time.monotonic() < deadline, log.read_text()
        with urlopen(request, timeout=2) as response:
            reports: list[dict[str, Any]] = json.load(response)
        for report in reversed(reports):
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
def snapshot_api(directory: Path) -> Iterator[SnapshotApi]:
    """Only this fixture seeds the control database; the code location uses HTTP exclusively."""
    directory.mkdir()
    log: Path = directory / "server.log"
    environment: dict[str, str] = os.environ.copy()
    environment.pop("SQLBUILD_CONTROL_DATABASE_URL", None)
    with log.open("wb") as stderr:
        process: subprocess.Popen[bytes] = subprocess.Popen(
            [str(Path(sys.executable).with_name("sqb")), "server", "--port", "0", "--no-scheduler"],
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
    directory: Path, monkeypatch: pytest.MonkeyPatch, test_case: WorkerExecutionTestCase
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
                "from sqlbuild.orchestration import job, schedule\n"
                'annual_orders = job("annual_orders", select="orders", '
                'description="Build orders.", '
                'schedule=schedule(cron="0 0 1 1 *", timezone="UTC", '
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
        snapshot_api(directory / "control") as api,
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
        _ = configuration.write_text(json.dumps({deployment.kind: [launcher]}))
        control_store: Path = directory / "control/.sqlbuild/control.sqlite3"
        with sqlite3.connect(control_store) as control:
            _ = control.executescript(test_case.store_setup)
        monkeypatch.setenv("SQLBUILD_SCHEDULER_TEST_STORE", f"sqlite://{control_store}")
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
            "SELECT count(*) FROM recall_events "
            "WHERE invocation_id=? AND event_type='run_launched'",
            (f"scheduler/{run['run_id']}",),
        ).fetchone() == (1,)
        contexts: list[tuple[str | None]] = control.execute(
            "SELECT external_context FROM recall_invocations WHERE command = 'location'"
        ).fetchall()
        assert contexts, log.read_text()
        assert all(
            context is not None and json.loads(context)["run_id"] == run["run_id"]
            for (context,) in contexts
        )
        assert control.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (0,)
    with duckdb.connect(str(project / "warehouse.duckdb"), read_only=True) as warehouse:
        assert warehouse.sql(test_case.warehouse_query).fetchall() == test_case.expected_rows
    return run


def running_statement(worker: ScheduledWorker) -> dict[str, Any]:
    """The worker has started the real long query, not merely entered its Python interpreter."""
    active: dict[str, Any] = wait_for_run(
        worker.api, worker.process, worker.log, pending=frozenset({"queued", "launched"})
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


def cancel_running_statement(worker: ScheduledWorker) -> None:
    """Wait for the real long-running query before exercising the administrative cancel command."""
    active: dict[str, Any] = running_statement(worker)
    cancelled: subprocess.CompletedProcess[str] = subprocess.run(
        [
            str(Path(sys.executable).with_name("sqb")),
            "admin",
            "--database-env",
            "SQLBUILD_SCHEDULER_TEST_STORE",
            "cancel-run",
            active["run_id"],
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
) -> dict[str, Any]:
    """A terminal API projection must follow the scheduler's actual isolated warehouse execution."""
    request: Request = Request(
        f"{api.url}/api/locations/{api.location_id}/targets/development/runs",
        headers={"Authorization": f"Bearer {api.bearer}"},
    )
    deadline: float = time.monotonic() + 30
    current: dict[str, Any] = {"status": "queued"}
    while current["status"] in pending:
        assert process.poll() is None, log.read_text()
        assert time.monotonic() < deadline, log.read_text()
        with urlopen(request, timeout=2) as response:
            current = next(iter(json.load(response)), current)
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
        "overlap": "allow",
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
            "INSERT INTO sqb_run_liveness (run_id,worker_instance,claimed_at,last_heartbeat_at) "
            "VALUES (?,?,?,?)",
            (run_id, claim_id, now.isoformat(), now.isoformat()),
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
