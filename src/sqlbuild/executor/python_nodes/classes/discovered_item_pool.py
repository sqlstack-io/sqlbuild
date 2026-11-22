"""Runs ready discovered items on threads, capped per set; the calling thread keeps all run state."""

from __future__ import annotations

from collections import Counter
from concurrent.futures import FIRST_COMPLETED, Future, ThreadPoolExecutor, wait
from contextvars import copy_context
from dataclasses import replace
from threading import Lock
from typing import Any, cast

from sqlbuild._native import CommandTerminated, termination_requested
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.compiler.python_nodes.types import PythonNodeStatus
from sqlbuild.executor.node_results.main._direct_store import build_direct_node_result_store
from sqlbuild.executor.python_nodes._helpers.parallel_items import parallel_set
from sqlbuild.executor.python_nodes.constants import CANCEL_REPEAT_SECONDS
from sqlbuild.executor.python_nodes.main._ready import run_ready_python_node
from sqlbuild.executor.python_nodes.models import (
    DiscoveredItemCallbacks,
    ItemSlot,
    PythonNodeExecutionResult,
    PythonNodeRunState,
    PythonNodeRuntime,
    RunningItem,
)
from sqlbuild.executor.python_nodes.types import ExecutablePythonNode
from sqlbuild.runtime.execution_limits.main._cancel_running_statements import (
    cancel_running_statements,
)


class DiscoveredItemPool:
    """One warehouse connection and result store per running item, and one write at a time.

    Threads only call the items. The calling thread submits them, and records and reports each as
    soon as it finishes, before it submits what that one unlocked.
    """

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

    def run(self, *, thread_limit: int) -> None:
        """Run until no item is ready or running, and join every thread before returning."""

        try:
            with ThreadPoolExecutor(
                max_workers=thread_limit, thread_name_prefix="sqlbuild-item"
            ) as threads:
                try:
                    self._run_until_drained(threads)
                except BaseException as error:
                    self._stop(cancelled=isinstance(error, KeyboardInterrupt))
                    raise
        finally:
            self._close()

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
            slot: ItemSlot = self._take_slot()
            future: Future[PythonNodeExecutionResult] = cast(
                Future[PythonNodeExecutionResult],
                threads.submit(
                    copy_context().run,
                    run_ready_python_node,
                    node=node,
                    upstream_results=self._callbacks.upstream_results(node.name),
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

    def _settle(
        self, *, future: Future[PythonNodeExecutionResult], result: PythonNodeExecutionResult
    ) -> None:
        item: RunningItem = self._running[future]
        with self._write_lock:
            self._callbacks.complete(item.node, result)
        del self._running[future]
        self._idle.append(item.slot)

    def _stop(self, *, cancelled: bool) -> None:
        """Start nothing more; keep what succeeded, and record the other items of a cancel as cancelled."""

        while self._running:
            if cancelled:
                cancel_running_statements()
            done: set[Future[PythonNodeExecutionResult]]
            done, _ = wait(
                self._running,
                timeout=CANCEL_REPEAT_SECONDS if cancelled else None,
                return_when=FIRST_COMPLETED,
            )
            for future in [future for future in self._running if future in done]:
                returned: bool = future.exception() is None
                if returned and (
                    not cancelled or future.result().status == PythonNodeStatus.SUCCESS
                ):
                    self._settle(future=future, result=future.result())
                    continue
                item: RunningItem = self._running.pop(future)
                if cancelled and self._callbacks.cancelled is not None:
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

    def _close(self) -> None:
        slot: ItemSlot
        for slot in self._opened:
            self._runtime.adapter.close(slot.connection)
        self._opened.clear()
        self._idle.clear()
