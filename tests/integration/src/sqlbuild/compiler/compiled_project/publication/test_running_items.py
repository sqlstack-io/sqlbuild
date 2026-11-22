"""A step reads `running` on the server while it runs, and one waiting for a slot doesn't."""

import sqlite3
import time
from collections import Counter
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    RunningItemsTestCase,
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    INGRESS_JOB_SETTINGS,
    INGRESS_LOADER_FILES,
    scheduled_worker,
    wait_for_run,
)

_HELD: str = '''\
import time
from pathlib import Path

from sqlbuild.assets import AssetContext, asset
from sqlbuild.discovery import Discovered, Item, discover
from sqlbuild.refs import model

DIRECTORY = Path({directory!r})
STEPS = ("label", "pack", "pick")


@discover
def fulfilment_plan() -> Discovered:
    """Three steps."""
    return Discovered(items=[Item(id=step) for step in STEPS], max_parallel={max_parallel})


@asset(for_each=fulfilment_plan, depends_on={depends_on}, tags=["fulfilment"])
def fulfil(ctx: AssetContext) -> None:
    """Hold the slot until released."""
    (DIRECTORY / f"{{ctx.discovered.item}}.running").touch()
    deadline = time.monotonic() + 60
    while not (DIRECTORY / "release").exists() and time.monotonic() < deadline:
        time.sleep(0.05)
'''


@pytest.mark.parametrize(
    "test_case",
    [
        RunningItemsTestCase(
            description="two slots on the read side",
            depends_on='model("orders")',
            job_select="orders fulfil",
            job_settings="",
            max_parallel=2,
            files={},
            expected_running=2,
            expected_pending=1,
        ),
        RunningItemsTestCase(
            description="two slots in ingress",
            depends_on="()",
            job_select="+raw_steps",
            job_settings=INGRESS_JOB_SETTINGS,
            max_parallel=2,
            files=INGRESS_LOADER_FILES,
            expected_running=2,
            expected_pending=1,
        ),
        RunningItemsTestCase(
            description="one by one on the read side",
            depends_on="()",
            job_select="fulfil",
            job_settings="",
            max_parallel=1,
            files={},
            expected_running=1,
            expected_pending=2,
        ),
        RunningItemsTestCase(
            description="one by one in ingress",
            depends_on="()",
            job_select="+raw_steps",
            job_settings=INGRESS_JOB_SETTINGS,
            max_parallel=1,
            files=INGRESS_LOADER_FILES,
            expected_running=1,
            expected_pending=2,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_running_steps_when_reading_the_server_then_only_those_with_a_slot_read_running(
    test_case: RunningItemsTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    steps: str = _HELD.format(
        directory=str(tmp_path),
        max_parallel=test_case.max_parallel,
        depends_on=test_case.depends_on,
    )
    execution: WorkerExecutionTestCase = WorkerExecutionTestCase(
        description=test_case.description,
        model_sql="SELECT 1 AS amount",
        warehouse_query="",
        expected_status="success",
        expected_rows=[],
        files={"python/steps.py": steps} | test_case.files,
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
        deadline: float = time.monotonic() + 40
        while len(list(tmp_path.glob("*.running"))) < test_case.expected_running:
            assert time.monotonic() < deadline, worker.log.read_text()
            time.sleep(0.05)
        with sqlite3.connect(worker.control_store) as control:
            during: dict[str, str] = dict(
                control.execute(
                    "SELECT node_key, status FROM sqb_run_nodes "
                    "WHERE kind='node' AND node_key LIKE 'asset:fulfil[%'"
                ).fetchall()
            )
        held: set[str] = {f"asset:fulfil[{marker.stem}]" for marker in tmp_path.glob("*.running")}
        (tmp_path / "release").touch()
        run: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="annual_orders_job"
        )

    assert Counter(during.values()) == Counter(
        running=test_case.expected_running, pending=test_case.expected_pending
    )
    assert {during[key] for key in held} == {"running"}
    assert run["status"] == execution.expected_status
