"""Explicit backfills and their reruns execute the same pinned window through real workers."""

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
    BackfillExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    scheduled_worker,
    wait_for_run,
)


@pytest.mark.parametrize(
    "test_case",
    [
        BackfillExecutionTestCase(
            description="local integer window overrides authored bounds and survives group rerun",
            model_sql="SELECT 1 AS amount",
            warehouse_query="SELECT amount FROM orders ORDER BY amount",
            expected_status="success",
            expected_rows=[(23,), (37,)],
            files={
                "models/orders.sql": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/backfill_integer.sql"
                ).read_text()
            },
            window_arguments=("--start-cursor-int", "3", "--end-cursor-int", "6"),
            expected_window={"kind": "integer", "start": 3, "end": 6},
        ),
        BackfillExecutionTestCase(
            description="Docker timestamp window normalizes offsets and survives group rerun",
            model_sql="SELECT 1 AS amount",
            warehouse_query="SELECT amount FROM orders ORDER BY amount",
            expected_status="success",
            expected_rows=[(23,), (37,)],
            launcher="docker",
            files={
                "models/orders.sql": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/backfill_timestamp.sql"
                ).read_text()
            },
            window_arguments=(
                "--start-cursor-ts",
                "2026-07-03T05:30:00+05:30",
                "--end-cursor-ts",
                "2026-07-04T05:30:00+05:30",
            ),
            expected_window={
                "kind": "timestamp",
                "start": "2026-07-03T00:00:00Z",
                "end": "2026-07-04T00:00:00Z",
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_explicit_backfill_bounds_when_running_and_rerunning_a_group_then_only_its_frozen_window_is_written(
    test_case: BackfillExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    test_case = replace(
        test_case,
        files=test_case.files
        | {
            "sources/raw_orders.yml": "sources:\n  - name: raw_orders\n    description: Synthetic order windows.\n    schema: main\n    table: raw_orders\n",
            "orchestration/jobs/windowed_orders_job.toml": (
                'name = "windowed_orders_job"\n'
                'description = "Windowed orders."\n'
                'select = "orders"\n'
                "\n"
                "[settings]\n"
                'start_cursor = "0"\n'
                'end_cursor = "1"\n'
            ),
            "orchestration/job_groups/annual_orders_job_group.toml": (
                'name = "annual_orders_job_group"\n'
                'description = "Windowed group."\n'
                'jobs = ["windowed_orders_job"]\n'
            ),
        },
        default_job_files={},
    )
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        with duckdb.connect(str(worker.project / "warehouse.duckdb")) as warehouse:
            warehouse.execute(
                "CREATE TABLE raw_orders AS SELECT * FROM (VALUES (11, 2, TIMESTAMP '2026-07-02'), (23, 3, TIMESTAMP '2026-07-03'), (37, 6, TIMESTAMP '2026-07-04'), (41, 7, TIMESTAMP '2026-07-05')) t(amount, batch_id, ordered_at)"
            )
        executable: str = str(Path(sys.executable).with_name("sqb"))
        run_id: str = str(uuid4())
        arguments: list[str] = [
            executable,
            "admin",
            "--database-env",
            "SQLBUILD_SCHEDULER_TEST_STORE",
            "request-run",
            worker.api.location_id,
            "annual_orders_job_group",
            "--target",
            "development",
            "--run-id",
            run_id,
            "--reason",
            "Backfill an explicit order window",
            *test_case.window_arguments,
        ]
        rejected: subprocess.CompletedProcess[str] = subprocess.run(
            arguments[:-2], capture_output=True, text=True, timeout=10, check=False
        )
        assert rejected.returncode != 0
        assert "both start and end of exactly one cursor type" in rejected.stderr
        with sqlite3.connect(worker.control_store) as control:
            assert control.execute("SELECT count(*) FROM sqb_runs").fetchone() == (0,)
        for expected in [True, False]:
            requested: subprocess.CompletedProcess[str] = subprocess.run(
                arguments, capture_output=True, text=True, timeout=10, check=False
            )
            assert requested.returncode == 0, requested.stderr
            assert json.loads(requested.stdout) == {"run_id": run_id, "applied": expected}
        completed: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, run_id=run_id
        )
        assert completed["status"] == test_case.expected_status, worker.log.read_text()
        with duckdb.connect(str(worker.project / "warehouse.duckdb")) as warehouse:
            assert warehouse.sql(test_case.warehouse_query).fetchall() == test_case.expected_rows
            warehouse.execute("UPDATE raw_orders SET amount=amount+100")
        retry_id: str = str(uuid4())
        retried: subprocess.CompletedProcess[str] = subprocess.run(
            [
                executable,
                "admin",
                "--database-env",
                "SQLBUILD_SCHEDULER_TEST_STORE",
                "rerun-run",
                run_id,
                "--run-id",
                retry_id,
                "--reason",
                "Repeat the same explicit order window",
            ],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
        assert retried.returncode == 0, retried.stderr
        retry: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, run_id=retry_id
        )
        assert retry["status"] == test_case.expected_status, worker.log.read_text()
        with duckdb.connect(str(worker.project / "warehouse.duckdb"), read_only=True) as warehouse:
            assert warehouse.sql(test_case.warehouse_query).fetchall() == [(123,), (137,)]
        with sqlite3.connect(worker.control_store) as control:
            requests: list[dict[str, Any]] = [
                json.loads(row[0])
                for row in control.execute("SELECT request_json FROM sqb_runs").fetchall()
            ]
            assert len(requests) == 4
            assert all(request["window"] == test_case.expected_window for request in requests)
            assert {request["origin"] for request in requests} == {"backfill", "rerun"}
            assert control.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (0,)
            assert control.execute(
                "SELECT count(*) FROM sqb_security_audit WHERE action='job.run_requested'"
            ).fetchone() == (1,)
