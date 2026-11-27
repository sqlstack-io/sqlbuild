"""A running asset's `ctx.log` lines and printed output reach the server while it runs, not when it ends."""

import time
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    LiveLogTestCase,
    LogOverflowTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    live_log_execution,
    marked_output,
    scheduled_worker,
    stored_node_logs,
    wait_for_run,
)

_NODE: str = "asset:stream_steps"
_BLOCKED_ASSET: str = '''\
import pathlib
import time

from sqlbuild.assets import AssetContext, asset

RELEASE = pathlib.Path({release!r})


@asset(tags=["steps"])
def stream_steps(ctx: AssetContext):
    """Report the first step, then wait until the test lets the asset finish."""
    {first_step}
    deadline = time.monotonic() + 60
    while not RELEASE.exists():
        if time.monotonic() > deadline:
            raise TimeoutError("the test never released the asset")
        time.sleep(0.05)
    ctx.log.info("step 2 of 2")
'''
_CHATTY_ASSET: str = '''\
from sqlbuild.assets import AssetContext, asset


@asset(tags=["steps"])
def stream_steps(ctx: AssetContext):
    """Log more lines than a node keeps."""
    for line in range({lines}):
        ctx.log.info(f"line {{line}}")
'''


@pytest.mark.parametrize(
    "test_case",
    [
        LiveLogTestCase(
            description="a ctx.log line shows while the asset runs",
            first_step='ctx.log.info("step 1 of 2")',
            expected_live_logs=[("info", "step 1 of 2")],
            expected_live_output=[],
            expected_logs=[("info", "step 1 of 2"), ("info", "step 2 of 2")],
        ),
        LiveLogTestCase(
            description="a printed line shows in the output while the asset runs",
            first_step='print("WARNING: printed step 1 of 2")',
            expected_live_logs=[],
            expected_live_output=["WARNING: printed step 1 of 2"],
            expected_logs=[("info", "step 2 of 2")],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_running_asset_when_it_logs_or_prints_then_the_server_has_the_line_before_it_ends(
    test_case: LiveLogTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    release: Path = tmp_path / "release"
    steps: str = _BLOCKED_ASSET.format(release=str(release), first_step=test_case.first_step)
    with scheduled_worker(
        tmp_path, monkeypatch, live_log_execution(test_case.description, steps)
    ) as worker:
        running: dict[str, Any] = wait_for_run(
            worker.api,
            worker.process,
            worker.log,
            pending=frozenset({"queued", "launched"}),
            job_key="steps_job",
        )
        deadline: float = time.monotonic() + 20
        live: tuple[list[tuple[str, str]], list[str]] = ([], [])
        expected: tuple[list[tuple[str, str]], list[str]] = (
            test_case.expected_live_logs,
            test_case.expected_live_output,
        )
        while not (live[0][: len(expected[0])] == expected[0] and set(expected[1]) <= set(live[1])):
            assert worker.process.poll() is None, worker.log.read_text()
            assert time.monotonic() < deadline, (live, worker.log.read_text()[-4000:])
            time.sleep(0.1)
            live = (
                stored_node_logs(worker.control_store, running["run_id"], _NODE),
                marked_output(worker.control_store, running["run_id"], _NODE),
            )
        release.touch()
        finished: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="steps_job"
        )
        logs: list[tuple[str, str]] = stored_node_logs(
            worker.control_store, finished["run_id"], _NODE
        )

    assert finished["status"] == "success", worker.log.read_text()[-4000:]
    assert logs == test_case.expected_logs


@pytest.mark.parametrize(
    "test_case",
    [
        LogOverflowTestCase(
            description="lines past the cap end in one warning that counts them",
            lines=205,
            expected_kept=200,
            expected_last_log=(
                "warning",
                "5 more lines not kept as log lines; ctx.log keeps 200 per node, "
                "and print() output is kept in full in Output",
            ),
        ),
        LogOverflowTestCase(
            description="exactly the cap keeps every line and adds no warning",
            lines=200,
            expected_kept=200,
            expected_last_log=("info", "line 199"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_asset_logging_past_the_cap_when_a_worker_runs_it_then_the_rest_are_counted(
    test_case: LogOverflowTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    steps: str = _CHATTY_ASSET.format(lines=test_case.lines)
    with scheduled_worker(
        tmp_path, monkeypatch, live_log_execution(test_case.description, steps)
    ) as worker:
        run: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="steps_job"
        )
        logs: list[tuple[str, str]] = stored_node_logs(worker.control_store, run["run_id"], _NODE)

    assert run["status"] == "success", worker.log.read_text()[-4000:]
    assert [message for _, message in filter(lambda row: row[0] == "info", logs)] == [
        f"line {line}" for line in range(test_case.expected_kept)
    ]
    assert logs[-1] == test_case.expected_last_log
