"""Seed execution pipeline."""

from __future__ import annotations

import queue
import time
from collections.abc import Callable
from concurrent.futures import Future, ThreadPoolExecutor
from contextvars import copy_context
from typing import Any, cast

from sqlbuild._native import PlanOutput, SeedPlanEntry
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.executor.build.models import SeedExecutionResult
from sqlbuild.executor.pipeline.main.raise_warehouse_connection_failure import (
    raise_warehouse_connection_failure,
)
from sqlbuild.executor.run.classes.fingerprint_writes import FingerprintWrites
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.executor.seed.main._execute import execute_seed
from sqlbuild.runtime.contracts.types import ConnectionElapsedCallback
from sqlbuild.runtime.observability.classes.resource_attempt_lifecycle import (
    ResourceAttemptLifecycle,
)


def run_seed_pipeline(
    *,
    plan: PlanOutput,
    connection_config: dict[str, object],
    adapter: BaseAdapter,
    max_concurrency: int = 1,
    run_id: str = "",
    query_change_tracking: bool = False,
    on_seed_complete: Callable[[SeedExecutionResult], None] | None = None,
    on_connection_start: Callable[[int], None] | None = None,
    on_connection_complete: ConnectionElapsedCallback | None = None,
    on_connection_error: ConnectionElapsedCallback | None = None,
) -> tuple[SeedExecutionResult, ...]:
    """Execute all seed loads from a compiled plan."""

    seed_entries: tuple[SeedPlanEntry, ...] = plan.seed_entries
    if not seed_entries:
        return ()
    effective_concurrency: int = max(1, min(max_concurrency, len(seed_entries)))
    if on_connection_start is not None:
        on_connection_start(effective_concurrency)
    start: float = time.monotonic()
    connections: list[Any] = []
    try:
        for _ in range(effective_concurrency):
            connections.append(adapter.connect(connection_config))
    except Exception as error:
        if on_connection_error is not None:
            on_connection_error(effective_concurrency, elapsed_seconds=time.monotonic() - start)
        connection: Any
        for connection in connections:
            adapter.close(connection)
        raise_warehouse_connection_failure(adapter=adapter, error=error)
    if on_connection_complete is not None:
        on_connection_complete(effective_concurrency, elapsed_seconds=time.monotonic() - start)

    pending: queue.SimpleQueue[tuple[int, SeedPlanEntry]] = queue.SimpleQueue()
    indexed_entry: tuple[int, SeedPlanEntry]
    for indexed_entry in enumerate(seed_entries):
        pending.put(indexed_entry)
    fingerprint_writes: FingerprintWrites = FingerprintWrites()
    try:
        with ThreadPoolExecutor(max_workers=effective_concurrency) as pool:
            futures: tuple[Future[tuple[tuple[int, SeedExecutionResult], ...]], ...] = tuple(
                cast(
                    Future[tuple[tuple[int, SeedExecutionResult], ...]],
                    pool.submit(
                        copy_context().run,
                        _run_seed_worker,
                        pending=pending,
                        fingerprint_writes=fingerprint_writes,
                        adapter=adapter,
                        connection=connections[worker_index],
                        run_id=run_id,
                        query_change_tracking=query_change_tracking,
                        on_seed_complete=on_seed_complete,
                    ),
                )
                for worker_index in range(effective_concurrency)
            )
            worker_results: tuple[tuple[tuple[int, SeedExecutionResult], ...], ...] = tuple(
                future.result() for future in futures
            )
        indexed_results: list[tuple[int, SeedExecutionResult]] = []
        for worker_result in worker_results:
            indexed_results.extend(worker_result)
        ordered_results: list[SeedExecutionResult] = []
        for _, result in sorted(indexed_results, key=lambda item: item[0]):
            ordered_results.append(result)
        return tuple(ordered_results)
    finally:
        connection = None
        for connection in connections:
            adapter.close(connection)


def _run_seed_worker(
    *,
    pending: queue.SimpleQueue[tuple[int, SeedPlanEntry]],
    fingerprint_writes: FingerprintWrites,
    adapter: BaseAdapter,
    connection: Any,
    run_id: str,
    query_change_tracking: bool,
    on_seed_complete: Callable[[SeedExecutionResult], None] | None,
) -> tuple[tuple[int, SeedExecutionResult], ...]:
    """Load seeds from the shared queue until it is empty, so a slow seed holds up only itself."""

    indexed_results: list[tuple[int, SeedExecutionResult]] = []
    seed_index: int
    entry: SeedPlanEntry
    while True:
        try:
            seed_index, entry = pending.get_nowait()
        except queue.Empty:
            break
        with ResourceAttemptLifecycle(
            resource_id=f"seed:{entry.name}",
            resource_kind="seed",
            resource_name=entry.name,
            run_id=run_id,
        ) as lifecycle:
            result: SeedExecutionResult = execute_seed(
                seed_entry=entry,
                adapter=adapter,
                connection=connection,
                statement_recorder=StatementRecorder(),
                run_id=run_id,
                query_change_tracking=query_change_tracking,
                fingerprint_writes=fingerprint_writes,
            )
            if result.status == ExecutionStatus.FAILED:
                lifecycle.failed(error_code=result.error_code)
        indexed_results.append((seed_index, result))
        if on_seed_complete is not None:
            on_seed_complete(result)
    return tuple(indexed_results)
