"""Behavior tests for bounded build-scheduler microbatch sub-work."""

from __future__ import annotations

import threading
from concurrent.futures import Future, ThreadPoolExecutor
from queue import Queue
from typing import Any

import pytest

from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.executor.build.classes.build_scheduler import BuildScheduler
from sqlbuild.executor.run.models import (
    BatchWindow,
    MicrobatchLifecycleState,
    MicrobatchPhaseOutcome,
    ModelExecutionResult,
)
from sqlbuild.executor.scheduling.types import ExecutionStatus
from tests.unit.src.sqlbuild.executor.build.classes._test_types import (
    MicrobatchBoundedFutureTestCase,
)


class _TrackingThreadPoolExecutor(ThreadPoolExecutor):
    def __init__(self, *, max_workers: int) -> None:
        super().__init__(max_workers=max_workers)
        self.total_submitted = 0
        self.outstanding = 0
        self.max_outstanding = 0
        self._tracking_lock = threading.Lock()
        self._tracking_condition = threading.Condition(self._tracking_lock)

    def submit(self, fn: Any, /, *args: Any, **kwargs: Any) -> Future[Any]:
        future: Future[Any] = super().submit(fn, *args, **kwargs)
        with self._tracking_lock:
            self.total_submitted += 1
            self.outstanding += 1
            self.max_outstanding = max(self.max_outstanding, self.outstanding)
            self._tracking_condition.notify_all()

        def mark_complete(_future: Future[Any]) -> None:
            with self._tracking_lock:
                self.outstanding -= 1

        future.add_done_callback(mark_complete)
        return future

    def wait_for_submissions(self, *, count: int, timeout: float) -> bool:
        with self._tracking_condition:
            return self._tracking_condition.wait_for(
                lambda: self.total_submitted >= count,
                timeout=timeout,
            )


