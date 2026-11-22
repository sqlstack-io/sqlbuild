"""Runs ready discovered items on threads, capped per set; only the calling thread records them."""

from __future__ import annotations

import sys
from collections import Counter
from concurrent.futures import FIRST_COMPLETED, Future, ThreadPoolExecutor, wait
from contextvars import copy_context
from dataclasses import replace
from threading import Lock
from time import monotonic
from typing import Any, cast

from sqlbuild._native import CommandTerminated, termination_requested
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.compiler.python_nodes.types import PythonNodeFanInAction, PythonNodeStatus
from sqlbuild.executor.node_results.main._direct_store import build_direct_node_result_store
from sqlbuild.executor.python_nodes._helpers.discovered_items import parallel_set
from sqlbuild.executor.python_nodes._helpers.results import evaluate_python_node_fan_in
from sqlbuild.executor.python_nodes.constants import (
    ABANDONABLE_THREAD_PREFIX,
    ABANDONED_STEPS_NOTICE,
    CANCEL_GRACE_SECONDS,
    CANCEL_REPEAT_SECONDS,
)
from sqlbuild.executor.python_nodes.main._ready import run_ready_python_node
from sqlbuild.executor.python_nodes.models import (
    DiscoveredItemCallbacks,
    ItemSlot,
    PythonNodeExecutionResult,
    PythonNodeFanInDecision,
    PythonNodeRunState,
    PythonNodeRuntime,
    RunningItem,
)
from sqlbuild.executor.python_nodes.types import ExecutablePythonNode
from sqlbuild.runtime.execution_limits.main.cancel_running_statements import (
    cancel_running_statements,
)


