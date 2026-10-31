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
from pathlib import Path
from typing import Any
from urllib.parse import quote
from urllib.request import Request, urlopen
from uuid import uuid4

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    SnapshotApi,
)


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
