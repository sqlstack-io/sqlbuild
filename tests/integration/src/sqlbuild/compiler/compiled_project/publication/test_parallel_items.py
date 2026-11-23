"""A worker runs discovered items side by side on real connections, and cancels them cleanly."""

import json
import sqlite3
import subprocess
import sys
import time
from pathlib import Path
from typing import Any
from uuid import uuid4

import duckdb
import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    ParallelItemsTestCase,
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    scheduled_worker,
    wait_for_run,
)

_TOGETHER: str = '''\
import threading

from sqlbuild.assets import AssetContext, asset
from sqlbuild.discovery import Discovered, Item, discover
from sqlbuild.refs import model

STEPS = ("label", "pack", "pick", "ship")
MEET = threading.Barrier(len(STEPS), timeout=30)


@discover
def fulfilment_plan() -> Discovered:
    """Four steps that run at once."""
    return Discovered(items=[Item(id=step) for step in STEPS], max_parallel=len(STEPS))


@asset(for_each=fulfilment_plan, depends_on={depends_on})
def fulfil(ctx: AssetContext):
    """Write a table and a result while every other step does."""
    step = ctx.discovered.item
    MEET.wait()
    for _ in range(STEPS.index(step) + 1):
        print(f"printing {{step}}", flush=True)
    ctx.execute_sql(f"CREATE OR REPLACE TABLE step_{{step}} AS SELECT '{{step}}' AS step")
    count = ctx.query(f"SELECT count(*) FROM step_{{step}}").fetchall()[0][0]
    MEET.wait()
    return ctx.result(payload={{"step": step, "count": count}})
'''
_CANCELLED: str = '''\
from pathlib import Path

from sqlbuild.assets import AssetContext, asset
from sqlbuild.discovery import Discovered, Item, discover
from sqlbuild.refs import model

DIRECTORY = Path({directory!r})


@discover
def fulfilment_plan() -> Discovered:
    """A step that finishes, one that fails, two long ones and one that waits."""
    items = [Item(id="done"), Item(id="broken"), Item(id="long_a"), Item(id="long_b")]
    return Discovered(items=[*items, Item(id="waiting", depends_on=("long_a",))], max_parallel=4)


@asset(for_each=fulfilment_plan, depends_on={depends_on})
def fulfil(ctx: AssetContext) -> None:
    """Run one step."""
    step = ctx.discovered.item
    (DIRECTORY / f"{{step}}.started").touch()
    if step == "broken":
        raise ValueError("broken step")
    if step.startswith("long"):
        ctx.query("SELECT count(*) FROM range(10000000000000) t(v) WHERE v % 7 = 0").fetchall()
'''


