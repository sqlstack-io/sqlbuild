"""Ingress loaders beside each other, each on its own warehouse connection, within the limit."""

from __future__ import annotations

from collections.abc import Callable
from concurrent.futures import FIRST_COMPLETED, Future, ThreadPoolExecutor, wait
from contextvars import copy_context
from dataclasses import replace
from functools import partial
from threading import Lock
from typing import Any, cast

from sqlbuild.executor.node_results.main._direct_store import build_direct_node_result_store
from sqlbuild.executor.python_nodes.constants import ABANDONABLE_THREAD_PREFIX
from sqlbuild.executor.python_nodes.models import PythonNodeRuntime
from sqlbuild.executor.scheduling.classes.lifecycle_progress import LifecycleProgress
from sqlbuild.executor.scheduling.models import LifecycleExecutionNode, LifecycleNodeResult


class IngressLoaderPool:
    """Start ready loaders up to the limit and complete each as it finishes, until none is ready."""

    def __init__(
        self,
        *,
        runtime: PythonNodeRuntime,
        eligible: Callable[[LifecycleExecutionNode], bool],
        execute: Callable[..., LifecycleNodeResult],
    ) -> None:
        self._runtime: PythonNodeRuntime = runtime
        self._eligible: Callable[[LifecycleExecutionNode], bool] = eligible
        self._execute: Callable[..., LifecycleNodeResult] = execute
        self._write_lock: Lock = Lock()

    def _thread_runtime(self, connection: Any) -> PythonNodeRuntime:
        """The runtime one loader runs with: its own connection, and a result store on it."""

        runtime: PythonNodeRuntime = self._runtime
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
        return replace(runtime, connection=connection, result_store=result_store)

    def run(self, progress: LifecycleProgress) -> None:
        """Run the ready loaders `progress` offers; a cancel leaves running threads unjoined."""

        limit: int = max(1, self._runtime.max_concurrency)
        threads: ThreadPoolExecutor = ThreadPoolExecutor(
            max_workers=limit, thread_name_prefix=ABANDONABLE_THREAD_PREFIX
        )
        running: dict[Future[LifecycleNodeResult], tuple[LifecycleExecutionNode, Any]] = {}
        try:
            while True:
                started: set[str] = {node.name for node, _ in running.values()}
                for node in progress.startable(eligible=self._eligible):
                    if len(running) >= limit:
                        break
                    if node.name in started:
                        continue
                    connection: Any = self._runtime.adapter.connect(
                        dict(self._runtime.connection_config)
                    )
                    future: Future[LifecycleNodeResult] = cast(
                        Future[LifecycleNodeResult],
                        threads.submit(
                            copy_context().run,
                            partial(
                                self._execute,
                                node=node,
                                on_runtime=self._thread_runtime(connection),
                            ),
                        ),
                    )
                    running[future] = (node, connection)
                if not running:
                    break
                done, _ = wait(running, return_when=FIRST_COMPLETED)
                for finished in done:
                    node, connection = running.pop(finished)
                    self._runtime.adapter.close(connection)
                    progress.complete(node=node, result=finished.result())
        except KeyboardInterrupt:
            threads.shutdown(wait=False, cancel_futures=True)
            raise
        threads.shutdown(wait=True)
