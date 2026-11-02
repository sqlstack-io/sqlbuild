"""Start and reap an installed native executable in a disposable installation."""

import json
import os
import subprocess
import time
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path
from typing import Any


def environment() -> dict[str, str]:
    """Never inherit a real control store into a disposable acceptance run."""
    values: dict[str, str] = os.environ.copy()
    values.pop("SQLBUILD_CONTROL_DATABASE_URL", None)
    return values


def administer(
    installation: tuple[Path, Path], arguments: list[str], password: str | None = None
) -> subprocess.CompletedProcess[str]:
    """Password input remains on stdin and is never included in process arguments or logs."""
    binary, root = installation
    return subprocess.run(
        [str(binary), "admin", *arguments],
        cwd=root,
        env=environment(),
        input=password,
        text=True,
        encoding="utf-8",
        capture_output=True,
        timeout=20,
        check=False,
    )


def successful(output: subprocess.CompletedProcess[str]) -> dict[str, Any]:
    """Parse machine stdout only after the native command completed successfully."""
    assert output.returncode == 0, f"Native administration failed: {output.stderr}"
    return json.loads(output.stdout)


@contextmanager
def serve(installation: tuple[Path, Path]) -> Iterator[str]:
    """Kill and reap the server before returning, on Windows as well as Unix."""
    binary, root = installation
    log: Path = root / "server.log"
    with log.open("w", encoding="utf-8") as stderr:
        process: subprocess.Popen[bytes] = subprocess.Popen(
            [
                str(binary),
                "server",
                "--port",
                "0",
                "--no-scheduler",
                "--password-origin",
                "https://control.example.com",
            ],
            cwd=root,
            env=environment(),
            stdout=subprocess.DEVNULL,
            stderr=stderr,
        )
    try:
        deadline: float = time.monotonic() + 10
        while "Control API ready: " not in log.read_text(encoding="utf-8"):
            assert process.poll() is None, log.read_text(encoding="utf-8")
            assert time.monotonic() < deadline, log.read_text(encoding="utf-8")
            time.sleep(0.02)
        address: str = (
            log.read_text(encoding="utf-8").split("Control API ready: ", 1)[1].splitlines()[0]
        )
        yield f"http://{address}"
    finally:
        process.kill()
        process.wait(timeout=10)
