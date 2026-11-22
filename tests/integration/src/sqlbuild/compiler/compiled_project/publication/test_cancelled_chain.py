"""A cancelled chain ends promptly, and a rerun from failure runs exactly what is unfinished."""

import sqlite3
import subprocess
import sys
import time
from pathlib import Path
from typing import Any
from uuid import uuid4

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    CancelledChainTestCase,
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    scheduled_worker,
    wait_for_run,
)

_CHAIN: str = '''\
import os
import time
from pathlib import Path

from providers.marker import MarkerProvider
from sqlbuild.assets import AssetContext, asset
from sqlbuild.discovery import Discovered, Item, discover

DIRECTORY = Path({directory!r})


@discover
def fulfilment_plan() -> Discovered:
    """A chain of three."""
    return Discovered(
        items=[
            Item(id="pick"),
            Item(id="pack", depends_on=("pick",)),
            Item(id="ship", depends_on=("pack",)),
        ],
        max_parallel={max_parallel},
    )


@asset(for_each=fulfilment_plan)
def fulfil(ctx: AssetContext, marker_provider: MarkerProvider) -> None:
    """Record the step and its chain; the first pack ignores any cancel."""
    with (DIRECTORY / "ran.txt").open("a") as ran:
        ran.write(f"{{ctx.discovered.item}} {{ctx.root_run_id}}\\n")
    if ctx.discovered.item == "pack" and not (DIRECTORY / "pack.pid").exists():
        (DIRECTORY / "pack.pid").write_text(str(os.getpid()))
        time.sleep(120)
'''
_PROVIDER: str = '''\
import os
from pathlib import Path

from sqlbuild.providers import Provider


class MarkerProvider(Provider):
    """Say which process tore it down."""

    def teardown(self) -> None:
        Path({directory!r}, f"torn-down.{{os.getpid()}}").touch()
'''


@pytest.mark.skipif(sys.platform == "win32", reason="a cancel stops the worker with SIGTERM")
@pytest.mark.parametrize(
    "test_case",
    [
        CancelledChainTestCase(
            description="a parallel step that ignores the cancel is left after 5 seconds",
            max_parallel=2,
            expected_first_run=["pick", "pack"],
            expected_second_run=["pack", "ship"],
            expected_statuses={
                "asset:fulfil[pick]": "success",
                "asset:fulfil[pack]": "cancelled",
                "asset:fulfil[ship]": "cancelled",
            },
            expected_torn_down=False,
        ),
        CancelledChainTestCase(
            description="a serial step is interrupted",
            max_parallel=1,
            expected_first_run=["pick", "pack"],
            expected_second_run=["pack", "ship"],
            expected_statuses={
                "asset:fulfil[pick]": "success",
                "asset:fulfil[pack]": "cancelled",
                "asset:fulfil[ship]": "cancelled",
            },
            expected_torn_down=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_cancel_mid_chain_when_rerun_from_failure_then_only_unfinished_steps_run_in_the_chain(
    test_case: CancelledChainTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    execution: WorkerExecutionTestCase = WorkerExecutionTestCase(
        description=test_case.description,
        model_sql="SELECT 1 AS amount",
        warehouse_query="",
        expected_status="cancelled",
        expected_rows=[],
        files={
            "python/steps.py": _CHAIN.format(
                directory=str(tmp_path), max_parallel=test_case.max_parallel
            ),
            "providers/marker.py": _PROVIDER.format(directory=str(tmp_path)),
        },
        default_job_files={
            "orchestration/jobs/annual_orders_job.toml": (
                'name = "annual_orders_job"\n'
                'description = "Run the steps."\n'
                'select = "fulfil"\n'
                'schedule = { cron = "0 0 * * *", timezone = "UTC", '
                'starts_at = "2000-01-01T00:00:00Z" }\n'
            )
        },
    )
    sqb: list[str] = [
        str(Path(sys.executable).with_name("sqb")),
        "admin",
        "--database-env",
        "SQLBUILD_SCHEDULER_TEST_STORE",
    ]
    ran: Path = tmp_path / "ran.txt"
    with scheduled_worker(tmp_path, monkeypatch, execution) as worker:
        pidfile: Path = tmp_path / "pack.pid"
        deadline: float = time.monotonic() + 40
        while not (pidfile.exists() and pidfile.read_text()):
            assert time.monotonic() < deadline, worker.log.read_text()
            time.sleep(0.05)
        worker_pid: int = int(pidfile.read_text())
        with sqlite3.connect(worker.control_store) as control:
            run_id: str = control.execute("SELECT run_id FROM sqb_runs").fetchone()[0]
        cancelled: subprocess.CompletedProcess[str] = subprocess.run(
            [*sqb, "cancel-run", run_id, "--event-id", str(uuid4()), "--reason", "Stop."],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
        assert cancelled.returncode == 0, cancelled.stderr
        cancelled_at: float = time.monotonic()
        first: dict[str, Any] = wait_for_run(worker.api, worker.process, worker.log, run_id=run_id)
        ended_after: float = time.monotonic() - cancelled_at
        assert first["status"] == execution.expected_status, worker.log.read_text()
        assert ended_after < 30, worker.log.read_text()
        with sqlite3.connect(worker.control_store) as control:
            statuses: dict[str, str] = dict(
                control.execute(
                    "SELECT node_key, status FROM sqb_run_nodes "
                    "WHERE run_id=? AND kind='node' AND node_key LIKE 'asset:%'",
                    (run_id,),
                ).fetchall()
            )
        assert statuses == test_case.expected_statuses
        while Path(f"/proc/{worker_pid}/cmdline").exists():
            assert time.monotonic() < cancelled_at + 30, worker.log.read_text()
            time.sleep(0.05)
        torn_down: bool = (tmp_path / f"torn-down.{worker_pid}").exists()
        assert torn_down == test_case.expected_torn_down
        rerun_id: str = str(uuid4())
        rerun: subprocess.CompletedProcess[str] = subprocess.run(
            [*sqb, "rerun-run", run_id, "--run-id", rerun_id, "--from-failure"]
            + ["--reason", "Finish the chain."],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
        assert rerun.returncode == 0, rerun.stderr
        second: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, run_id=rerun_id
        )
        assert second["status"] == "success", worker.log.read_text()

    expected: list[str] = test_case.expected_first_run + test_case.expected_second_run
    assert ran.read_text().splitlines() == [f"{step} {run_id}" for step in expected]
