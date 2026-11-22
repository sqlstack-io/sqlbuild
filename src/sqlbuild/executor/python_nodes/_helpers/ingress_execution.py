"""Python ingress pre-SQL Python/loader lifecycle execution."""

from __future__ import annotations

from collections.abc import Callable, Mapping
from contextlib import AbstractContextManager, nullcontext
from dataclasses import replace
from functools import partial
from typing import Any

from sqlbuild._native import (
    CostContext,
    DiscoveredAssetFunction,
    DiscoveredLoaderFunction,
    DiscoveredPythonNode,
    PythonNodeGraph,
    PythonSqlRunLifecyclePlan,
    SourceEntry,
    output_node_scope,
)
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.compiler.python_nodes.types import PythonNodeKind, PythonNodeStatus, SkipMode
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.load.main._execute import execute_source_load
from sqlbuild.executor.load.main._persist_result import persist_loader_result
from sqlbuild.executor.load.main._resource_kind import load_resource_kind
from sqlbuild.executor.load.main._skipped_result import skipped_load_result
from sqlbuild.executor.load.models import LoadExecutionResult, LoadRuntimeParams
from sqlbuild.executor.node_results.main._direct_store import build_direct_node_result_store
from sqlbuild.executor.python_nodes._helpers.fingerprinting import (
    try_write_python_node_identity_fingerprint,
)
from sqlbuild.executor.python_nodes._helpers.lifecycle_nodes import build_ingress_lifecycle_nodes
from sqlbuild.executor.python_nodes._helpers.node_warehouse import node_runtime
from sqlbuild.executor.python_nodes._helpers.parallel_items import parallel_set
from sqlbuild.executor.python_nodes.classes.discovered_item_pool import DiscoveredItemPool
from sqlbuild.executor.python_nodes.classes.ingress_results import IngressResultAccumulator
from sqlbuild.executor.python_nodes.main._ready import run_ready_python_node
from sqlbuild.executor.python_nodes.models import (
    DiscoveredItemCallbacks,
    IngressCallbacks,
    PythonIngressLoaderExecutorResult,
    PythonNodeExecutionResult,
    PythonNodeRunState,
    PythonNodeRuntime,
)
from sqlbuild.executor.python_nodes.types import ExecutablePythonNode
from sqlbuild.executor.scheduling.main._run_lifecycle import run_lifecycle_scheduler
from sqlbuild.executor.scheduling.models import (
    LifecycleConcurrency,
    LifecycleExecutionNode,
    LifecycleNodeResult,
    LifecycleSchedulerResult,
)
from sqlbuild.executor.scheduling.types import ExecutionStatus, LifecycleNodeStatus
from sqlbuild.runtime.contracts.types import ExecutionResourceKind
from sqlbuild.runtime.observability.classes.resource_attempt_lifecycle import (
    ResourceAttemptLifecycle,
)
from sqlbuild.runtime.observability.main.current_execution_identity import (
    current_execution_identity,
)
from sqlbuild.runtime.observability.models import ExecutionIdentity


