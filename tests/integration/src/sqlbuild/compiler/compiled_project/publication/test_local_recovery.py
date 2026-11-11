"""A replacement Linux dev scheduler retains physical ownership of surviving local workers."""

import json
import os
import signal
import sqlite3
import sys
import time
from contextlib import suppress
from pathlib import Path
from types import ModuleType
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    ScheduledWorker,
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    cancel_running_statement,
    location_process,
    running_statement,
    scheduled_worker,
    wait_for_interpreter,
    wait_for_log,
    wait_for_run,
    worker_result,
)

fcntl: ModuleType = pytest.importorskip("fcntl")


@pytest.mark.skipif(sys.platform != "linux", reason="Linux inherited-lock and pidfd recovery")
@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "scheduler SIGKILL does not orphan a running local warehouse statement",
            Path(
                "benchmarks/duckshop/edge_cases/orchestration/local_worker_recovery.sql"
            ).read_text(),
            "SELECT count(*) FROM duckdb_tables() WHERE table_name = 'orders'",
            "cancelled",
            [(0,)],
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_running_local_worker_when_scheduler_dies_then_restart_adopts_and_cancels_without_relaunch(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        active: dict[str, Any] = running_statement(worker)
        with sqlite3.connect(worker.control_store) as control:
            claim_id: str = control.execute(
                "SELECT worker_instance FROM sqb_runs WHERE run_id=?", (active["run_id"],)
            ).fetchone()[0]
        lock: Path = worker.project / ".sqlbuild/workers" / f"{claim_id}.lock"
        with lock.open("rb") as ownership:
            with pytest.raises(BlockingIOError):
                fcntl.flock(ownership, fcntl.LOCK_EX | fcntl.LOCK_NB)
            worker.process.kill()
            assert worker.process.wait(timeout=10) == -signal.SIGKILL
            with pytest.raises(BlockingIOError):
                fcntl.flock(ownership, fcntl.LOCK_EX | fcntl.LOCK_NB)
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
                with sqlite3.connect(worker.control_store) as control:
                    assert control.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (
                        1,
                    )
                cancel_running_statement(replacement)
                run: dict[str, Any] = worker_result(replacement, test_case)
                assert run["status"] == test_case.expected_status, log.read_text()
                with sqlite3.connect(worker.control_store) as control:
                    assert control.execute(
                        "SELECT count(*) FROM recall_events WHERE event_type='run_claimed'"
                    ).fetchone() == (1,)
                deadline: float = time.monotonic() + 10
                while True:
                    try:
                        fcntl.flock(ownership, fcntl.LOCK_EX | fcntl.LOCK_NB)
                        break
                    except BlockingIOError:
                        assert time.monotonic() < deadline, log.read_text()
                        time.sleep(0.02)


@pytest.mark.skipif(sys.platform != "linux", reason="Linux inherited-lock and pidfd recovery")
@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "successive supervisor crashes retain a stopped child's claim until publication commits",
            "SELECT 23 AS amount",
            "",
            "failed",
            [],
            launcher="uncooperative",
            concurrency_limits={"orders_limit": 1},
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/local_recovery_orders.py"
                ).read_text(),
            },
            store_setup=Path(
                "benchmarks/duckshop/edge_cases/orchestration/local_recovery_publication_fault.sql"
            ).read_text(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_an_unstarted_orphan_when_recovery_publication_fails_then_claims_wait_for_physical_exit_and_commit(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        pid: int = wait_for_interpreter(worker.process, worker.project / "worker.pid", worker.log)
        active: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, pending=frozenset({"queued"})
        )
        assert active["status"] == "launched"
        claims: tuple[str, ...] = (
            "SELECT * FROM sqb_write_claims",
            "SELECT * FROM sqb_concurrency_slots",
            "SELECT run_id, worker_instance, container_id, claimed_at, last_heartbeat_at "
            "FROM sqb_runs WHERE worker_instance IS NOT NULL",
        )
        with sqlite3.connect(worker.control_store) as control:
            assert control.execute(
                "SELECT group_key, run_id FROM sqb_concurrency_slots"
            ).fetchall() == [("orders_limit", active["run_id"])]
            retained: dict[str, list[tuple[Any, ...]]] = {
                claim: control.execute(claim).fetchall() for claim in claims
            }
            request: tuple[Any, ...] = control.execute(
                "SELECT request_json FROM sqb_runs"
            ).fetchone()
            claim_id: str = control.execute(
                "SELECT worker_instance FROM sqb_runs WHERE worker_instance IS NOT NULL"
            ).fetchone()[0]
        arguments: list[str] = [
            str(Path(sys.executable).with_name("sqb")),
            "scheduler",
            "--database-env",
            "SQLBUILD_SCHEDULER_TEST_STORE",
            "--launchers",
            str(tmp_path / "launchers.json"),
        ]
        handle: int = os.pidfd_open(pid)
        try:
            worker.process.kill()
            assert worker.process.wait(timeout=10) == -signal.SIGKILL
            log: Path = tmp_path / "replacement-scheduler.log"
            with location_process(
                arguments,
                log,
                tmp_path / "replacement-scheduler.stdout",
            ) as process:
                wait_for_log(process, log, "Worker supervision recovered for run", timeout=45)
                signal.pidfd_send_signal(handle, 0)
                with sqlite3.connect(worker.control_store) as control:
                    assert control.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (
                        1,
                    )
                    assert control.execute("SELECT status FROM sqb_runs").fetchone() == (
                        "launched",
                    )
                signal.pidfd_send_signal(handle, signal.SIGKILL)
                wait_for_log(process, log, "Retrying worker shutdown publication")
                assert process.poll() is None, log.read_text()
                assert "Worker launched for run" not in log.read_text()
                with sqlite3.connect(worker.control_store) as control:
                    assert control.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (
                        1,
                    )
                    assert {
                        claim: control.execute(claim).fetchall() for claim in claims
                    } == retained
                    assert control.execute(
                        "SELECT count(*) FROM recall_events WHERE event_type='run_terminated'"
                    ).fetchone() == (0,)
                process.kill()
                assert process.wait(timeout=10) == -signal.SIGKILL
            log = tmp_path / "second-replacement-scheduler.log"
            with location_process(
                arguments, log, tmp_path / "second-replacement-scheduler.stdout"
            ) as process:
                wait_for_log(process, log, "Worker supervision recovered for run", timeout=45)
                wait_for_log(process, log, "Retrying worker shutdown publication")
                assert process.poll() is None, log.read_text()
                assert "Worker launched for run" not in log.read_text()
                assert int((worker.project / "worker.pid").read_text()) == pid
                with sqlite3.connect(worker.control_store) as control:
                    assert {
                        claim: control.execute(claim).fetchall() for claim in claims
                    } == retained
                    assert (
                        control.execute("SELECT request_json FROM sqb_runs").fetchone() == request
                    )
                    assert control.execute(
                        "SELECT count(*) FROM recall_events WHERE event_type='run_terminated'"
                    ).fetchone() == (0,)
                lock: Path = worker.project / ".sqlbuild/workers" / f"{claim_id}.lock"
                with lock.open("rb") as ownership:
                    with pytest.raises(BlockingIOError):
                        fcntl.flock(ownership, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    with sqlite3.connect(worker.control_store) as control:
                        _ = control.execute("DROP TRIGGER reject_terminal")
                    assert process.wait(timeout=10) != 0, log.read_text()
                    fcntl.flock(ownership, fcntl.LOCK_EX | fcntl.LOCK_NB)
                with sqlite3.connect(worker.control_store) as control:
                    assert control.execute("SELECT status FROM sqb_runs").fetchone() == (
                        test_case.expected_status,
                    )
                    assert control.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (
                        0,
                    )
                    assert control.execute(
                        "SELECT count(*) FROM sqb_concurrency_slots"
                    ).fetchone() == (0,)
                    assert control.execute(
                        "SELECT count(*) FROM sqb_runs WHERE worker_instance IS NOT NULL"
                    ).fetchone() == (0,)
                    assert (
                        control.execute("SELECT request_json FROM sqb_runs").fetchone() == request
                    )
                    assert control.execute(
                        "SELECT count(*) FROM recall_events WHERE event_type='run_claimed'"
                    ).fetchone() == (1,)
                    assert control.execute(
                        "SELECT count(*) FROM recall_events WHERE event_type='run_terminated'"
                    ).fetchone() == (1,)
                    assert control.execute(
                        "SELECT json_extract(payload, '$.change.change.exit_code') FROM recall_events "
                        "WHERE event_type='run_terminated'"
                    ).fetchone() == (None,)
                assert not (worker.project / "warehouse.duckdb").exists()
        finally:
            with suppress(ProcessLookupError):
                signal.pidfd_send_signal(handle, signal.SIGKILL)
            os.close(handle)


@pytest.mark.skipif(sys.platform != "linux", reason="Linux inherited-lock and pidfd recovery")
@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "a launched worker lost with its scheduler",
            "SELECT 23 AS amount",
            "",
            "failed",
            [],
            launcher="uncooperative",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_worker_lost_with_its_scheduler_when_restarted_then_the_run_fails_naming_the_restart(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    edge_case: dict[str, Any] = json.loads(
        Path("benchmarks/duckshop/edge_cases/orchestration/lost_worker_restart.json").read_text()
    )
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        pid: int = wait_for_interpreter(worker.process, worker.project / "worker.pid", worker.log)
        worker.process.kill()
        assert worker.process.wait(timeout=10) == -signal.SIGKILL
        os.kill(pid, signal.SIGKILL)
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
            wait_for_log(process, log, "Worker supervision recovered for run", timeout=45)
            run: dict[str, Any] = wait_for_run(worker.api, process, log)
            assert (run["status"], run["status_reason"]) == (
                test_case.expected_status,
                edge_case["expected_reason"],
            ), log.read_text()
            assert "Worker launched for run" not in log.read_text()
