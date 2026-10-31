"""A published schedule executes in a real pinned container, not a mocked launcher."""

import signal
import sqlite3
import sys
import time
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    ScheduledWorker,
    WorkerExecutionTestCase,
    WorkerRestartTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.docker.helpers import (
    REQUIRES_DOCKER,
    docker_command,
    worker_container,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    cancel_running_statement,
    location_process,
    running_statement,
    scheduled_worker,
    wait_for_log,
    wait_for_run,
    worker_result,
)


@REQUIRES_DOCKER
@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "pinned container inherits project context but not privileged credentials",
            "SELECT CAST(value AS BIGINT) AS amount FROM (VALUES ('37')) t(value)",
            "SELECT amount FROM orders",
            "success",
            [(37,)],
            launcher="docker",
            files={
                "orchestration/orders.py": (
                    "import os\nfrom sqlbuild.orchestration import job, schedule\n"
                    "if 'SQLBUILD_RUN_TOKEN' in os.environ:\n"
                    "    assert os.getenv('SQLBUILD_WORKER_TEST_VALUE') == 'orders-context'\n"
                    "    for name in ('SQLBUILD_LOCATION_TOKEN', 'SQLBUILD_CONTROL_DATABASE_URL', "
                    "'SQLBUILD_SCHEDULER_TEST_STORE'):\n"
                    "        assert name not in os.environ, name\n"
                    'annual_orders = job("annual_orders", select="orders", '
                    'description="Build orders.", '
                    'schedule=schedule(cron="0 0 1 1 *", timezone="UTC", '
                    'starts_at="2000-01-01T00:00:00Z"))\n'
                )
            },
        ),
        WorkerExecutionTestCase(
            "container warehouse error retains failed node evidence",
            "SELECT CAST(value AS BIGINT) AS amount FROM (VALUES ('invalid')) t(value)",
            "SELECT count(*) FROM duckdb_tables() WHERE table_name = 'orders'",
            "failed",
            [(0,)],
            launcher="docker",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_scheduled_docker_job_when_published_then_a_pinned_worker_executes_and_reports(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        run: dict[str, Any] = worker_result(worker, test_case)
        assert run["status"] == test_case.expected_status, worker.log.read_text()
        assert worker.docker is not None
        deadline: float = time.monotonic() + 10
        state: dict[str, object] = worker_container(
            worker.docker, worker.api.location_id, run["run_id"]
        )
        while state["Running"]:
            assert time.monotonic() < deadline, "Terminal worker did not physically exit"
            time.sleep(0.02)
            state = worker_container(worker.docker, worker.api.location_id, run["run_id"])
        assert state["ExitCode"] == {"success": 0, "failed": 1}[test_case.expected_status]


@REQUIRES_DOCKER
@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "container refuses changed source before writing",
            "SELECT 23 AS amount",
            "",
            "failed",
            [],
            launcher="docker",
            files_after_publication={
                "models/orders.sql": "MODEL (description 'Orders.', materialized table);\nSELECT 41 AS amount\n"
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_pinned_snapshot_when_container_source_drifts_then_no_warehouse_write_is_permitted(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        run: dict[str, Any] = wait_for_run(worker.api, worker.process, worker.log)
        assert run["status"] == test_case.expected_status, worker.log.read_text()
        assert run["node_counts_json"] == {"skipped": 1}
        assert not (worker.project / "warehouse.duckdb").exists()
        assert worker.docker is not None
        name: str = f"sqlbuild-{worker.api.location_id}-{run['run_id']}"
        assert "node content differs" in docker_command(worker.docker.socket, "logs", name)
        assert (
            worker_container(worker.docker, worker.api.location_id, run["run_id"])["ExitCode"] == 1
        )
        with sqlite3.connect(worker.control_store) as control:
            assert control.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (0,)


@REQUIRES_DOCKER
@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "Docker cancellation stops an active warehouse statement",
            "SELECT sum(sin(value)) AS amount FROM range(10000000000) t(value)",
            "SELECT count(*) FROM duckdb_tables() WHERE table_name = 'orders'",
            "cancelled",
            [(0,)],
            launcher="docker",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_an_executing_container_when_cancelled_then_the_statement_stops_before_claim_release(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        cancel_running_statement(worker)
        run: dict[str, Any] = worker_result(worker, test_case)
        assert run["status"] == test_case.expected_status, worker.log.read_text()


@REQUIRES_DOCKER
@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "killed Docker worker preserves its physical exit code",
            "SELECT sum(sin(value)) AS amount FROM range(10000000000) t(value)",
            "SELECT count(*) FROM duckdb_tables() WHERE table_name = 'orders'",
            "failed",
            [(0,)],
            launcher="docker",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_an_executing_container_when_killed_then_the_monitor_records_exit_and_releases_claims(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        active: dict[str, Any] = running_statement(worker)
        assert worker.docker is not None
        name: str = f"sqlbuild-{worker.api.location_id}-{active['run_id']}"
        _ = docker_command(worker.docker.socket, "kill", "--signal", "KILL", name)
        run: dict[str, Any] = worker_result(worker, test_case)
        assert run["status"] == test_case.expected_status, worker.log.read_text()
        assert (
            worker_container(worker.docker, worker.api.location_id, run["run_id"])["ExitCode"]
            == 137
        )
        with sqlite3.connect(worker.control_store) as control:
            assert control.execute(
                "SELECT json_extract(payload, '$.change.change.exit_code') FROM recall_events "
                "WHERE event_type='run_terminated'"
            ).fetchone() == (137,)


@REQUIRES_DOCKER
@pytest.mark.parametrize(
    "test_case",
    [
        WorkerRestartTestCase(
            "graceful scheduler restart retains the executing container",
            "SELECT sum(sin(value)) AS amount FROM range(10000000000) t(value)",
            "SELECT count(*) FROM duckdb_tables() WHERE table_name = 'orders'",
            "cancelled",
            [(0,)],
            launcher="docker",
            shutdown_signal=signal.SIGTERM,
            expected_scheduler_exit=0,
        ),
        WorkerRestartTestCase(
            "abrupt scheduler death retains the executing container",
            "SELECT sum(sin(value)) AS amount FROM range(10000000000) t(value)",
            "SELECT count(*) FROM duckdb_tables() WHERE table_name = 'orders'",
            "cancelled",
            [(0,)],
            launcher="docker",
            shutdown_signal=signal.SIGKILL,
            expected_scheduler_exit=-signal.SIGKILL,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_executing_container_when_scheduler_restarts_then_the_same_worker_is_adopted_without_relaunch(
    test_case: WorkerRestartTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        active: dict[str, Any] = running_statement(worker)
        assert worker.docker is not None
        before: dict[str, object] = worker_container(
            worker.docker, worker.api.location_id, active["run_id"]
        )
        worker.process.send_signal(test_case.shutdown_signal)
        assert worker.process.wait(timeout=10) == test_case.expected_scheduler_exit, (
            worker.log.read_text()
        )
        assert worker_container(worker.docker, worker.api.location_id, active["run_id"])["Running"]
        with sqlite3.connect(worker.control_store) as control:
            assert control.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (1,)
        log: Path = tmp_path / "replacement-scheduler.log"
        with location_process(
            [
                str(Path(sys.executable).with_name("sqb")),
                "scheduler",
                "--database-env",
                "SQLBUILD_SCHEDULER_TEST_STORE",
                "--launchers",
                str(tmp_path / "launchers.json"),
            ],
            log,
            tmp_path / "replacement-scheduler.stdout",
        ) as process:
            replacement: ScheduledWorker = worker._replace(process=process, log=log)
            wait_for_log(process, log, "Worker supervision recovered for run", timeout=45)
            assert (
                worker_container(worker.docker, worker.api.location_id, active["run_id"])["Pid"]
                == before["Pid"]
            )
            cancel_running_statement(replacement)
            run: dict[str, Any] = worker_result(replacement, test_case)
            assert run["status"] == test_case.expected_status, log.read_text()
            with sqlite3.connect(worker.control_store) as control:
                assert control.execute(
                    "SELECT count(*) FROM recall_events WHERE event_type='run_claimed'"
                ).fetchone() == (1,)
