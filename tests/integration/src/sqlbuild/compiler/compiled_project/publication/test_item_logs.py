"""A worker files each item's `ctx.log` lines under that item, whatever its outcome and path."""

import sqlite3
from itertools import product
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    ItemLogsTestCase,
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    INGRESS_JOB_SETTINGS,
    INGRESS_LOADER_FILES,
    scheduled_worker,
    wait_for_run,
)

_LOGGING: str = '''\
import threading

from sqlbuild.assets import AssetContext, asset
from sqlbuild.discovery import Discovered, Item, discover
from sqlbuild.refs import model

OUTCOMES = {outcomes!r}
MEET = threading.Barrier({max_parallel}, timeout=30)


@discover
def fulfilment_plan() -> Discovered:
    """Two steps."""
    return Discovered(items=[Item(id=step) for step in OUTCOMES], max_parallel={max_parallel})


@asset(for_each=fulfilment_plan, depends_on={depends_on}, tags=["fulfilment"])
def fulfil(ctx: AssetContext):
    """Log at every level beside the other step, then end as planned."""
    step = ctx.discovered.item
    MEET.wait()
    ctx.log.info(f"{{step}} info")
    ctx.log.warning(f"{{step}} warning")
    ctx.log.error(f"{{step}} error")
    MEET.wait()
    if OUTCOMES[step] == "failed":
        raise ValueError("the step failed")
    if OUTCOMES[step] == "skipped":
        return ctx.skip(reason="nothing to do")
'''


@pytest.mark.parametrize(
    "test_case",
    [
        ItemLogsTestCase(
            description="two at once on the read side, one failing",
            depends_on='model("orders")',
            job_select="orders fulfil",
            job_settings="",
            max_parallel=2,
            outcomes={"pack": "success", "pick": "failed"},
            files={},
            expected_statuses={"asset:fulfil[pack]": "success", "asset:fulfil[pick]": "failed"},
        ),
        ItemLogsTestCase(
            description="two at once on the read side, one skipping",
            depends_on="()",
            job_select="fulfil",
            job_settings="",
            max_parallel=2,
            outcomes={"pack": "skipped", "pick": "success"},
            files={},
            expected_statuses={"asset:fulfil[pack]": "skipped", "asset:fulfil[pick]": "success"},
        ),
        ItemLogsTestCase(
            description="two at once in ingress, one failing",
            depends_on="()",
            job_select="+raw_steps",
            job_settings=INGRESS_JOB_SETTINGS,
            max_parallel=2,
            outcomes={"pack": "failed", "pick": "success"},
            files=INGRESS_LOADER_FILES,
            expected_statuses={"asset:fulfil[pack]": "failed", "asset:fulfil[pick]": "success"},
        ),
        ItemLogsTestCase(
            description="two at once in ingress, one skipping",
            depends_on="()",
            job_select="+raw_steps",
            job_settings=INGRESS_JOB_SETTINGS,
            max_parallel=2,
            outcomes={"pack": "success", "pick": "skipped"},
            files=INGRESS_LOADER_FILES,
            expected_statuses={"asset:fulfil[pack]": "success", "asset:fulfil[pick]": "skipped"},
        ),
        ItemLogsTestCase(
            description="one by one on the read side",
            depends_on='model("orders")',
            job_select="orders fulfil",
            job_settings="",
            max_parallel=1,
            outcomes={"pack": "failed", "pick": "skipped"},
            files={},
            expected_statuses={"asset:fulfil[pack]": "failed", "asset:fulfil[pick]": "skipped"},
        ),
        ItemLogsTestCase(
            description="one by one in ingress",
            depends_on="()",
            job_select="+raw_steps",
            job_settings=INGRESS_JOB_SETTINGS,
            max_parallel=1,
            outcomes={"pack": "success", "pick": "failed"},
            files=INGRESS_LOADER_FILES,
            expected_statuses={"asset:fulfil[pack]": "success", "asset:fulfil[pick]": "failed"},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_items_logging_at_every_level_when_a_worker_runs_them_then_each_keeps_its_own_lines(
    test_case: ItemLogsTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    steps: str = _LOGGING.format(
        outcomes=test_case.outcomes,
        max_parallel=test_case.max_parallel,
        depends_on=test_case.depends_on,
    )
    execution: WorkerExecutionTestCase = WorkerExecutionTestCase(
        description=test_case.description,
        model_sql="SELECT 1 AS amount",
        warehouse_query="",
        expected_status="",
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
        run: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="annual_orders_job"
        )
        with sqlite3.connect(worker.control_store) as control:
            statuses: dict[str, str] = dict(
                control.execute(
                    "SELECT node_key, status FROM sqb_run_nodes "
                    "WHERE run_id=? AND kind='node' AND node_key LIKE 'asset:fulfil[%'",
                    (run["run_id"],),
                ).fetchall()
            )
            logs: list[tuple[str, str, str]] = control.execute(
                "SELECT node_key, level, message FROM sqb_run_node_logs "
                "WHERE run_id=? AND node_key LIKE 'asset:%' ORDER BY node_key, level",
                (run["run_id"],),
            ).fetchall()
        output: str = worker.log.read_text()

    assert statuses == test_case.expected_statuses, output
    assert logs == [
        (f"asset:fulfil[{step}]", level, f"{step} {level}")
        for step, level in product(sorted(test_case.outcomes), ("error", "info", "warning"))
    ]
