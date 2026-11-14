"""Pinned group retries rebuild failed and skipped nodes without repeating successful writes."""

import json
import sqlite3
import subprocess
import sys
from dataclasses import replace
from pathlib import Path
from typing import Any
from uuid import uuid4

import duckdb
import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    scheduled_worker,
    wait_for_run,
)


@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            description="local group retries only failed and skipped nodes",
            model_sql="SELECT 11 AS amount",
            warehouse_query="SELECT amount FROM orders UNION ALL SELECT amount FROM totals ORDER BY amount",
            expected_status="success",
            expected_rows=[(17,), (99,)],
        ),
        WorkerExecutionTestCase(
            description="Docker group retries only failed and skipped nodes",
            model_sql="SELECT 11 AS amount",
            warehouse_query="SELECT amount FROM orders UNION ALL SELECT amount FROM totals ORDER BY amount",
            expected_status="success",
            expected_rows=[(17,), (99,)],
            launcher="docker",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_partial_group_when_retrying_from_failure_then_successful_nodes_are_not_rewritten(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    test_case = replace(
        test_case,
        files={
            "models/broken_orders.sql": Path(
                "benchmarks/duckshop/edge_cases/orchestration/rerun_from_failure.sql"
            ).read_text(),
            "models/totals.sql": (
                "MODEL (description 'Downstream totals.', materialized table);\n"
                'SELECT amount FROM __ref("broken_orders")\n'
            ),
            "models/inventory.sql": (
                "MODEL (description 'Independent inventory.', materialized table);\n"
                "SELECT 29 AS amount\n"
            ),
            "orchestration/jobs/staging_job.toml": (
                'name = "staging_job"\n'
                'description = "Stage orders."\n'
                'select = "orders broken_orders"\n'
            ),
            "orchestration/jobs/marts_job.toml": (
                'name = "marts_job"\ndescription = "Build totals."\nselect = "totals"\n'
            ),
            "orchestration/jobs/stock_job.toml": (
                'name = "stock_job"\ndescription = "Build inventory."\nselect = "inventory"\n'
            ),
            "orchestration/job_groups/annual_orders_job_group.toml": (
                'name = "annual_orders_job_group"\n'
                'description = "Retry boundaries."\n'
                'jobs = ["stock_job", "marts_job", "staging_job"]\n'
                'schedule = { cron = "0 0 * * *", timezone = "UTC", starts_at = "2000-01-01T00:00:00Z" }\n'
            ),
        },
        default_job_files={},
    )
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        source: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="annual_orders_job_group"
        )
        assert source["status"] == "partial", worker.log.read_text()
        with duckdb.connect(str(worker.project / "warehouse.duckdb")) as warehouse:
            warehouse.execute("CREATE TABLE retry_ready AS SELECT 1 AS id")
            warehouse.execute("UPDATE orders SET amount=99")
            warehouse.execute("UPDATE inventory SET amount=71")
        run_id: str = str(uuid4())
        command: list[str] = [
            str(Path(sys.executable).with_name("sqb")),
            "admin",
            "--database-env",
            "SQLBUILD_SCHEDULER_TEST_STORE",
            "rerun-run",
            source["run_id"],
            "--run-id",
            run_id,
            "--from-failure",
            "--reason",
            "Retry failed and skipped nodes after fixing warehouse input.",
        ]
        for expected_applied in [True, False]:
            requested: subprocess.CompletedProcess[str] = subprocess.run(
                command, capture_output=True, text=True, timeout=10, check=False
            )
            assert requested.returncode == 0, requested.stderr
            assert json.loads(requested.stdout) == {"run_id": run_id, "applied": expected_applied}
        completed: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, run_id=run_id
        )
        assert completed["status"] == test_case.expected_status, worker.log.read_text()
        assert completed["snapshot_id"] == source["snapshot_id"]
        assert completed["selection_json"] == ["model:broken_orders", "model:totals"]
        assert completed["node_counts_json"] == {"success": 2}
        with sqlite3.connect(worker.control_store) as control:
            children: list[tuple[str, str]] = control.execute(
                "SELECT job_key, selection_json FROM sqb_runs WHERE parent_run_id=? ORDER BY job_key",
                (run_id,),
            ).fetchall()
            assert [(name, json.loads(selection)) for name, selection in children] == [
                ("marts_job", ["model:totals"]),
                ("staging_job", ["model:broken_orders"]),
            ]
            assert control.execute(
                "SELECT count(*) FROM sqb_security_audit WHERE action='run.rerun_requested'"
            ).fetchone() == (1,)
            assert control.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (0,)
        with duckdb.connect(str(worker.project / "warehouse.duckdb"), read_only=True) as warehouse:
            assert warehouse.sql(test_case.warehouse_query).fetchall() == test_case.expected_rows
            assert warehouse.sql("SELECT amount FROM inventory").fetchall() == [(71,)]