@pytest.mark.parametrize(
    "test_case",
    [
        MicrobatchBoundedFutureTestCase(
            description="five thousand batches keep only the model ceiling in flight",
            batch_count=5_000,
            global_concurrency=4,
            model_concurrency=4,
            expected_max_subworker_futures=3,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_large_microbatch_phase_when_workers_are_blocked_then_future_submission_stays_bounded(
    test_case: MicrobatchBoundedFutureTestCase,
) -> None:
    scheduler: Any = object.__new__(BuildScheduler)
    scheduler._max_concurrency = test_case.global_concurrency
    scheduler._microbatch_coordinator_lock = threading.Lock()
    scheduler._job_lock = threading.Lock()
    scheduler._job_limits = {}
    scheduler._job_running = {}
    scheduler._microbatch_coordinators = {"orders"}
    scheduler._microbatch_coordinator_demand = 1
    scheduler._microbatch_subworkers = 0
    scheduler._connection_pool = Queue()
    for index in range(test_case.expected_max_subworker_futures):
        scheduler._connection_pool.put(f"worker-{index}")

    release_workers: threading.Event = threading.Event()
    state: MicrobatchLifecycleState = MicrobatchLifecycleState(
        warnings=[],
        audit_results=[],
        hook_results=[],
        statement_recorder=StatementRecorder(),
    )

    def coordinator_execute(_batch: BatchWindow) -> MicrobatchPhaseOutcome:
        return MicrobatchPhaseOutcome(state=state, completed_batches=1, rows_affected=0)

    def worker_execute(_batch: BatchWindow) -> MicrobatchPhaseOutcome:
        assert release_workers.wait(timeout=5.0)
        return MicrobatchPhaseOutcome(state=state, completed_batches=1, rows_affected=0)

    def execute(batch: BatchWindow, connection: object) -> MicrobatchPhaseOutcome:
        return {"coordinator": coordinator_execute}.get(str(connection), worker_execute)(batch)

    batches: tuple[BatchWindow, ...] = tuple(
        BatchWindow(start=str(index), end=str(index + 1), index=index)
        for index in range(test_case.batch_count)
    )
    results: list[tuple[MicrobatchPhaseOutcome, ...]] = []
    errors: list[BaseException] = []

    with _TrackingThreadPoolExecutor(max_workers=test_case.expected_max_subworker_futures) as pool:

        def run_phase() -> None:
            try:
                results.append(
                    BuildScheduler._run_microbatch_subwork(
                        scheduler,
                        jobs=(),
                        pool=pool,
                        coordinator_connection="coordinator",
                        batches=batches,
                        concurrency=test_case.model_concurrency,
                        execute=execute,
                    )
                )
            except BaseException as error:
                errors.append(error)

        runner: threading.Thread = threading.Thread(target=run_phase, daemon=True)
        runner.start()
        assert pool.wait_for_submissions(
            count=test_case.expected_max_subworker_futures,
            timeout=5.0,
        )
        assert pool.total_submitted == test_case.expected_max_subworker_futures
        assert pool.max_outstanding == test_case.expected_max_subworker_futures
        release_workers.set()
        runner.join(timeout=10.0)
        assert not runner.is_alive()

    assert errors == []
    assert len(results) == 1
    assert len(results[0]) == test_case.batch_count
    assert pool.max_outstanding <= test_case.expected_max_subworker_futures
    assert scheduler._microbatch_subworkers == 0
    assert scheduler._connection_pool.qsize() == test_case.expected_max_subworker_futures


@pytest.mark.parametrize(
    "test_case",
    [
        MicrobatchBoundedFutureTestCase(
            description="coordinator failure drains only initially submitted siblings",
            batch_count=5_000,
            global_concurrency=4,
            model_concurrency=4,
            expected_max_subworker_futures=3,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_coordinator_failure_when_subworkers_are_running_then_pending_batches_are_not_submitted(
    test_case: MicrobatchBoundedFutureTestCase,
) -> None:
    scheduler: Any = object.__new__(BuildScheduler)
    scheduler._max_concurrency = test_case.global_concurrency
    scheduler._microbatch_coordinator_lock = threading.Lock()
    scheduler._job_lock = threading.Lock()
    scheduler._job_limits = {}
    scheduler._job_running = {}
    scheduler._microbatch_coordinators = {"orders"}
    scheduler._microbatch_coordinator_demand = 1
    scheduler._microbatch_subworkers = 0
    scheduler._connection_pool = Queue()
    for index in range(test_case.expected_max_subworker_futures):
        scheduler._connection_pool.put(f"worker-{index}")

    release_workers: threading.Event = threading.Event()
    state: MicrobatchLifecycleState = MicrobatchLifecycleState(
        warnings=[],
        audit_results=[],
        hook_results=[],
        statement_recorder=StatementRecorder(),
    )

    def coordinator_execute(_batch: BatchWindow) -> MicrobatchPhaseOutcome:
        release_workers.set()
        return MicrobatchPhaseOutcome(
            state=state,
            failure=ModelExecutionResult(
                model_name="orders",
                status=ExecutionStatus.FAILED,
                error_message="coordinator failed",
            ),
        )

    def worker_execute(_batch: BatchWindow) -> MicrobatchPhaseOutcome:
        assert release_workers.wait(timeout=5.0)
        return MicrobatchPhaseOutcome(state=state, completed_batches=1, rows_affected=0)

    def execute(batch: BatchWindow, connection: object) -> MicrobatchPhaseOutcome:
        return {"coordinator": coordinator_execute}.get(str(connection), worker_execute)(batch)

    batches: tuple[BatchWindow, ...] = tuple(
        BatchWindow(start=str(index), end=str(index + 1), index=index)
        for index in range(test_case.batch_count)
    )
    with _TrackingThreadPoolExecutor(max_workers=test_case.expected_max_subworker_futures) as pool:
        outcomes: tuple[MicrobatchPhaseOutcome, ...] = BuildScheduler._run_microbatch_subwork(
            scheduler,
            jobs=(),
            pool=pool,
            coordinator_connection="coordinator",
            batches=batches,
            concurrency=test_case.model_concurrency,
            execute=execute,
        )

    assert pool.total_submitted == test_case.expected_max_subworker_futures
    assert pool.max_outstanding <= test_case.expected_max_subworker_futures
    assert len(outcomes) <= test_case.expected_max_subworker_futures + 1
    assert any(outcome.failure is not None for outcome in outcomes)
    assert scheduler._microbatch_subworkers == 0
    assert scheduler._connection_pool.qsize() == test_case.expected_max_subworker_futures


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
