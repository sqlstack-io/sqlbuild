"""A standalone native API and real SQLite store, with test-only credential provisioning."""

from __future__ import annotations

import hashlib
import json
import os
import secrets
import sqlite3
import subprocess
import sys
import time
from collections.abc import Iterator
from contextlib import contextmanager
from datetime import UTC, datetime, timedelta
from io import StringIO
from pathlib import Path
from typing import Any
from urllib.parse import quote
from urllib.request import Request, urlopen
from uuid import uuid4

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    ClaimedWorker,
    SnapshotApi,
)


class RejectedPublicationOutput(StringIO):
    """A caller loses only the result stream, after the real API accepts a publication."""

    def write(self, text: str) -> int:
        raise OSError("synthetic publication result delivery failure")


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