class DiscoveredItemPool:
    """One connection and result store per running item, each result recorded as it finishes."""

    def __init__(
        self,
        *,
        runtime: PythonNodeRuntime,
        run_state: PythonNodeRunState,
        callbacks: DiscoveredItemCallbacks,
    ) -> None:
        self._runtime: PythonNodeRuntime = runtime
        self._run_state: PythonNodeRunState = run_state
        self._callbacks: DiscoveredItemCallbacks = callbacks
        self._write_lock: Lock = Lock()
        self._idle: list[ItemSlot] = []
        self._opened: list[ItemSlot] = []
        self._running: dict[Future[PythonNodeExecutionResult], RunningItem] = {}
        self._cancelled: bool = False

    def run(self, *, thread_limit: int) -> None:
        """Run until no item is ready or running; only a cancel leaves a thread unjoined."""

        threads: ThreadPoolExecutor = ThreadPoolExecutor(
            max_workers=thread_limit, thread_name_prefix=ABANDONABLE_THREAD_PREFIX
        )
        try:
            try:
                self._run_until_drained(threads)
            except KeyboardInterrupt:
                self._cancel()
                raise
            except BaseException:
                self._finish_running()
                raise
        finally:
            self._release(threads)

    def _run_until_drained(self, threads: ThreadPoolExecutor) -> None:
        while True:
            self._submit_ready(threads)
            if not self._running:
                return
            done: set[Future[PythonNodeExecutionResult]]
            done, _ = wait(self._running, return_when=FIRST_COMPLETED)
            for future in [future for future in self._running if future in done]:
                self._settle(future=future, result=future.result())

    def _submit_ready(self, threads: ThreadPoolExecutor) -> None:
        started: set[str] = {item.node.name for item in self._running.values()}
        running_by_set: Counter[str] = Counter(item.set_name for item in self._running.values())
        node: ExecutablePythonNode
        for node in self._callbacks.ready():
            if termination_requested():
                raise CommandTerminated("terminated by SIGTERM")
            if node.name in started:
                continue
            limited: tuple[str, int] | None = parallel_set(
                discovered=node.discovered, runtime=self._runtime
            )
            if limited is None or running_by_set[limited[0]] >= limited[1]:
                continue
            upstream: tuple[PythonNodeExecutionResult, ...] = self._callbacks.upstream_results(
                node.name
            )
            slot: ItemSlot = self._take_slot()
            self._report_start(node=node, upstream=upstream)
            future: Future[PythonNodeExecutionResult] = cast(
                Future[PythonNodeExecutionResult],
                threads.submit(
                    copy_context().run,
                    run_ready_python_node,
                    node=node,
                    upstream_results=upstream,
                    runtime=replace(
                        self._runtime, connection=slot.connection, result_store=slot.result_store
                    ),
                    statement_recorder=StatementRecorder(),
                    run_state=self._run_state,
                ),
            )
            self._running[future] = RunningItem(node=node, set_name=limited[0], slot=slot)
            started.add(node.name)
            running_by_set[limited[0]] += 1

    def _report_start(
        self,
        *,
        node: ExecutablePythonNode,
        upstream: tuple[PythonNodeExecutionResult, ...],
    ) -> None:
        """Report that `node` has its slot, unless its upstream results keep it from running."""

        if self._callbacks.started is None:
            return
        decision: PythonNodeFanInDecision = evaluate_python_node_fan_in(upstream_results=upstream)
        if decision.action == PythonNodeFanInAction.RUN:
            self._callbacks.started(node.name)

    def _settle(
        self,
        *,
        future: Future[PythonNodeExecutionResult],
        result: PythonNodeExecutionResult,
        lock_seconds: float = -1,
    ) -> None:
        """Record and report `result`; a cancel does not wait past its grace for a step's write."""

        item: RunningItem = self._running[future]
        locked: bool = self._write_lock.acquire(timeout=lock_seconds)
        try:
            self._callbacks.complete(node=item.node, result=result)
        finally:
            if locked:
                self._write_lock.release()
        del self._running[future]
        self._idle.append(item.slot)

    def _finish_running(self) -> None:
        """After an error: start nothing more, and settle what is running as it returns."""

        try:
            while self._running:
                done: set[Future[PythonNodeExecutionResult]]
                done, _ = wait(self._running, return_when=FIRST_COMPLETED)
                for future in [future for future in self._running if future in done]:
                    if future.exception() is None:
                        self._settle(future=future, result=future.result())
                    else:
                        del self._running[future]
        except KeyboardInterrupt:
            self._cancel()
            raise

    def _cancel(self) -> None:
        """Keep what succeeded; report the rest cancelled, waiting only the grace for them."""

        self._cancelled = True
        deadline: float = monotonic() + CANCEL_GRACE_SECONDS
        while self._running and monotonic() < deadline:
            cancel_running_statements()
            done: set[Future[PythonNodeExecutionResult]]
            done, _ = wait(
                self._running,
                timeout=max(0.0, min(CANCEL_REPEAT_SECONDS, deadline - monotonic())),
                return_when=FIRST_COMPLETED,
            )
            for future in [future for future in self._running if future in done]:
                if future.exception() is None and future.result().status == (
                    PythonNodeStatus.SUCCESS
                ):
                    self._settle(
                        future=future,
                        result=future.result(),
                        lock_seconds=max(0.0, deadline - monotonic()),
                    )
                    continue
                self._report_cancelled(self._running.pop(future))
        if not self._running:
            return
        steps: str = ", ".join(sorted(item.node.name for item in self._running.values()))
        _ = sys.stderr.write(
            ABANDONED_STEPS_NOTICE.format(steps=steps, seconds=CANCEL_GRACE_SECONDS)
        )
        for item in self._running.values():
            self._report_cancelled(item)

    def _report_cancelled(self, item: RunningItem) -> None:
        if self._callbacks.cancelled is not None:
            self._callbacks.cancelled(item.node.name)

    def _take_slot(self) -> ItemSlot:
        if self._idle:
            return self._idle.pop()
        runtime: PythonNodeRuntime = self._runtime
        connection: Any = runtime.adapter.connect(dict(runtime.connection_config))
        result_store: Any | None = runtime.result_store
        if result_store is None and runtime.persist_node_results:
            result_store = build_direct_node_result_store(
                adapter=runtime.adapter,
                connection=connection,
                database=runtime.default_database,
                schema=runtime.default_schema,
                target_config=runtime.target_config,
                write_lock=self._write_lock,
            )
        slot: ItemSlot = ItemSlot(connection=connection, result_store=result_store)
        self._opened.append(slot)
        return slot

    def _release(self, threads: ThreadPoolExecutor) -> None:
        """Join and close; leave a cancelled step still running its slot and its providers."""

        abandoned: bool = self._cancelled and bool(self._running)
        if abandoned and self._runtime.providers is not None:
            self._runtime.providers.leave_open()
        threads.shutdown(wait=not abandoned)
        held: list[ItemSlot] = [item.slot for item in self._running.values()] if abandoned else []
        slot: ItemSlot
        for slot in self._opened:
            if all(slot is not kept for kept in held):
                self._runtime.adapter.close(slot.connection)
        self._opened.clear()
        self._idle.clear()
