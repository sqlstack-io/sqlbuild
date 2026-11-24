"""A cancel reaching every statement leaves the steps that finished succeeded."""

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
from tests.unit.src.sqlbuild.python_nodes.helpers import node_module_files

_STEPS: str = """\
import time
from pathlib import Path

from sqlbuild._native import termination_requested
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.assets import AssetContext, asset
from sqlbuild.discovery import Discovered, Item, discover

DIRECTORY = Path({directory!r})
EXECUTE = DuckDbAdapter._execute
BUSY = "SELECT max(sqrt(step.range)) FROM range(20000000) step"


def busy_bookkeeping(self, connection, sql):
    \"\"\"A finished step's result and identity writes stay busy until just after the cancel lands.\"\"\"
    inserting = sql.lstrip().upper().startswith("INSERT")
    if inserting and "_sqlbuild_node_results" in sql:
        (DIRECTORY / f"writing-{{threading_name()}}").touch()
    if (
        inserting
        and ("_sqlbuild_node_results" in sql or "_sqlbuild_fingerprints" in sql)
        and not (DIRECTORY / "cancelled").exists()
    ):
        until = None
        while until is None or time.monotonic() < until:
            if until is None and termination_requested():
                until = time.monotonic() + 0.5
            EXECUTE(self, connection=connection, sql=BUSY)
            if until is None and not (DIRECTORY / "go").exists():
                break
    return EXECUTE(self, connection=connection, sql=sql)


def threading_name():
    import threading
    return threading.current_thread().name


DuckDbAdapter._execute = busy_bookkeeping


@discover
def fulfilment_plan() -> Discovered:
    \"\"\"Five steps at once, and one after them.\"\"\"
    steps = [Item(id=step) for step in ("label", "pack", "pick", "ship", "stuck")]
    return Discovered(items=[*steps, Item(id="bill", depends_on=("pick",))], max_parallel=5)


@asset(for_each=fulfilment_plan, tags=["fulfilment"])
def fulfil(ctx: AssetContext) -> None:
    \"\"\"Three finish before the cancel; ship finishes 2 seconds into it and stuck never.\"\"\"
    step = ctx.discovered.item
    with (DIRECTORY / "ran.txt").open("a") as ran:
        ran.write(f"{{step}}\\n")
    if (DIRECTORY / "cancelled").exists():
        return
    (DIRECTORY / f"{{step}}.running").touch()
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        if step in ("label", "pack", "pick") and (DIRECTORY / "go").exists():
            return
        if step == "ship" and termination_requested():
            time.sleep(2)
            return
        time.sleep(0.02)
"""
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
        files={**node_module_files("python/nodes/steps.py", _STEPS.format(directory=str(tmp_path)))}
        | test_case.files,
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
        (tmp_path / "go").touch()
        while not list(tmp_path.glob("writing-*")):
            assert time.monotonic() < deadline, worker.log.read_text()
            time.sleep(0.02)
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
