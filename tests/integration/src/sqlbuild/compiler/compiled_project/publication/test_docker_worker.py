"""A published schedule executes in a real pinned container, not a mocked launcher."""

import json
import signal
import sqlite3
import sys
import time
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path
from threading import Event
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import duckshop_orchestration
from tests.integration.src.sqlbuild.compiler.compiled_project.publication import helpers
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    ScheduledWorker,
    WorkerDeployment,
    WorkerExecutionTestCase,
    WorkerRestartTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.docker.helpers import (
    REQUIRES_DOCKER,
    docker_command,
    docker_deployment,
    lost_start_response,
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
            "WITH source(value) AS (VALUES ('37')) SELECT CAST(value AS BIGINT) AS amount FROM source",
            "SELECT amount FROM orders",
            "success",
            [(37,)],
            launcher="docker",
            files={
                "orchestration/jobs/annual_orders_job.toml": (
                    'name = "annual_orders_job"\n'
                    'description = "Build orders."\n'
                    'select = "orders"\n'
                    'schedule = { cron = "0 0 * * *", timezone = "UTC", starts_at = "2000-01-01T00:00:00Z" }\n'
                )
            },
        ),
        WorkerExecutionTestCase(
            "launcher defaults and job resources bound the worker, capped by max_memory",
            "SELECT 41 AS amount",
            "SELECT amount FROM orders",
            "success",
            [(41,)],
            launcher="docker",
            files={**duckshop_orchestration("container_resources_orders")},
            default_job_files={},
            launcher_settings={
                "run": {"memory": "768m", "cpus": 2, "pids": 256, "max_memory": "1g"}
            },
            expected_host_config={"Memory": 1 << 30, "NanoCpus": 500_000_000, "PidsLimit": 256},
        ),
        WorkerExecutionTestCase(
            "without max_memory a job cannot raise memory above its location container's",
            "SELECT 43 AS amount",
            "SELECT amount FROM orders",
            "success",
            [(43,)],
            launcher="docker",
            files={**duckshop_orchestration("container_resources_orders")},
            default_job_files={},
            expected_host_config={"Memory": 768 << 20, "NanoCpus": 500_000_000},
        ),
        WorkerExecutionTestCase(
            "container warehouse error retains failed node evidence",
            "WITH source(value) AS (VALUES ('invalid')) SELECT CAST(value AS BIGINT) AS amount FROM source",
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
        name: str = f"sqlbuild-{worker.api.location_id}-{run['run_id']}"
        host: dict[str, Any] = json.loads(
            docker_command(
                worker.docker.socket, "inspect", "--format", "{{json .HostConfig}}", name
            )
        )
        location: dict[str, Any] = json.loads(
            docker_command(
                worker.docker.socket,
                "inspect",
                "--format",
                "{{json .HostConfig}}",
                worker.docker.container,
            )
        )
        assert {key: host[key] for key in test_case.expected_host_config} == (
            test_case.expected_host_config
        )
        assert location["CapDrop"] and location["SecurityOpt"]
        assert (host["CapDrop"], host["SecurityOpt"]) == (
            location["CapDrop"],
            location["SecurityOpt"],
        )


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
                "--config",
                str(tmp_path / "server.toml"),
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


@REQUIRES_DOCKER
@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "worker completes before the lost launch response can be durably observed",
            "SELECT 37 AS amount",
            "SELECT amount FROM orders",
            "success",
            [(37,)],
            launcher="docker",
            store_setup=(
                "CREATE TRIGGER reject_launch_observation BEFORE INSERT ON recall_events "
                "WHEN NEW.event_type='platform_launch_failed' "
                "BEGIN SELECT RAISE(ABORT,'synthetic observation outage'); END;"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_lost_docker_start_response_when_the_worker_finishes_then_late_observation_retries_do_not_revive_it(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    inspection: Event = Event()

    @contextmanager
    def deployment(directory: Path, executable: str) -> Iterator[WorkerDeployment]:
        with docker_deployment(directory, executable) as actual:
            assert actual.docker is not None
            with lost_start_response(directory, actual.docker.socket, inspection) as proxy:
                yield actual._replace(configuration=actual.configuration | {"socket": str(proxy)})

    monkeypatch.setattr(helpers, "docker_deployment", deployment)
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        wait_for_log(worker.process, worker.log, "Retrying worker shutdown publication")
        run: dict[str, Any] = worker_result(worker, test_case)
        assert run["status"] == test_case.expected_status, worker.log.read_text()
        with sqlite3.connect(worker.control_store) as control:
            before: list[tuple[Any, ...]] = control.execute("SELECT * FROM sqb_runs").fetchall()
            assert control.execute(
                "SELECT COUNT(*) FROM recall_events WHERE event_type IN ('run_launch_failed','platform_launch_failed')"
            ).fetchone() == (0,)
            _ = control.execute("DROP TRIGGER reject_launch_observation")
            deadline: float = time.monotonic() + 10
            while control.execute(
                "SELECT COUNT(*) FROM recall_events WHERE event_type='platform_launch_failed'"
            ).fetchone() != (1,):
                assert worker.process.poll() is None, worker.log.read_text()
                assert time.monotonic() < deadline, worker.log.read_text()
                time.sleep(0.02)
            # Terminal state alone cannot bypass the still-unavailable physical observation.
            time.sleep(1.2)
            assert worker.process.poll() is None, worker.log.read_text()
            assert control.execute("SELECT * FROM sqb_runs").fetchall() == before
            assert control.execute(
                "SELECT event_type FROM recall_events WHERE event_type IN "
                "('run_terminated','run_launch_failed','platform_launch_failed') ORDER BY storage_id"
            ).fetchall() == [
                ("run_terminated",),
                ("run_launch_failed",),
                ("platform_launch_failed",),
            ]
            assert control.execute("SELECT COUNT(*) FROM sqb_write_claims").fetchone() == (0,)
            assert control.execute(
                "SELECT COUNT(*) FROM sqb_runs WHERE worker_instance IS NOT NULL"
            ).fetchone() == (0,)
            inspection.set()
            assert worker.process.wait(timeout=5) != 0, "Original response failure remains visible"
            assert worker.docker is not None
            state: dict[str, object] = worker_container(
                worker.docker, worker.api.location_id, run["run_id"]
            )
            assert state["Running"] is False
            assert state["ExitCode"] == 0
