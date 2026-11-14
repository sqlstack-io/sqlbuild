"""A narrowed job run executes only the job's selected nodes and is recorded as a run of that job."""

import json
import sqlite3
import subprocess
import sys
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
            description="an operator excludes one model from a published job",
            model_sql="SELECT 1 AS amount",
            warehouse_query="SELECT table_name FROM duckdb_tables() ORDER BY table_name",
            expected_status="success",
            expected_rows=[],
            files={
                "models/customers.sql": "MODEL (description 'Customers.', materialized table);\nSELECT 7 AS customer_id\n",
                "models/products.sql": "MODEL (description 'Products.', materialized table);\nSELECT 3 AS product_id\n",
                "orchestration/jobs/nightly_job.toml": (
                    'name = "nightly_job"\n'
                    'description = "Orders and customers."\n'
                    'select = "orders customers"\n'
                ),
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_published_job_when_an_operator_excludes_a_model_then_only_the_rest_of_the_job_runs(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        executable: str = str(Path(sys.executable).with_name("sqb"))
        request_run: list[str] = [
            executable,
            "admin",
            "--database-env",
            "SQLBUILD_SCHEDULER_TEST_STORE",
            "request-run",
            worker.api.location_id,
            "nightly_job",
            "--target",
            "development",
            "--reason",
            "Rebuild the job without customers",
        ]
        refused: subprocess.CompletedProcess[str] = subprocess.run(
            [*request_run, "--run-id", str(uuid4()), "--select", "products"],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
        assert refused.returncode != 0
        assert "leave none of the job's nodes" in refused.stderr
        run_id: str = str(uuid4())
        requested: subprocess.CompletedProcess[str] = subprocess.run(
            [*request_run, "--run-id", run_id, "--exclude", "customers"],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
        assert requested.returncode == 0, requested.stderr
        assert json.loads(requested.stdout) == {"run_id": run_id, "applied": True}
        completed: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, run_id=run_id
        )
        assert completed["status"] == test_case.expected_status, worker.log.read_text()
        assert completed["job_key"] == "nightly_job"
        with duckdb.connect(str(worker.project / "warehouse.duckdb"), read_only=True) as warehouse:
            assert warehouse.sql(
                "SELECT table_name FROM duckdb_tables() "
                "WHERE NOT internal AND NOT starts_with(table_name, '_sqlbuild') ORDER BY 1"
            ).fetchall() == [("orders",)]
        with sqlite3.connect(worker.control_store) as control:
            rows: list[tuple[str, str, str]] = control.execute(
                "SELECT kind, selection_json, authored_json FROM sqb_runs"
            ).fetchall()
        assert len(rows) == 1
        kind, selection, authored = rows[0]
        assert (kind, json.loads(selection)) == ("job", ["model:orders"])
        assert json.loads(authored)["request"]["exclude"] == ["customers"]
