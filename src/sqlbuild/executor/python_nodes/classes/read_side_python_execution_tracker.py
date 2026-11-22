"""Python read-side SQL-read Python lifecycle execution helpers."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import replace
from typing import Any

from sqlbuild._native import CostContext, DiscoveredPythonNode, PythonNodeGraph, output_node_scope
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.compiler.python_nodes.types import PythonNodeKind, PythonNodeStatus
from sqlbuild.executor.node_results.main._direct_store import build_direct_node_result_store
from sqlbuild.executor.python_nodes._helpers.discovered_items import parallel_set
from sqlbuild.executor.python_nodes._helpers.fingerprinting import (
    try_write_python_node_identity_fingerprint,
)
from sqlbuild.executor.python_nodes._helpers.ingress_execution import _to_executable_python_node
from sqlbuild.executor.python_nodes._helpers.read_side_execution import (
    _sql_result_failed,
    _sql_result_name,
)
from sqlbuild.executor.python_nodes.classes.discovered_item_pool import DiscoveredItemPool
from sqlbuild.executor.python_nodes.main._ready import run_ready_python_node
from sqlbuild.executor.python_nodes.models import (
    DiscoveredItemCallbacks,
    PythonNodeExecutionResult,
    PythonNodeRunState,
    PythonNodeRuntime,
)
from sqlbuild.executor.python_nodes.types import ExecutablePythonNode, PythonIdentityRecorder
from sqlbuild.runtime.observability.classes.resource_attempt_lifecycle import (
    ResourceAttemptLifecycle,
)


class ReadSidePythonExecutionTracker:
    """Dispatch selected read-only Python nodes as SQL dependencies scheduled in this run finish."""

    def __init__(
        self,
        *,
        python_graph: PythonNodeGraph,
        selected_python_names: frozenset[str],
        scheduled_sql_names: frozenset[str],
        runtime: PythonNodeRuntime,
        identity_recorder: PythonIdentityRecorder | None = None,
        on_node_complete: Callable[[object], None] | None = None,
        on_node_cancelled: Callable[[str], None] | None = None,
    ) -> None:
        self._python_graph: PythonNodeGraph = python_graph
        self._selected_python_names: frozenset[str] = selected_python_names
        self._scheduled_sql_names: frozenset[str] = scheduled_sql_names
        self._runtime: PythonNodeRuntime = runtime
        self._identity_recorder: PythonIdentityRecorder | None = identity_recorder
        self._on_node_complete: Callable[[object], None] | None = on_node_complete
        self._on_node_cancelled: Callable[[str], None] | None = on_node_cancelled
        self._parallel: dict[str, tuple[str, int]] = {}
        name: str
        for name in selected_python_names:
            selected: DiscoveredPythonNode = python_graph.nodes_by_name[name]
            limited: tuple[str, int] | None = parallel_set(
                discovered=None if selected.asset is None else selected.asset.discovered,
                runtime=runtime,
            )
            if limited is not None:
                self._parallel[name] = limited
        self._result_store: Any | None = (
            runtime.result_store
            if runtime.result_store is not None
            else (
                build_direct_node_result_store(
                    adapter=runtime.adapter,
                    connection=runtime.connection,
                    database=runtime.default_database,
                    schema=runtime.default_schema,
                    target_config=runtime.target_config,
                )
                if runtime.persist_node_results
                else None
            )
        )
        self._completed_sql_names: set[str] = set()
        self._failed_sql_names: set[str] = set()
        self._completed_python_names: set[str] = set()
        self._results_by_name: dict[str, PythonNodeExecutionResult] = {}
        self._run_state: PythonNodeRunState = PythonNodeRunState()

    @property
    def results(self) -> tuple[PythonNodeExecutionResult, ...]:
        return tuple(self._results_by_name.values())

    def finalize_unrun_python_nodes(self) -> tuple[PythonNodeExecutionResult, ...]:
        """Record selected Python nodes that could not run after SQL scheduling finished."""

        finalized_results: list[PythonNodeExecutionResult] = []
        node_name: str
        for node_name in sorted(self._selected_python_names):
            if node_name in self._completed_python_names:
                continue
            node: DiscoveredPythonNode = self._python_graph.nodes_by_name[node_name]
            with ResourceAttemptLifecycle(
                resource_id=f"{node.kind.value}:{node.name}",
                resource_kind=node.kind.value,
                resource_name=node.name,
                run_id=self._runtime.run_id,
            ) as lifecycle:
                result: PythonNodeExecutionResult = PythonNodeExecutionResult(
                    node_name=node.name,
                    kind=node.kind,
                    status=PythonNodeStatus.SKIPPED,
                    skip_reason=self._unrun_reason(node),
                )
                self._results_by_name[node.name] = result
                self._completed_python_names.add(node.name)
                finalized_results.append(result)
                lifecycle.skipped(skip_code="dependency")
        return tuple(finalized_results)

    def record_sql_result(self, result: object) -> None:
        """Record one SQL/load result and dispatch newly-ready Python nodes."""

        sql_name: str | None = _sql_result_name(result)
        if sql_name is None:
            return
        if _sql_result_failed(result):
            self._failed_sql_names.add(sql_name)
        else:
            self._completed_sql_names.add(sql_name)
        self.dispatch_ready_python_nodes()

    def dispatch_ready_python_nodes(self) -> None:
        """Run every ready node, items with `max_parallel` side by side, reporting each at once."""

        progressed: bool = True
        while progressed:
            progressed = False
            node_name: str
            for node_name in sorted(self._selected_python_names):
                if not self._is_ready(node_name):
                    continue
                if node_name in self._parallel:
                    self._run_parallel_items()
                else:
                    self._execute_python_node(self._python_graph.nodes_by_name[node_name])
                progressed = True

    def _is_ready(self, node_name: str) -> bool:
        if node_name in self._completed_python_names:
            return False
        node: DiscoveredPythonNode = self._python_graph.nodes_by_name[node_name]
        return (
            node.kind == PythonNodeKind.ASSET
            and self._sql_dependencies_satisfied(node)
            and self._python_dependencies_satisfied(node)
        )

    def _run_parallel_items(self) -> None:
        DiscoveredItemPool(
            runtime=self._runtime,
            run_state=self._run_state,
            callbacks=DiscoveredItemCallbacks(
                ready=self._ready_items,
                upstream_results=self._upstream_results,
                complete=self._complete_item,
                cancelled=self._on_node_cancelled,
            ),
        ).run(thread_limit=sum(dict(self._parallel.values()).values()))

    def _ready_items(self) -> tuple[ExecutablePythonNode, ...]:
        return tuple(
            _to_executable_python_node(self._python_graph.nodes_by_name[name])
            for name in sorted(self._parallel)
            if self._is_ready(name)
        )

    def _complete_item(
        self, *, node: ExecutablePythonNode, result: PythonNodeExecutionResult
    ) -> None:
        self._finalize_owned_result(node=node, result=result)
        if self._on_node_complete is not None:
            self._on_node_complete(result)

    def _upstream_results(self, node_name: str) -> tuple[PythonNodeExecutionResult, ...]:
        return tuple(
            self._results_by_name[upstream_name]
            for upstream_name in self._python_graph.upstream_deps.get(node_name, ())
            if upstream_name in self._results_by_name
        )

    def _sql_dependencies_satisfied(self, node: DiscoveredPythonNode) -> bool:
        sql_dep_name: str
        for sql_dep_name in (dependency.name for dependency in node.sql_deps):
            if sql_dep_name in self._failed_sql_names:
                return False
            if (
                sql_dep_name in self._scheduled_sql_names
                and sql_dep_name not in self._completed_sql_names
            ):
                return False
        return True

    def _python_dependencies_satisfied(self, node: DiscoveredPythonNode) -> bool:
        upstream_name: str
        for upstream_name in self._python_graph.upstream_deps.get(node.name, ()):
            if upstream_name in self._selected_python_names:
                if upstream_name not in self._completed_python_names:
                    return False
        return True

    def _execute_python_node(self, node: DiscoveredPythonNode) -> None:
        runtime: PythonNodeRuntime = replace(self._runtime, result_store=self._result_store)
        result: PythonNodeExecutionResult = run_ready_python_node(
            node=_to_executable_python_node(node),
            upstream_results=self._upstream_results(node.name),
            runtime=runtime,
            statement_recorder=StatementRecorder(),
            run_state=self._run_state,
            on_result=self._finalize_owned_result,
        )
        if self._on_node_complete is not None:
            self._on_node_complete(result)

    def _finalize_owned_result(
        self, *, node: ExecutablePythonNode, result: PythonNodeExecutionResult
    ) -> None:
        discovered_node: DiscoveredPythonNode = self._python_graph.nodes_by_name[node.name]
        self._run_state.record_result(result=result)
        self._results_by_name[discovered_node.name] = result
        self._completed_python_names.add(discovered_node.name)
        if result.status != PythonNodeStatus.SUCCESS:
            return
        with (
            output_node_scope(f"{discovered_node.kind.value}:{discovered_node.name}"),
            CostContext.resource_scope(
                resource_type=discovered_node.kind.value,
                resource_name=discovered_node.name,
                phase="finalize",
            ),
        ):
            if self._identity_recorder is not None:
                self._identity_recorder(identity=discovered_node.identity, _target_name=None)
            else:
                try_write_python_node_identity_fingerprint(
                    identity=discovered_node.identity,
                    adapter=self._runtime.adapter,
                    connection=self._runtime.connection,
                    run_id=self._runtime.run_id,
                    database=self._runtime.default_database,
                    schema=self._runtime.default_schema,
                )

    def _unrun_reason(self, node: DiscoveredPythonNode) -> str:
        sql_dep_name: str
        for sql_dep_name in (dependency.name for dependency in node.sql_deps):
            if sql_dep_name in self._failed_sql_names:
                return f"Upstream SQL resource did not succeed: {sql_dep_name}"
            if (
                sql_dep_name in self._scheduled_sql_names
                and sql_dep_name not in self._completed_sql_names
            ):
                return f"Upstream SQL resource did not complete: {sql_dep_name}"
        upstream_name: str
        for upstream_name in self._python_graph.upstream_deps.get(node.name, ()):
            if upstream_name in self._selected_python_names:
                return f"Upstream Python node did not complete: {upstream_name}"
        return "Read-side Python node did not become ready"
