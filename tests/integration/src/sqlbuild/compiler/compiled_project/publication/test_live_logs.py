"""A running asset's `ctx.log` lines and printed output reach the server while it runs, not when it ends."""

import sqlite3
import time
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    LiveLogTestCase,
    LogOverflowTestCase,
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    scheduled_worker,
    wait_for_run,
)
from tests.unit.src.sqlbuild.python_nodes.helpers import node_module_files

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


def _execution(description: str, steps: str) -> WorkerExecutionTestCase:
    return WorkerExecutionTestCase(
        description=description,
        model_sql="SELECT 1 AS amount",
        warehouse_query="",
        expected_status="",
        expected_rows=[],
        files=node_module_files("python/nodes/steps.py", steps),
        default_job_files={
            "orchestration/jobs/steps_job.toml": (
                'name = "steps_job"\n'
                'description = "Run the steps."\n'
                'select = "stream_steps"\n'
                'schedule = { cron = "0 0 * * *", timezone = "UTC", '
                'starts_at = "2000-01-01T00:00:00Z" }\n'
            )
        },
    )


def _node_logs(control_store: Path, run_id: str) -> list[tuple[str, str]]:
    with sqlite3.connect(control_store) as control:
        return control.execute(
            "SELECT level, message FROM sqb_run_node_logs WHERE run_id=? AND node_key=? "
            "ORDER BY logged_at, event_id",
            (run_id, _NODE),
        ).fetchall()


def _marked_output(control_store: Path, run_id: str) -> list[str]:
    with sqlite3.connect(control_store) as control:
        return [
            text
            for (text,) in control.execute(
                "SELECT text FROM sqb_run_output_marks WHERE run_id=? AND node_key=? "
                "ORDER BY node_line",
                (run_id, _NODE),
            ).fetchall()
        ]


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
        tmp_path, monkeypatch, _execution(test_case.description, steps)
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
                _node_logs(worker.control_store, running["run_id"]),
                _marked_output(worker.control_store, running["run_id"]),
            )
        release.touch()
        finished: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="steps_job"
        )
        logs: list[tuple[str, str]] = _node_logs(worker.control_store, finished["run_id"])

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
        tmp_path, monkeypatch, _execution(test_case.description, steps)
    ) as worker:
        run: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="steps_job"
        )
        logs: list[tuple[str, str]] = _node_logs(worker.control_store, run["run_id"])

    assert run["status"] == "success", worker.log.read_text()[-4000:]
    assert [message for level, message in logs if level == "info"] == [
        f"line {line}" for line in range(test_case.expected_kept)
    ]
    assert logs[-1] == test_case.expected_last_log
