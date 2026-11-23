"""One Ctrl+C on a worker whose discovered steps run no statement still reports each of them."""

import os
import signal
import sqlite3
import sys
import time
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    InterruptedStepsTestCase,
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    scheduled_worker,
    wait_for_run,
)

_STEPS: str = '''\
import os
import time
from pathlib import Path

from python.providers.marker import MarkerProvider
from sqlbuild.assets import AssetContext, asset
from sqlbuild.discovery import Discovered, Item, discover

DIRECTORY = Path({directory!r})


@discover
def fulfilment_plan() -> Discovered:
    """Three steps at once."""
    items = [Item(id="pick"), Item(id="label"), Item(id="pack")]
    return Discovered(items=items, max_parallel=len(items))


@asset(for_each=fulfilment_plan)
def fulfil(ctx: AssetContext, marker_provider: MarkerProvider) -> None:
    """Pick at once, label in the cancel's grace; pack ignores the cancel."""
    step = ctx.discovered.item
    (DIRECTORY / f"{{step}}.started").write_text(str(os.getpid()))
    if step == "label":
        while not (DIRECTORY / "pressed").exists():
            time.sleep(0.05)
        # Later than the 0.6 seconds after which a press may force an exit.
        time.sleep(1.5)
    if step == "pack":
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
# One Ctrl+C may force an exit only while no statement has ever run, so this adapter records none.
_ADAPTER: str = '''\
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter


class DuckDbUnrecordedAdapter(DuckDbAdapter):
    """Run statements without recording them as running."""

    adapter_name = "duckdb_unrecorded"

    def execute(self, *, connection, sql):
        return self._execute(connection=connection, sql=sql)
'''


@pytest.mark.skipif(sys.platform == "win32", reason="Unix signals")
@pytest.mark.parametrize(
    "test_case",
    [
        InterruptedStepsTestCase(
            description="one finished, one finishing in the grace and one ignoring the cancel",
            expected_status="partial",
            expected_statuses={
                "asset:fulfil[pick]": "success",
                "asset:fulfil[label]": "success",
                "asset:fulfil[pack]": "cancelled",
            },
            expected_notice="Stopped waiting for fulfil[pack] after 5 seconds",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_only_steps_when_the_worker_gets_one_ctrl_c_then_each_is_reported_before_exit(
    test_case: InterruptedStepsTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    execution: WorkerExecutionTestCase = WorkerExecutionTestCase(
        description=test_case.description,
        model_sql="SELECT 1 AS amount",
        warehouse_query="",
        expected_status=test_case.expected_status,
        expected_rows=[],
        files={
            "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb_unrecorded"\n'
            '[connections.local]\ndatabase = "warehouse.duckdb"\n'
            '[targets.development]\nschema = "main"\n',
            "python/adapters/duckdb_unrecorded.py": _ADAPTER,
            "python/nodes/steps.py": _STEPS.format(directory=str(tmp_path)),
            "python/providers/marker.py": _PROVIDER.format(directory=str(tmp_path)),
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
    with scheduled_worker(tmp_path, monkeypatch, execution) as worker:
        started: Path = tmp_path / "pack.started"
        deadline: float = time.monotonic() + 40
        with sqlite3.connect(worker.control_store) as control:
            picked: str = (
                "SELECT count(*) FROM sqb_run_nodes "
                "WHERE node_key='asset:fulfil[pick]' AND status='success'"
            )
            while not (
                control.execute(picked).fetchone()[0]
                and (tmp_path / "label.started").exists()
                and started.exists()
                and started.read_text()
            ):
                assert time.monotonic() < deadline, worker.log.read_text()
                time.sleep(0.05)
            worker_pid: int = int(started.read_text())
            os.kill(worker_pid, signal.SIGINT)
            (tmp_path / "pressed").touch()
            run: dict[str, Any] = wait_for_run(
                worker.api, worker.process, worker.log, job_key="annual_orders_job"
            )
            # The worker's exit hooks run after its last report, and must tear nothing down either.
            exits_by: float = time.monotonic() + 20
            while Path(f"/proc/{worker_pid}/cmdline").exists():
                assert time.monotonic() < exits_by, worker.log.read_text()
                time.sleep(0.05)
            statuses: dict[str, str] = dict(
                control.execute(
                    "SELECT node_key, status FROM sqb_run_nodes "
                    "WHERE run_id=? AND kind='node' AND node_key LIKE 'asset:%'",
                    (run["run_id"],),
                ).fetchall()
            )

    assert statuses == test_case.expected_statuses, worker.log.read_text()
    assert run["status"] == test_case.expected_status, worker.log.read_text()
    assert test_case.expected_notice in worker.log.read_text()
    assert not (tmp_path / f"torn-down.{worker_pid}").exists()
