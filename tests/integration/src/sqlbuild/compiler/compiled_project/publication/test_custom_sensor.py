"""A real location callback times out, restarts and queues a real DuckDB worker."""

import ast
import json
import os
import sqlite3
import sys
from pathlib import Path
from typing import Any, cast

import duckdb
import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    location_process,
    scheduled_worker,
    sensor_calls,
    wait_for_generation,
    wait_for_location_work,
    wait_for_run,
    wait_for_sensor_phase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.person.helpers import (
    person_login,
    person_read,
    person_request,
    provision_person,
    verify_compute_transport,
)


@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "polling fulfillment sensor",
            "SELECT 83 AS amount",
            "SELECT amount FROM orders",
            "success",
            [(83,)],
            expected_origin="sensor",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/custom_orders.py"
                ).read_text(),
                "providers/fulfillment.py": (
                    "import json, os, time\nfrom pathlib import Path\n"
                    "from sqlbuild.orchestration import RunRequest, SkipReason\n"
                    "from sqlbuild.provider.classes.provider import Provider\n"
                    "class Fulfillment(Provider):\n"
                    '    """Synthetic fulfillment readiness."""\n'
                    "    def record(self, value):\n"
                    "        with Path(os.environ['SQLBUILD_SENSOR_CALLS']).open('a') as out:\n"
                    "            out.write(json.dumps(value | {'pid': os.getpid()}) + '\\n')\n"
                    "    def setup(self, ctx):\n        self.record({'phase': 'setup'})\n"
                    "    def teardown(self):\n        self.record({'phase': 'teardown'})\n"
                    "    def poll(self, ctx):\n"
                    "        mode = json.loads(Path(os.environ['SQLBUILD_FULFILLMENT_DATA']).read_text())['mode']\n"
                    "        self.record({'phase': 'evaluate', 'cursor': ctx.cursor, 'evaluation_id': ctx.evaluation_id, 'target': ctx.target, 'mode': mode})\n"
                    "        if mode == 'failed':\n"
                    "            ctx.cursor = 'uncommitted'\n"
                    "            raise ValueError('synthetic source unavailable')\n"
                    "        if mode == 'timeout':\n            time.sleep(90)\n"
                    "        if ctx.cursor:\n            return SkipReason('No new orders')\n"
                    "        ctx.target = 'not-an-authority-input'\n"
                    "        ctx.evaluation_id = 'not-an-authority-input'\n"
                    "        ctx.cursor = 'orders/page-1'\n"
                    "        return [RunRequest(run_key='orders-1'), RunRequest(run_key='orders-1')]\n"
                ),
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_custom_polling_sensor_when_a_callback_fails_or_times_out_then_a_fresh_process_retries_without_advancing_its_cursor(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    context_module: ast.Module = ast.parse(
        Path("src/sqlbuild/orchestration_declarations/models/sensor_context.py").read_text()
    )
    assert [type(node).__name__ for node in context_module.body] == [
        "Expr",
        "ImportFrom",
        "ClassDef",
    ]
    declaration: ast.ClassDef = cast(ast.ClassDef, context_module.body[-1])
    assert declaration.name == "SensorContext"
    assert [type(node).__name__ for node in declaration.body] == [
        "Expr",
        "AnnAssign",
        "AnnAssign",
        "AnnAssign",
    ]
    data: Path = tmp_path / "fulfillment.json"
    calls: Path = tmp_path / "sensor-calls.jsonl"
    data.write_text(json.dumps({"mode": "failed"}))
    monkeypatch.setenv("SQLBUILD_FULFILLMENT_DATA", str(data))
    monkeypatch.setenv("SQLBUILD_SENSOR_CALLS", str(calls))
    with scheduled_worker(
        tmp_path,
        monkeypatch,
        test_case,
        server_arguments=("--password-origin", "https://control.example.com"),
    ) as worker:
        administrator: dict[str, Any] = provision_person(
            tmp_path / "control", "create-user", "orders-admin"
        )
        person: dict[str, Any] = provision_person(
            tmp_path / "control", "create-principal", "orders-developer"
        )
        admin: tuple[str, str] = person_login(worker.api, administrator["name"])
        session: tuple[str, str] = person_login(worker.api, person["name"])
        assert person_request(
            worker.api,
            "/api/admin/roles",
            body={
                "principal_id": person["principal_id"],
                "role": "developer",
                "location_id": worker.api.location_id,
                "target": "development",
            },
            session=admin,
        ) == {"applied": True}
        monkeypatch.setenv("SQLBUILD_LOCATION_TOKEN", worker.api.bearer)
        monkeypatch.setenv("SQLBUILD_CONTROL_DATABASE_URL", "unsupported://must-not-open")
        log: Path = tmp_path / "location.log"
        with location_process(
            [
                str(Path(sys.executable).with_name("sqb")),
                "--project-dir",
                str(worker.project),
                "location",
                "serve",
                "--server",
                worker.api.url,
                "--location-id",
                worker.api.location_id,
                "--target",
                "development",
            ],
            log,
            tmp_path / "publications.jsonl",
        ) as location:
            _ = wait_for_generation(worker.api, "development", 2, location, log)
            failed: dict[str, Any] = wait_for_sensor_phase(worker.control_store, "failed")
            assert failed["request"]["cursor"] == {"event_storage_id": 0}
            work: dict[str, Any] = wait_for_location_work(
                worker.api, failed["request"]["evaluation_id"], location, log
            )
            assert work["request_json"]["work"]["kind"] == "evaluate_sensor"
            assert work["request_json"]["request_id"] == failed["request"]["evaluation_id"]
            path: str = (
                f"/api/control/locations/{worker.api.location_id}/targets/development/connection-tests/"
                + failed["request"]["evaluation_id"]
            )
            for browser in [admin, session]:
                assert person_read(worker.api, path, session=browser) is None
            data.write_text(json.dumps({"mode": "timeout"}))
            timed_out: dict[str, Any] = wait_for_sensor_phase(worker.control_store, "timed_out")
            assert timed_out["request"]["cursor"] == failed["request"]["cursor"]
            timed_call: dict[str, Any] = sensor_calls(
                calls, {"evaluation_id": timed_out["request"]["evaluation_id"]}
            )[0]
            with pytest.raises(ProcessLookupError):
                os.kill(timed_call["pid"], 0)
            data.write_text(json.dumps({"mode": "ready"}))
            destination: dict[str, Any] = wait_for_run(
                worker.api, worker.process, worker.log, job_key="annual_orders"
            )
            assert destination["status"] == test_case.expected_status, log.read_text()
            for browser in [admin, session]:
                verify_compute_transport(worker.api, browser, destination["run_id"])
            # The next successful evaluation skips but retains its committed cursor.
            skipped: dict[str, Any] = wait_for_sensor_phase(
                worker.control_store, "completed", empty_runs=True
            )
            assert skipped["outcome"]["cursor"]["custom"] == "orders/page-1"
            evaluated: list[dict[str, Any]] = sensor_calls(calls, {"phase": "evaluate"})
            assert {record["target"] for record in evaluated} == {"development"}
            assert len({record["pid"] for record in evaluated}) == len(evaluated)
            assert {
                record["cursor"]
                for record in sensor_calls(calls, {"phase": "evaluate", "mode": "failed"})
            } == {None}
            assert {
                record["cursor"]
                for record in sensor_calls(calls, {"phase": "evaluate", "mode": "timeout"})
            } == {None}
            assert location.poll() is None and worker.process.poll() is None
            with sqlite3.connect(worker.control_store) as control:
                assert control.execute(
                    "SELECT run_key,run_id FROM sqb_run_requests"
                ).fetchall() == [("orders-1", destination["run_id"])]
                assert control.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (0,)
            with duckdb.connect(
                str(worker.project / "warehouse.duckdb"), read_only=True
            ) as warehouse:
                assert (
                    warehouse.sql(test_case.warehouse_query).fetchall() == test_case.expected_rows
                )
        assert not (worker.project / ".sqlbuild/control.sqlite3").exists()
        assert worker.api.bearer not in log.read_text()