@pytest.mark.parametrize(
    "test_case",
    [
        ParallelItemsTestCase(
            description="steps ready when the build starts",
            first_depends_on="()",
            job_select="fulfil",
            expected_status="success",
            expected_statuses={
                f"asset:fulfil[{step}]": "success" for step in ("label", "pack", "pick", "ship")
            },
        ),
        ParallelItemsTestCase(
            description="steps made ready by a finished model",
            first_depends_on='model("orders")',
            job_select="orders fulfil",
            expected_status="success",
            expected_statuses={
                f"asset:fulfil[{step}]": "success" for step in ("label", "pack", "pick", "ship")
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_concurrent_steps_using_sql_when_a_worker_runs_them_then_writes_and_output_do_not_collide(
    test_case: ParallelItemsTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    execution: WorkerExecutionTestCase = WorkerExecutionTestCase(
        description=test_case.description,
        model_sql="SELECT 1 AS amount",
        warehouse_query="",
        expected_status=test_case.expected_status,
        expected_rows=[],
        files={"python/nodes/steps.py": _TOGETHER.format(depends_on=test_case.first_depends_on)},
        default_job_files={
            "orchestration/jobs/annual_orders_job.toml": (
                'name = "annual_orders_job"\n'
                'description = "Run the steps."\n'
                f'select = "{test_case.job_select}"\n'
                'schedule = { cron = "0 0 * * *", timezone = "UTC", '
                'starts_at = "2000-01-01T00:00:00Z" }\n'
            )
        },
    )
    with scheduled_worker(tmp_path, monkeypatch, execution) as worker:
        run: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="annual_orders_job"
        )
        assert run["status"] == test_case.expected_status, worker.log.read_text()
        with sqlite3.connect(worker.control_store) as control:
            statuses: dict[str, str] = dict(
                control.execute(
                    "SELECT node_key, status FROM sqb_run_nodes "
                    "WHERE run_id=? AND kind='node' AND node_key LIKE 'asset:%'",
                    (run["run_id"],),
                ).fetchall()
            )
            printed: dict[str, int] = dict(
                control.execute(
                    "SELECT node_key, line_count FROM sqb_run_output_streams "
                    "WHERE run_id=? AND stream='stdout' AND node_key LIKE 'asset:%'",
                    (run["run_id"],),
                ).fetchall()
            )
        with duckdb.connect(str(worker.project / "warehouse.duckdb")) as warehouse:
            tables: set[str] = {
                name
                for (name,) in warehouse.execute(
                    "SELECT table_name FROM duckdb_tables()"
                ).fetchall()
            }

    assert statuses == test_case.expected_statuses
    steps: dict[str, str] = {key: key[len("asset:fulfil[") : -1] for key in statuses}
    assert printed == {key: index + 1 for index, key in enumerate(sorted(steps))}
    assert {f"step_{step}" for step in steps.values()} <= tables


@pytest.mark.skipif(sys.platform == "win32", reason="a cancel stops the worker with SIGTERM")
@pytest.mark.parametrize(
    "test_case",
    [
        ParallelItemsTestCase(
            description=description,
            first_depends_on=depends_on,
            job_select=job_select,
            expected_status="cancelled",
            expected_statuses={
                "asset:fulfil[done]": "success",
                "asset:fulfil[broken]": "failed",
                "asset:fulfil[long_a]": "cancelled",
                "asset:fulfil[long_b]": "cancelled",
                "asset:fulfil[waiting]": "cancelled",
            },
        )
        for description, depends_on, job_select in (
            ("steps ready when the build starts", "()", "fulfil"),
            ("steps made ready by a finished model", 'model("orders")', "orders fulfil"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_steps_in_flight_when_cancelled_then_finished_steps_stay_and_are_not_rerun(
    test_case: ParallelItemsTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    steps: str = _CANCELLED.format(directory=str(tmp_path), depends_on=test_case.first_depends_on)
    execution: WorkerExecutionTestCase = WorkerExecutionTestCase(
        description=test_case.description,
        model_sql="SELECT 1 AS amount",
        warehouse_query="",
        expected_status=test_case.expected_status,
        expected_rows=[],
        files={"python/nodes/steps.py": steps},
        default_job_files={
            "orchestration/jobs/annual_orders_job.toml": (
                'name = "annual_orders_job"\n'
                'description = "Run the steps."\n'
                f'select = "{test_case.job_select}"\n'
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
    with scheduled_worker(tmp_path, monkeypatch, execution) as worker:
        deadline: float = time.monotonic() + 40
        with sqlite3.connect(worker.control_store) as control:
            progress: str = (
                "SELECT count(*) FROM sqb_run_nodes WHERE kind='node' "
                "AND node_key IN ('asset:fulfil[done]', 'asset:fulfil[broken]') "
                "AND status IN ('success', 'failed')"
            )
            while (
                control.execute(progress).fetchone()[0],
                len(list(tmp_path.glob("long_*.started"))),
            ) != (2, 2):
                assert time.monotonic() < deadline, worker.log.read_text()
                time.sleep(0.05)
            run_id: str = control.execute("SELECT run_id FROM sqb_runs").fetchone()[0]
        cancelled: subprocess.CompletedProcess[str] = subprocess.run(
            [*sqb, "cancel-run", run_id, "--event-id", str(uuid4()), "--reason", "Stop."],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
        assert cancelled.returncode == 0, cancelled.stderr
        run: dict[str, Any] = wait_for_run(worker.api, worker.process, worker.log, run_id=run_id)
        assert run["status"] == test_case.expected_status, worker.log.read_text()
        with sqlite3.connect(worker.control_store) as control:
            statuses: dict[str, str] = dict(
                control.execute(
                    "SELECT node_key, status FROM sqb_run_nodes "
                    "WHERE run_id=? AND kind='node' AND node_key LIKE 'asset:%'",
                    (run_id,),
                ).fetchall()
            )
            assert statuses == test_case.expected_statuses, worker.log.read_text()
            rerun_id: str = str(uuid4())
            rerun: subprocess.CompletedProcess[str] = subprocess.run(
                [*sqb, "rerun-run", run_id, "--run-id", rerun_id, "--from-failure"]
                + ["--reason", "Retry."],
                capture_output=True,
                text=True,
                timeout=10,
                check=False,
            )
            assert rerun.returncode == 0, rerun.stderr
            selection: list[str] = json.loads(
                control.execute(
                    "SELECT selection_json FROM sqb_runs WHERE run_id=?", (rerun_id,)
                ).fetchone()[0]
            )
    assert set(selection).intersection(test_case.expected_statuses) == (
        set(test_case.expected_statuses) - {"asset:fulfil[done]"}
    )
