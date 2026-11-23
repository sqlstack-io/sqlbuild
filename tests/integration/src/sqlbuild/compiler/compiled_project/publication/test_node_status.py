"""A worker reports each Python node as it starts and finishes, so a chain keeps finished steps."""

import json
import os
import signal
import sqlite3
import subprocess
import sys
import time
from pathlib import Path
from typing import Any
from uuid import uuid4

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    NodeStatusTestCase,
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

from sqlbuild.assets import AssetContext, asset
from sqlbuild.refs import model

DIRECTORY = Path({directory!r})


@asset(depends_on={depends_on})
def pick(ctx: AssetContext) -> None:
    """First step."""


@asset(depends_on=pick)
def pack(ctx: AssetContext) -> None:
    """Second step, which waits to be interrupted."""
    (DIRECTORY / "pack.pid").write_text(str(os.getpid()))
    time.sleep(120)


@asset(depends_on=pack)
def ship(ctx: AssetContext) -> None:
    """Third step."""
'''
_STEPS: frozenset[str] = frozenset({"asset:pick", "asset:pack", "asset:ship"})


@pytest.mark.skipif(sys.platform != "linux", reason="the worker is interrupted with SIGKILL")
@pytest.mark.parametrize(
    "test_case",
    [
        NodeStatusTestCase(
            description="a chain ready when the build starts",
            first_depends_on="()",
            job_select="pick pack ship",
            job_settings="",
            files={},
            expected_statuses_during_second={
                "asset:pick": "success",
                "asset:pack": "running",
                "asset:ship": "pending",
            },
            expected_rerun_selection=frozenset({"asset:pack", "asset:ship"}),
        ),
        NodeStatusTestCase(
            description="a chain made ready by a finished model",
            first_depends_on='model("orders")',
            job_select="orders pick pack ship",
            job_settings="",
            files={},
            expected_statuses_during_second={
                "asset:pick": "success",
                "asset:pack": "running",
                "asset:ship": "pending",
            },
            expected_rerun_selection=frozenset({"asset:pack", "asset:ship"}),
        ),
        NodeStatusTestCase(
            description="an ingress chain ahead of its loader",
            first_depends_on="()",
            job_select="+raw_steps",
            job_settings="\n[settings]\nreload = true\n",
            files={
                "python/nodes/loads/fetch_steps/loader.py": (
                    "from sqlbuild.loaders import loader\n"
                    "from python.nodes.steps import ship\n"
                    "\n"
                    "\n"
                    "@loader(depends_on=[ship], write_strategy='table', columns=[{'name': 'step', 'type': 'INTEGER'}])\n"
                    "def fetch_steps(ctx):\n"
                    '    """Fetch the steps."""\n'
                    '    return [{"step": 1}]\n'
                ),
                "python/nodes/loads/raw_steps/loader.py": (
                    "from sqlbuild.loaders import loader\n"
                    "from python.nodes.steps import ship\n"
                    "from python.nodes.loads.fetch_steps.loader import fetch_steps\n"
                    "\n"
                    "\n"
                    "@loader(depends_on=[fetch_steps])\n"
                    "def raw_steps(ctx):\n"
                    '    """Load the steps."""\n'
                    '    return [{"step": 1}]\n'
                ),
                "python/nodes/loads/__init__.py": (
                    "from .fetch_steps.loader import fetch_steps\n"
                    "from .raw_steps.loader import raw_steps\n"
                ),
                "sql/sources/raw.yml": (
                    "sources:\n  - name: raw_steps\n    description: Steps.\n"
                    "    managed: true\n    write_strategy: table\n"
                ),
            },
            expected_statuses_during_second={
                "asset:pick": "success",
                "asset:pack": "running",
                "asset:ship": "pending",
            },
            expected_rerun_selection=frozenset({"asset:pack", "asset:ship"}),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_chain_of_three_when_the_second_is_interrupted_then_the_first_stays_succeeded(
    test_case: NodeStatusTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    chain: str = _CHAIN.format(directory=str(tmp_path), depends_on=test_case.first_depends_on)
    execution: WorkerExecutionTestCase = WorkerExecutionTestCase(
        description=test_case.description,
        model_sql="SELECT 1 AS amount",
        warehouse_query="",
        expected_status="partial",
        expected_rows=[],
        files={"python/nodes/steps.py": chain} | test_case.files,
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
    with scheduled_worker(tmp_path, monkeypatch, execution) as worker:
        pidfile: Path = tmp_path / "pack.pid"
        deadline: float = time.monotonic() + 40
        while not (pidfile.exists() and pidfile.read_text()):
            assert time.monotonic() < deadline, worker.log.read_text()
            time.sleep(0.05)
        with sqlite3.connect(worker.control_store) as control:
            during: dict[str, str] = dict(
                control.execute(
                    "SELECT node_key, status FROM sqb_run_nodes WHERE kind='node'"
                ).fetchall()
            )
        os.kill(int(pidfile.read_text()), signal.SIGKILL)
        assert {key: during[key] for key in _STEPS} == test_case.expected_statuses_during_second

        run: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="annual_orders_job"
        )
        with sqlite3.connect(worker.control_store) as control:
            after: dict[str, str] = dict(
                control.execute(
                    "SELECT node_key, status FROM sqb_run_nodes WHERE kind='node' AND run_id=?",
                    (run["run_id"],),
                ).fetchall()
            )
        assert after["asset:pick"] == "success", str(after)
        rerun_id: str = str(uuid4())
        requested: subprocess.CompletedProcess[str] = subprocess.run(
            [str(Path(sys.executable).with_name("sqb")), "admin"]
            + ["--database-env", "SQLBUILD_SCHEDULER_TEST_STORE"]
            + ["rerun-run", run["run_id"], "--run-id", rerun_id, "--from-failure"]
            + ["--reason", "Retry the interrupted steps."],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
        assert requested.returncode == 0, requested.stderr
        with sqlite3.connect(worker.control_store) as control:
            selection: list[str] = json.loads(
                control.execute(
                    "SELECT selection_json FROM sqb_runs WHERE run_id=?", (rerun_id,)
                ).fetchone()[0]
            )
        assert _STEPS.intersection(selection) == test_case.expected_rerun_selection