def execute_ingress_python_loader_nodes(
    *,
    python_graph: PythonNodeGraph,
    selected_python_names: frozenset[str],
    loader_functions: tuple[DiscoveredLoaderFunction, ...],
    source_map: Mapping[str, SourceEntry],
    runtime: PythonNodeRuntime,
    callbacks: IngressCallbacks | None = None,
) -> PythonIngressLoaderExecutorResult:
    """Execute Python ingress asset and loader nodes in lifecycle topological order."""

    resolved_callbacks: IngressCallbacks = IngressCallbacks() if callbacks is None else callbacks
    run_state: PythonNodeRunState = PythonNodeRunState()
    loader_by_name: dict[str, DiscoveredLoaderFunction] = {
        loader.name: loader for loader in loader_functions
    }
    source_by_loader_name: dict[str, SourceEntry] = {
        source.loader: source for source in source_map.values() if source.loader is not None
    }
    lifecycle_nodes: tuple[LifecycleExecutionNode, ...] = build_ingress_lifecycle_nodes(
        plan=PythonSqlRunLifecyclePlan(
            ingress_python_node_names=selected_python_names,
            ingress_loader_names=frozenset(
                name
                for name in selected_python_names
                if python_graph.nodes_by_name[name].kind == PythonNodeKind.LOADER
            ),
            read_side_python_node_names=frozenset(),
        ),
        python_graph=python_graph,
    )
    results: IngressResultAccumulator = IngressResultAccumulator()
    resolved_result_store: Any | None = (
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
    node_runtime: PythonNodeRuntime = replace(runtime, result_store=resolved_result_store)

    scheduler_result: LifecycleSchedulerResult = run_lifecycle_scheduler(
        nodes=lifecycle_nodes,
        handler=lambda node: _execute_ingress_lifecycle_node(
            node=node,
            python_graph=python_graph,
            loader_by_name=loader_by_name,
            source_by_loader_name=source_by_loader_name,
            source_map=source_map,
            runtime=node_runtime,
            callbacks=resolved_callbacks,
            run_state=run_state,
            results=results,
        ),
        concurrency=_item_concurrency(
            nodes=lifecycle_nodes,
            python_graph=python_graph,
            runtime=runtime,
            callbacks=resolved_callbacks,
            run_state=run_state,
            results=results,
        ),
    )
    _record_scheduler_skips(
        scheduler_result=scheduler_result,
        python_graph=python_graph,
        source_by_loader_name=source_by_loader_name,
        results=results,
        result_store=resolved_result_store,
        run_id=runtime.run_id,
    )
    return PythonIngressLoaderExecutorResult(
        python_results=tuple(results.python_results_by_name.values()),
        load_results=tuple(results.load_results_by_name.values()),
        run_state=run_state,
    )


def _item_concurrency(
    *,
    nodes: tuple[LifecycleExecutionNode, ...],
    python_graph: PythonNodeGraph,
    runtime: PythonNodeRuntime,
    callbacks: IngressCallbacks,
    run_state: PythonNodeRunState,
    results: IngressResultAccumulator,
) -> LifecycleConcurrency | None:
    """How ingress runs discovered items beside each other; `None` when none may."""

    parallel: dict[str, tuple[str, int]] = {}
    lifecycle_node: LifecycleExecutionNode
    for lifecycle_node in nodes:
        discovered_node: DiscoveredPythonNode = python_graph.nodes_by_name[lifecycle_node.name]
        limited: tuple[str, int] | None = parallel_set(
            discovered=None if discovered_node.asset is None else discovered_node.asset.discovered,
            runtime=runtime,
        )
        if limited is not None:
            parallel[lifecycle_node.name] = limited
    if not parallel:
        return None
    by_name: dict[str, LifecycleExecutionNode] = {node.name: node for node in nodes}

    def run(
        ready: Callable[[], tuple[LifecycleExecutionNode, ...]],
        complete: Callable[[LifecycleExecutionNode, LifecycleNodeResult], None],
    ) -> None:
        def complete_item(node: ExecutablePythonNode, result: PythonNodeExecutionResult) -> None:
            _finalize_ingress_python_result(
                node=node,
                result=result,
                discovered_node=python_graph.nodes_by_name[node.name],
                runtime=runtime,
                callbacks=callbacks,
                run_state=run_state,
                results=results,
            )
            if callbacks.on_node_complete is not None:
                callbacks.on_node_complete(result)
            complete(by_name[node.name], _python_result_to_lifecycle_result(result))

        DiscoveredItemPool(
            runtime=runtime,
            run_state=run_state,
            callbacks=DiscoveredItemCallbacks(
                ready=lambda: tuple(
                    _to_executable_python_node(python_graph.nodes_by_name[node.name])
                    for node in ready()
                ),
                upstream_results=lambda name: _python_upstream_results(
                    name=name, python_graph=python_graph, results=results
                ),
                complete=complete_item,
                cancelled=callbacks.on_node_cancelled,
            ),
        ).run(thread_limit=sum(dict(parallel.values()).values()))

    return LifecycleConcurrency(eligible=lambda node: node.name in parallel, run=run)


def _python_upstream_results(
    *, name: str, python_graph: PythonNodeGraph, results: IngressResultAccumulator
) -> tuple[PythonNodeExecutionResult, ...]:
    return tuple(
        results.python_results_by_name[upstream_name]
        for upstream_name in python_graph.upstream_deps.get(name, ())
        if upstream_name in results.python_results_by_name
    )


def _execute_ingress_lifecycle_node(
    *,
    node: LifecycleExecutionNode,
    python_graph: PythonNodeGraph,
    loader_by_name: dict[str, DiscoveredLoaderFunction],
    source_by_loader_name: dict[str, SourceEntry],
    source_map: Mapping[str, SourceEntry],
    runtime: PythonNodeRuntime,
    callbacks: IngressCallbacks,
    run_state: PythonNodeRunState,
    results: IngressResultAccumulator,
) -> LifecycleNodeResult:
    discovered_node: DiscoveredPythonNode = python_graph.nodes_by_name[node.name]
    if discovered_node.kind == PythonNodeKind.LOADER:
        return _execute_ingress_loader(
            node=discovered_node,
            loader_by_name=loader_by_name,
            source_by_loader_name=source_by_loader_name,
            source_map=source_map,
            runtime=runtime,
            callbacks=callbacks,
            results=results,
        )
    return _execute_ingress_python_node(
        node=discovered_node,
        python_graph=python_graph,
        runtime=runtime,
        callbacks=callbacks,
        run_state=run_state,
        results=results,
    )


def _execute_ingress_python_node(
    *,
    node: DiscoveredPythonNode,
    python_graph: PythonNodeGraph,
    runtime: PythonNodeRuntime,
    callbacks: IngressCallbacks,
    run_state: PythonNodeRunState,
    results: IngressResultAccumulator,
) -> LifecycleNodeResult:
    executable_node: ExecutablePythonNode = _to_executable_python_node(node)
    upstream_results: tuple[PythonNodeExecutionResult, ...] = _python_upstream_results(
        name=node.name, python_graph=python_graph, results=results
    )

    result: PythonNodeExecutionResult = run_ready_python_node(
        node=executable_node,
        upstream_results=upstream_results,
        runtime=runtime,
        statement_recorder=StatementRecorder(),
        run_state=run_state,
        on_result=partial(
            _finalize_ingress_python_result,
            discovered_node=node,
            runtime=runtime,
            callbacks=callbacks,
            run_state=run_state,
            results=results,
        ),
    )
    if callbacks.on_node_complete is not None:
        callbacks.on_node_complete(result)
    return _python_result_to_lifecycle_result(result)


def _finalize_ingress_python_result(
    *,
    node: ExecutablePythonNode,
    result: PythonNodeExecutionResult,
    discovered_node: DiscoveredPythonNode,
    runtime: PythonNodeRuntime,
    callbacks: IngressCallbacks,
    run_state: PythonNodeRunState,
    results: IngressResultAccumulator,
) -> None:
    run_state.record_result(result=result)
    results.record_python_result(name=discovered_node.name, result=result)
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
        if callbacks.identity_recorder is not None:
            callbacks.identity_recorder(identity=discovered_node.identity, _target_name=None)
        else:
            try_write_python_node_identity_fingerprint(
                identity=discovered_node.identity,
                adapter=runtime.adapter,
                connection=runtime.connection,
                run_id=runtime.run_id,
                database=runtime.default_database,
                schema=runtime.default_schema,
            )


def _execute_ingress_loader(
    *,
    node: DiscoveredPythonNode,
    loader_by_name: dict[str, DiscoveredLoaderFunction],
    source_by_loader_name: dict[str, SourceEntry],
    source_map: Mapping[str, SourceEntry],
    runtime: PythonNodeRuntime,
    callbacks: IngressCallbacks,
    results: IngressResultAccumulator,
) -> LifecycleNodeResult:
    loader: DiscoveredLoaderFunction | None = loader_by_name.get(node.name)
    if loader is None:
        raise ExecutorInputError(f"No loader function found for Python ingress node '{node.name}'")
    source_entry: SourceEntry | None = source_by_loader_name.get(loader.name) or source_map.get(
        loader.name
    )
    if source_entry is None:
        raise ExecutorInputError(f"No source entry found for Python ingress loader '{loader.name}'")
    resource_kind: ExecutionResourceKind = load_resource_kind(source_entry)
    identity: ExecutionIdentity | None = current_execution_identity()
    lifecycle: AbstractContextManager[ResourceAttemptLifecycle | None] = (
        nullcontext(None)
        if identity is not None and identity.resource_attempt_id is not None
        else ResourceAttemptLifecycle(
            resource_id=f"source:{source_entry.name}",
            resource_kind=resource_kind.value,
            resource_name=source_entry.name,
            run_id=runtime.run_id,
        )
    )
    with lifecycle as attempt:
        if callbacks.on_node_start is not None:
            callbacks.on_node_start(name=source_entry.name, resource_kind=resource_kind)
        with (
            output_node_scope(f"source:{loader.name}"),
            CostContext.resource_scope(
                resource_type=PythonNodeKind.LOADER.value,
                resource_name=loader.name,
                phase="load",
            ),
        ):
            runtime, warehouse_failure = node_runtime(runtime=runtime, name=node.name)
            result: LoadExecutionResult = (
                LoadExecutionResult(
                    source_name=source_entry.name,
                    loader_name=loader.name,
                    status=ExecutionStatus.FAILED,
                    target=source_entry.table or source_entry.name,
                    resource_kind=resource_kind,
                    error_message=warehouse_failure,
                )
                if warehouse_failure is not None
                else execute_source_load(
                    source_entry=source_entry,
                    loader_function=loader,
                    adapter=runtime.adapter,
                    connection_config=runtime.connection_config,
                    connection=runtime.connection,
                    runtime=LoadRuntimeParams(
                        run_id=runtime.run_id,
                        target=runtime.target,
                        target_config=runtime.target_config,
                        default_database=runtime.default_database,
                        default_schema=runtime.default_schema,
                        vars=runtime.vars,
                        is_reload=runtime.is_reload,
                        start_cursor_ts=runtime.start_cursor_ts,
                        end_cursor_ts=runtime.end_cursor_ts,
                        start_cursor_int=runtime.start_cursor_int,
                        end_cursor_int=runtime.end_cursor_int,
                        use_color=callbacks.use_color,
                        providers=runtime.providers,
                        result_store=runtime.result_store,
                        project_relations=runtime.project_relations,
                    ),
                    statement_recorder=StatementRecorder(),
                    loader_ref_entries=_loader_ref_entries(
                        loader_by_name=loader_by_name,
                        source_by_loader_name=source_by_loader_name,
                    ),
                    source_ref_entries=source_map,
                )
            )
            results.record_load_result(name=node.name, result=result)
            persist_loader_result(
                result_store=runtime.result_store,
                loader_name=loader.name,
                result=result,
                run_id=runtime.run_id,
            )
            if result.status == ExecutionStatus.SUCCESS:
                if callbacks.identity_recorder is not None:
                    callbacks.identity_recorder(
                        identity=node.identity, _target_name=source_entry.name
                    )
                else:
                    try_write_python_node_identity_fingerprint(
                        identity=node.identity,
                        adapter=runtime.adapter,
                        connection=runtime.connection,
                        run_id=runtime.run_id,
                        database=runtime.default_database,
                        schema=runtime.default_schema,
                        target_name=source_entry.name,
                    )
        if attempt is not None and result.status == ExecutionStatus.FAILED:
            attempt.failed()
        elif attempt is not None and result.status == ExecutionStatus.SKIPPED:
            attempt.skipped(
                skip_code="explicit",
                skip_mode=result.skip_mode.value if result.skip_mode is not None else None,
            )
    if callbacks.on_node_complete is not None:
        callbacks.on_node_complete(result)
    return _load_result_to_lifecycle_result(node_name=node.name, result=result)


def _record_scheduler_skips(
    *,
    scheduler_result: LifecycleSchedulerResult,
    python_graph: PythonNodeGraph,
    source_by_loader_name: dict[str, SourceEntry],
    results: IngressResultAccumulator,
    result_store: Any | None,
    run_id: str,
) -> None:
    result: LifecycleNodeResult
    for result in scheduler_result.results:
        if result.status != LifecycleNodeStatus.SKIPPED:
            continue
        discovered_node: DiscoveredPythonNode = python_graph.nodes_by_name[result.name]
        if discovered_node.kind == PythonNodeKind.LOADER:
            source_entry: SourceEntry | None = source_by_loader_name.get(result.name)
            if source_entry is not None and result.name not in results.load_results_by_name:
                skip_mode: SkipMode = result.skip_mode or SkipMode.HARD
                with ResourceAttemptLifecycle(
                    resource_id=f"source:{source_entry.name}",
                    resource_kind=load_resource_kind(source_entry).value,
                    resource_name=source_entry.name,
                    run_id=run_id,
                ) as lifecycle:
                    skipped_result: LoadExecutionResult = skipped_load_result(
                        source=source_entry,
                        reason=result.skip_reason,
                        mode=skip_mode,
                    )
                    results.record_load_result(name=result.name, result=skipped_result)
                    with (
                        output_node_scope(f"source:{result.name}"),
                        CostContext.resource_scope(
                            resource_type=PythonNodeKind.LOADER.value,
                            resource_name=result.name,
                            phase="finalize",
                        ),
                    ):
                        persist_loader_result(
                            result_store=result_store,
                            loader_name=result.name,
                            result=skipped_result,
                            run_id=run_id,
                        )
                    lifecycle.skipped(skip_code="scheduler", skip_mode=skip_mode.value)
            continue
        if result.name not in results.python_results_by_name:
            skip_mode = result.skip_mode or SkipMode.HARD
            with ResourceAttemptLifecycle(
                resource_id=f"{discovered_node.kind.value}:{discovered_node.name}",
                resource_kind=discovered_node.kind.value,
                resource_name=discovered_node.name,
                run_id=run_id,
            ) as lifecycle:
                results.record_python_result(
                    name=result.name,
                    result=PythonNodeExecutionResult(
                        node_name=result.name,
                        kind=discovered_node.kind,
                        status=PythonNodeStatus.SKIPPED,
                        skip_reason=result.skip_reason,
                        skip_mode=skip_mode,
                    ),
                )
                lifecycle.skipped(skip_code="scheduler", skip_mode=skip_mode.value)


def _to_executable_python_node(node: DiscoveredPythonNode) -> ExecutablePythonNode:
    return DiscoveredAssetFunction(
        file_path=node.file_path,
        relative_path=node.relative_path,
        name=node.name,
        function=node.function,
        depends_on=node.depends_on,
        tags=node.tags,
        group=node.group,
        description=node.description,
        meta=node.meta,
        columns=node.asset.columns if node.asset is not None else (),
        column_lineage=node.asset.column_lineage if node.asset is not None else None,
        retry=node.asset.retry if node.asset is not None else None,
        discovered=node.asset.discovered if node.asset is not None else None,
    )


def _python_result_to_lifecycle_result(result: PythonNodeExecutionResult) -> LifecycleNodeResult:
    status: LifecycleNodeStatus = _python_status_to_lifecycle_status(result.status)
    return LifecycleNodeResult(
        name=result.node_name,
        kind=result.kind.value,
        status=status,
        error_message=result.error_message,
        skip_reason=result.skip_reason,
        skip_mode=result.skip_mode,
    )


def _load_result_to_lifecycle_result(
    *, node_name: str, result: LoadExecutionResult
) -> LifecycleNodeResult:
    return LifecycleNodeResult(
        name=node_name,
        kind=PythonNodeKind.LOADER.value,
        status=_execution_status_to_lifecycle_status(result.status),
        error_message=result.error_message,
        skip_reason=result.skip_reason,
        skip_mode=result.skip_mode,
    )


def _python_status_to_lifecycle_status(status: PythonNodeStatus) -> LifecycleNodeStatus:
    if status == PythonNodeStatus.SUCCESS:
        return LifecycleNodeStatus.SUCCESS
    if status == PythonNodeStatus.SKIPPED:
        return LifecycleNodeStatus.SKIPPED
    return LifecycleNodeStatus.FAILED


def _execution_status_to_lifecycle_status(status: ExecutionStatus) -> LifecycleNodeStatus:
    if status == ExecutionStatus.SUCCESS:
        return LifecycleNodeStatus.SUCCESS
    if status == ExecutionStatus.SKIPPED:
        return LifecycleNodeStatus.SKIPPED
    return LifecycleNodeStatus.FAILED


def _loader_ref_entries(
    *,
    loader_by_name: dict[str, DiscoveredLoaderFunction],
    source_by_loader_name: dict[str, SourceEntry],
) -> dict[Callable[..., object], SourceEntry]:
    return {
        loader.function: source_entry
        for loader_name, source_entry in source_by_loader_name.items()
        if (loader := loader_by_name.get(loader_name)) is not None
    }
