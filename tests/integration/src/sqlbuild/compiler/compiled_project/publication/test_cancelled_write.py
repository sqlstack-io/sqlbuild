"""A cancel that kills a finished step's identity write still reports every finished step."""

import sqlite3
import subprocess
import sys
import time
from pathlib import Path
from typing import Any
from uuid import uuid4

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    CancelledWriteTestCase,
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    INGRESS_JOB_SETTINGS,
    INGRESS_LOADER_FILES,
    scheduled_worker,
    wait_for_run,
)

_STEPS: str = '''\
import time
from pathlib import Path

from sqlbuild._native import termination_requested
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.assets import AssetContext, asset
from sqlbuild.discovery import Discovered, Item, discover

DIRECTORY = Path({directory!r})
EXECUTE = DuckDbAdapter._execute
INTERRUPT = DuckDbAdapter.interrupt_connection
WRITING_RESULTS = set()


def slow_identity_write(self, connection, sql):
    """Once a cancel lands, an identity write takes seconds; a result write stays quick."""
    if termination_requested() and "_sqlbuild_fingerprints" in sql:
        sql = "SELECT max(sqrt(step.range)) FROM range(3000000000) step"
    elif "_sqlbuild_node_results" in sql or "CREATE SCHEMA" in sql:
        WRITING_RESULTS.add(id(connection))
    try:
        return EXECUTE(self, connection=connection, sql=sql)
    finally:
        WRITING_RESULTS.discard(id(connection))


def cancel_identity_writes_only(self, connection):
    """A result write is over before a cancel reaches it, so only identity writes get cancelled."""
    return id(connection) in WRITING_RESULTS or INTERRUPT(self, connection)


DuckDbAdapter._execute = slow_identity_write
DuckDbAdapter.interrupt_connection = cancel_identity_writes_only


@discover
def fulfilment_plan() -> Discovered:
    """Five steps at once, and one after them."""
    steps = [Item(id=step) for step in ("label", "pack", "pick", "ship", "stuck")]
    return Discovered(items=[*steps, Item(id="bill", depends_on=("pick",))], max_parallel=5)


@asset(for_each=fulfilment_plan, tags=["fulfilment"])
def fulfil(ctx: AssetContext) -> None:
    """Finish as the cancel lands, but ship 2 seconds later and stuck never."""
    step = ctx.discovered.item
    with (DIRECTORY / "ran.txt").open("a") as ran:
        ran.write(f"{{step}}\\n")
    if (DIRECTORY / "cancelled").exists():
        return
    (DIRECTORY / f"{{step}}.running").touch()
    deadline = time.monotonic() + 60
    while not termination_requested() and time.monotonic() < deadline:
        time.sleep(0.02)
    time.sleep({{"ship": 2, "stuck": 120}}.get(step, 0))
'''
_STATUSES: dict[str, str] = {
    "asset:fulfil[bill]": "cancelled",
    "asset:fulfil[label]": "success",
    "asset:fulfil[pack]": "success",
    "asset:fulfil[pick]": "success",
    "asset:fulfil[ship]": "success",
    "asset:fulfil[stuck]": "cancelled",
}


@pytest.mark.skipif(sys.platform == "win32", reason="a cancel stops the worker with SIGTERM")
@pytest.mark.parametrize(
    "test_case",
    [
        CancelledWriteTestCase(
            description="on the read side",
            job_select="fulfil",
            job_settings="",
            files={},
            expected_statuses=_STATUSES,
            expected_rerun=["bill", "stuck"],
        ),
        CancelledWriteTestCase(
            description="in ingress",
            job_select="+raw_steps",
            job_settings=INGRESS_JOB_SETTINGS,
            files=INGRESS_LOADER_FILES,
            expected_statuses=_STATUSES,
            expected_rerun=["bill", "stuck"],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_steps_finishing_as_a_cancel_lands_when_a_write_is_cancelled_then_all_stay_succeeded(
    test_case: CancelledWriteTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    execution: WorkerExecutionTestCase = WorkerExecutionTestCase(
        description=test_case.description,
        model_sql="SELECT 1 AS amount",
        warehouse_query="",
        expected_status="cancelled",
        expected_rows=[],
        files={"python/steps.py": _STEPS.format(directory=str(tmp_path))} | test_case.files,
        default_job_files={
            "orchestration/jobs/annual_orders_job.toml": (
                'name = "annual_orders_job"\n'
                'description = "Run the steps."\n'
                f'select = "{test_case.job_select}"\n'
                'schedule = { cron = "0 0 * * *", timezone = "UTC", '
                'starts_at = "2000-01-01T00:00:00Z" }\n'
                f"{test_case.job_settings}"
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
        deadline: float = time.monotonic() + 40
        while len(list(tmp_path.glob("*.running"))) < 5:
            assert time.monotonic() < deadline, worker.log.read_text()
            time.sleep(0.05)
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
        first: dict[str, Any] = wait_for_run(worker.api, worker.process, worker.log, run_id=run_id)
        assert first["status"] == execution.expected_status, worker.log.read_text()
        with sqlite3.connect(worker.control_store) as control:
            statuses: dict[str, str] = dict(
                control.execute(
                    "SELECT node_key, status FROM sqb_run_nodes "
                    "WHERE run_id=? AND kind='node' AND node_key LIKE 'asset:fulfil[%'",
                    (run_id,),
                ).fetchall()
            )
        assert statuses == test_case.expected_statuses
        ran.write_text("")
        (tmp_path / "cancelled").touch()
        rerun_id: str = str(uuid4())
        rerun: subprocess.CompletedProcess[str] = subprocess.run(
            [*sqb, "rerun-run", run_id, "--run-id", rerun_id, "--from-failure"]
            + ["--reason", "Finish the plan."],
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

    assert sorted(ran.read_text().split()) == test_case.expected_rerun
