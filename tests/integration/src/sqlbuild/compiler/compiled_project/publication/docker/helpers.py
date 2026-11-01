"""A real Docker location with the installed runtime and an isolated synthetic warehouse."""

import json
import os
import shutil
import socket
import subprocess
import sys
from collections.abc import Iterator
from contextlib import contextmanager
from http.client import HTTPConnection, HTTPResponse
from http.server import BaseHTTPRequestHandler
from pathlib import Path
from socketserver import UnixStreamServer
from threading import Event, Thread
from uuid import uuid4

import pytest

import sqlbuild
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    DockerLocation,
    WorkerDeployment,
)

DOCKER_SOCKET: Path = Path(os.environ.get("SQLBUILD_TEST_DOCKER_SOCKET", "/var/run/docker.sock"))
REQUIRES_DOCKER: pytest.MarkDecorator = pytest.mark.skipif(
    os.environ.get("SQLBUILD_REQUIRE_DOCKER_TESTS") != "1"
    and (sys.platform != "linux" or shutil.which("docker") is None or not DOCKER_SOCKET.exists()),
    reason="Docker worker tests require a Linux Docker daemon",
)


def docker_command(socket: Path, *arguments: str) -> str:
    """Keep daemon errors useful without dumping a container's credential-bearing environment."""
    result: subprocess.CompletedProcess[str] = subprocess.run(
        ["docker", "--host", f"unix://{socket}", *arguments],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        timeout=20,
        check=False,
    )
    assert result.returncode == 0, result.stdout
    return result.stdout.strip()


@contextmanager
def docker_deployment(directory: Path, executable: str) -> Iterator[WorkerDeployment]:
    """The worker inherits only these runtime/project mounts, never the store or daemon socket."""
    socket: Path = DOCKER_SOCKET
    project: Path = directory / "project"
    image: str = os.environ.get("SQLBUILD_TEST_WORKER_IMAGE", "alpine:3.23")
    image_id: str = docker_command(socket, "image", "inspect", "--format", "{{.Id}}", image)
    name: str = f"sqlbuild-test-location-{uuid4()}"
    python: Path = Path(sys.executable)
    installations: list[Path] = [Path(sys.prefix), Path(sys.base_prefix)]
    while python.is_symlink():
        python = python.parent / python.readlink()
        installations.append(python.parent.parent)
    mounts: list[str] = []
    for path in filter(
        Path.exists,
        dict.fromkeys(
            [
                Path("/usr"),
                Path("/lib"),
                Path("/lib64"),
                *installations,
                Path(sqlbuild.__file__).parent.parent,
            ]
        ),
    ):
        mounts.extend(["--volume", f"{path}:{path}:ro"])
    mounts.extend(["--volume", f"{project}:{project}:rw"])
    container: str = docker_command(
        socket,
        "run",
        "--detach",
        "--name",
        name,
        "--network",
        "host",
        "--user",
        f"{os.getuid()}:{os.getgid()}",
        "--workdir",
        str(project),
        "--env",
        "HOME=/tmp",
        "--env",
        "SQLBUILD_WORKER_TEST_VALUE=orders-context",
        "--env",
        "SQLBUILD_LOCATION_TOKEN=synthetic-location-credential",
        "--env",
        "SQLBUILD_CONTROL_DATABASE_URL=synthetic-control-store",
        "--env",
        "SQLBUILD_PLATFORM_WEBHOOK_URL=http://127.0.0.1:1/events?token=synthetic-delivery-credential",
        "--env",
        "SQLBUILD_SCHEDULER_TEST_STORE=synthetic-configured-store",
        *mounts,
        image_id,
        "/usr/bin/sleep",
        "300",
    )
    try:
        yield WorkerDeployment(
            "docker",
            {"socket": str(socket), "location_container": container, "executable": executable},
            ("--image-id", image_id),
            DockerLocation(image_id, container, socket),
        )
    finally:
        containers: list[str] = docker_command(
            socket, "ps", "--all", "--quiet", "--filter", f"volume={project}"
        ).split()
        for owned in containers:
            _ = (directory / f"docker-{owned}.log").write_text(
                docker_command(socket, "logs", owned)
            )
            _ = docker_command(socket, "rm", "--force", owned)


def worker_container(location: DockerLocation, location_id: str, run_id: str) -> dict[str, object]:
    """Return only physical identity/exit evidence, not Docker's secret-bearing configuration."""
    name: str = f"sqlbuild-{location_id}-{run_id}"
    result: dict[str, object] = json.loads(
        docker_command(location.socket, "inspect", "--format", "{{json .State}}", name)
    )
    assert (
        docker_command(location.socket, "inspect", "--format", "{{.Image}}", name)
        == location.image_id
    )
    return result


@contextmanager
def lost_start_response(directory: Path, upstream: Path, inspection: Event) -> Iterator[Path]:
    """Fail the real start result and hold physical inspection until explicitly restored."""
    proxy: Path = directory / "docker-proxy.sock"

    class Handler(BaseHTTPRequestHandler):
        def forward(self) -> None:
            connection: HTTPConnection = HTTPConnection("localhost", timeout=5)
            connection.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            connection.sock.settimeout(5)
            connection.sock.connect(str(upstream))
            try:
                body: bytes = self.rfile.read(int(self.headers.get("Content-Length", "0")))
                connection.request(
                    self.command, self.path, body, {"Content-Type": "application/json"}
                )
                response: HTTPResponse = connection.getresponse()
                payload: bytes = response.read()
                status: int = response.status
                started: bool = self.command == "POST" and self.path.endswith("/start")
                unavailable: bool = (
                    self.command == "GET"
                    and self.path.startswith("/containers/sqlbuild-")
                    and not inspection.is_set()
                )
                assert not started or status == 204, (
                    "The actual Docker start must succeed before faulting"
                )
                status, payload = {
                    (False, False): (status, payload),
                    (True, False): (503, b'{"message":"synthetic lost start response"}'),
                    (False, True): (503, b'{"message":"synthetic inspection outage"}'),
                }[(started, unavailable)]
                self.send_response(status)
                self.send_header("Content-Length", str(len(payload)))
                self.end_headers()
                self.wfile.write(payload)
            finally:
                connection.close()

        def do_GET(self) -> None:
            self.forward()

        def do_POST(self) -> None:
            self.forward()

        def log_message(self, format: str, *args: object) -> None:
            """Never copy credential-bearing Docker requests to test output."""

    with UnixStreamServer(str(proxy), Handler) as server:
        thread: Thread = Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            yield proxy
        finally:
            server.shutdown()
            thread.join(timeout=6)
            assert not thread.is_alive(), "Docker fault proxy did not stop"
            proxy.unlink(missing_ok=True)
