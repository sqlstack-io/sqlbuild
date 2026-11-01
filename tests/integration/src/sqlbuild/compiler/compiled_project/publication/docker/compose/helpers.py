"""Own a disposable packaged PostgreSQL/HTTP/Docker stack without logging credentials."""

from __future__ import annotations

import json
import os
import secrets
import shutil
import subprocess
import time
from collections.abc import Iterator
from contextlib import contextmanager
from datetime import UTC, datetime, timedelta
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from threading import Thread
from typing import Any
from uuid import uuid4


class ComposeStack:
    """One test owns its containers, images, PostgreSQL volume, and warehouse volume."""

    def __init__(self, directory: Path) -> None:
        self.directory = directory
        self.name = f"sqlbuild-compose-{uuid4().hex}"
        self.location_id = "00000000-0000-0000-0000-000000000001"
        password: str = secrets.token_hex(24)
        self.environment = os.environ | {
            "COMPOSE_PROJECT_NAME": self.name,
            "SQLBUILD_PORT": "0",
            "DOCKER_GID": str(Path("/var/run/docker.sock").stat().st_gid),
            "POSTGRES_PASSWORD": password,
            "SQLBUILD_CONTROL_DATABASE_URL": f"postgresql://sqlbuild:{password}@postgres:5432/sqlbuild",
            "SQLBUILD_SERVER_IMAGE": os.environ.get(
                "SQLBUILD_TEST_SERVER_IMAGE", "sqlbuild-server:test"
            ),
            "SQLBUILD_LOCATION_IMAGE": f"{self.name}:first",
            "SQLBUILD_LOCATION_ID": self.location_id,
            "SQLBUILD_GIT_REVISION": "compose-acceptance",
        }
        self.override = directory / "override.json"
        self.override.write_text(
            json.dumps(
                {"services": {"server": {"extra_hosts": ["host.docker.internal:host-gateway"]}}}
            )
        )

    def docker(
        self, *arguments: str, check: bool = True, timeout: float = 120
    ) -> subprocess.CompletedProcess[str]:
        started: float = time.monotonic()
        result: subprocess.CompletedProcess[str] = subprocess.run(
            ["docker", *arguments],
            env=self.environment,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
        with (self.directory / "commands.jsonl").open("a") as log:
            log.write(
                json.dumps(
                    {
                        "args": arguments,
                        "exit": result.returncode,
                        "seconds": time.monotonic() - started,
                    }
                )
                + "\n"
            )
        assert not check or result.returncode == 0, result.stderr
        return result

    def compose(self, *arguments: str, check: bool = True) -> str:
        return self.docker(
            "compose",
            "--file",
            str(Path("compose.yml").resolve()),
            "--file",
            str(self.override),
            *arguments,
            check=check,
        ).stdout.strip()

    def query(self, sql: str) -> list[dict[str, Any]]:
        result: str = self.compose(
            "exec",
            "-T",
            "postgres",
            "psql",
            "-U",
            "sqlbuild",
            "-d",
            "sqlbuild",
            "-At",
            "-c",
            f"SELECT COALESCE(json_agg(result),'[]'::json)::text FROM ({sql}) result",
        )
        return json.loads(result)

    def wait(self, sql: str, *, timeout: float = 150) -> list[dict[str, Any]]:
        deadline: float = time.monotonic() + timeout
        rows: list[dict[str, Any]] = self.query(sql)
        while not rows:
            assert time.monotonic() < deadline, f"Timed out: {sql} (logs in {self.directory})"
            time.sleep(0.5)
            rows = self.query(sql)
        with (self.directory / "observations.jsonl").open("a") as log:
            log.write(
                json.dumps({"at": datetime.now(UTC).isoformat(), "query": sql, "rows": rows}) + "\n"
            )
        return rows

    def build_projects(self) -> list[str]:
        cases: dict[str, str] = json.loads(
            Path(
                "benchmarks/duckshop/edge_cases/orchestration/container_deployment.json"
            ).read_text()
        )
        base: str = os.environ.get("SQLBUILD_TEST_LOCATION_IMAGE", "sqlbuild-location:test")
        images: list[str] = []
        for version, sql in cases.items():
            project: Path = self.directory / version
            (project / "models").mkdir(parents=True)
            (project / "orchestration").mkdir()
            for relative in ("sqlbuild_project.toml", "orchestration/orders.py"):
                shutil.copyfile(Path("deploy/project") / relative, project / relative)
            (project / "models/orders.sql").write_text(
                "MODEL (description 'Example orders.', materialized table);\n" + sql + "\n"
            )
            # Only source enters this build context; credentials stay in the parent process.
            (project / "Dockerfile").write_text(
                f"FROM {base}\nENV SQLBUILD_GIT_REVISION={version}\n"
                "COPY --chown=10001:10001 . /project/\n"
            )
            self.docker("build", "--tag", f"{self.name}:{version}", str(project))
            images.append(
                self.docker(
                    "image", "inspect", "--format", "{{.Id}}", f"{self.name}:{version}"
                ).stdout.strip()
            )
        return images

    def register(self) -> None:
        self.compose("up", "-d", "--wait", "postgres")
        location: dict[str, Any] = json.loads(
            self.compose("run", "--rm", "--no-deps", "server", "admin", "create-location", "orders")
        )
        self.location_id = location["location_id"]
        self.environment["SQLBUILD_LOCATION_ID"] = self.location_id
        expiry: str = (datetime.now(UTC) + timedelta(hours=1)).isoformat()
        credential: dict[str, Any] = json.loads(
            self.compose(
                "run",
                "--rm",
                "--no-deps",
                "server",
                "admin",
                "issue-location-token",
                self.location_id,
                "--expires-at",
                expiry,
            )
        )
        self.environment["SQLBUILD_LOCATION_TOKEN"] = credential["bearer"]

    def redeploy(self, version: str) -> None:
        self.environment["SQLBUILD_LOCATION_IMAGE"] = f"{self.name}:{version}"
        self.compose("up", "-d", "--no-deps", "location")

    def close(self) -> None:
        self.compose("stop", "--timeout", "5", "server", "location", check=False)
        for service in ("server", "location", "postgres"):
            (self.directory / f"{service}.log").write_text(
                self.compose("logs", "--no-color", service, check=False)
            )
        workers: list[str] = self.docker(
            "ps", "--all", "--quiet", "--filter", f"name=sqlbuild-{self.location_id}-"
        ).stdout.split()
        for worker in workers:
            (self.directory / f"worker-{worker}.log").write_text(
                self.docker("logs", worker, check=False).stdout
            )
            self.docker("rm", "--force", worker)
        self.compose("down", "--volumes", "--remove-orphans")
        for version in ("first", "second", "broken"):
            self.docker("image", "rm", f"{self.name}:{version}", check=False)


@contextmanager
def platform_receiver() -> Iterator[tuple[int, list[dict[str, Any]]]]:
    """Own the real webhook HTTP boundary even if stack cleanup itself fails."""
    events: list[dict[str, Any]] = []

    class Receiver(BaseHTTPRequestHandler):
        def do_POST(self) -> None:
            event: dict[str, Any] = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
            assert self.headers["Idempotency-Key"] == event["event_id"]
            events.append(event)
            self.send_response(204)
            self.end_headers()

        def log_message(self, format: str, *args: object) -> None:
            """Keep HTTP chatter out of test output."""

    with ThreadingHTTPServer(("0.0.0.0", 0), Receiver) as receiver:
        thread: Thread = Thread(target=receiver.serve_forever, daemon=True)
        thread.start()
        try:
            yield receiver.server_port, events
        finally:
            receiver.shutdown()
            thread.join(timeout=5)
            assert not thread.is_alive()


def wait_for_location_outage(
    events: list[dict[str, Any]], prior_events: set[str]
) -> dict[str, Any]:
    """Wait for fresh delivery, not an outage already received during a prior restart."""
    deadline: float = time.monotonic() + 135
    matches: list[dict[str, Any]] = []
    while not matches:
        assert time.monotonic() < deadline, (
            "Native platform sink did not report the stopped location"
        )
        time.sleep(0.5)
        matches = list(
            filter(
                lambda event: (
                    event["event_id"] not in prior_events
                    and event["payload"]["platform"]["kind"] == "location_broken"
                ),
                events,
            )
        )
    return matches[0]
