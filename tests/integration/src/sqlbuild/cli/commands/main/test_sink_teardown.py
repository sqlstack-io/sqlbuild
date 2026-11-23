"""A sink's provider session is left open for an abandoned step, and torn down after a normal run."""

import os
import signal
import subprocess
import sys
import time
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    SinkAbandonedStepTestCase,
    SinkNormalRunTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import build_arguments, sink_project


@pytest.mark.skipif(sys.platform == "win32", reason="Unix signals")
@pytest.mark.parametrize(
    "test_case",
    [
        SinkAbandonedStepTestCase(
            description="a step that ignores the cancel",
            parameters="",
            step_body='(DIRECTORY / f"{ctx.discovered.item}.running").touch(); time.sleep(120)',
            started_markers=("pack.running", "pick.running"),
            started_lines=(),
            sent_signal=signal.SIGTERM,
            expected_exit_code=143,
        ),
        SinkAbandonedStepTestCase(
            description="a step in another provider's first setup",
            parameters="gateway_provider: GatewayProvider",
            step_body="gateway_provider.model_dump()",
            started_markers=("setup-started",),
            started_lines=("fulfil[pack] START", "fulfil[pick] START"),
            sent_signal=signal.SIGINT,
            expected_exit_code=130,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_sink_sharing_providers_when_a_step_is_abandoned_then_nothing_is_torn_down(
    test_case: SinkAbandonedStepTestCase, tmp_path: Path
) -> None:
    project: Path = sink_project(
        tmp_path, parameters=test_case.parameters, body=test_case.step_body
    )
    log: Path = tmp_path / "build.log"
    with log.open("wb") as output:
        process: subprocess.Popen[bytes] = subprocess.Popen(
            build_arguments(project),
            stdout=output,
            stderr=subprocess.STDOUT,
            env=os.environ | {"HOME": str(tmp_path)},
        )
        try:
            deadline: float = time.monotonic() + 60
            while not (
                all((tmp_path / marker).exists() for marker in test_case.started_markers)
                and all(line in log.read_text() for line in test_case.started_lines)
            ):
                assert process.poll() is None, log.read_text()
                assert time.monotonic() < deadline, log.read_text()
                time.sleep(0.05)
            process.send_signal(test_case.sent_signal)
            exit_code: int = process.wait(timeout=20)
        finally:
            process.kill()
            _ = process.wait(timeout=5)

    journal: list[str] = (tmp_path / "journal.txt").read_text().splitlines()
    assert exit_code == test_case.expected_exit_code, log.read_text()
    assert "Stopped waiting for fulfil[pack], fulfil[pick] after 5 seconds" in log.read_text()
    assert "event invocation_started" in journal
    assert "torn down" not in journal


@pytest.mark.parametrize(
    "test_case",
    [
        SinkNormalRunTestCase(
            description="the sink's providers are torn down after its last event",
            expected_last_line="torn down",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_sink_sharing_providers_when_the_build_finishes_then_providers_are_torn_down(
    test_case: SinkNormalRunTestCase, tmp_path: Path
) -> None:
    project: Path = sink_project(tmp_path, parameters="", body='journal_provider.write("ran")')

    built: subprocess.CompletedProcess[str] = subprocess.run(
        build_arguments(project),
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
        env=os.environ | {"HOME": str(tmp_path)},
    )

    journal: list[str] = (tmp_path / "journal.txt").read_text().splitlines()
    assert built.returncode == 0, built.stdout + built.stderr
    assert journal.count("ran") == 2
    assert "event invocation_completed" in journal
    assert journal[-1] == test_case.expected_last_line
