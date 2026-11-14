"""Serial executor helpers for Python task and asset nodes."""

from __future__ import annotations

import logging
import time
from collections.abc import Callable
from dataclasses import replace
from typing import Any

from sqlbuild._native import CostContext, DiscoveredAssetFunction
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.compiler.python_nodes.types import (
    PythonNodeFanInAction,
    PythonNodeKind,
    PythonNodeStatus,
)
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.node_results.main._direct_store import build_direct_node_result_store
from sqlbuild.executor.node_results.models import NodeResultRecord
from sqlbuild.executor.node_results.types import NodeResultStatus
from sqlbuild.executor.python_nodes._helpers.relation_guard import build_python_node_relation_guard
from sqlbuild.executor.python_nodes._helpers.results import (
    build_python_node_failure_result,
    evaluate_python_node_fan_in,
    normalize_python_node_return,
)
from sqlbuild.executor.python_nodes.models import (
    AssetContext,
    PythonNodeExecutionResult,
    PythonNodeFanInDecision,
    PythonNodeRunState,
    PythonNodeRuntime,
    TaskContext,
)
from sqlbuild.executor.python_nodes.types import ExecutablePythonNode, OwnedResultCallback
from sqlbuild.provider.main.runtime import (
    ProviderContainer,
    _empty_provider_container,
    invoke_with_providers,
)
from sqlbuild.python_nodes.main.calculate_retry_delay import calculate_retry_delay
from sqlbuild.python_nodes.models import (
    RetryPolicy,
    SqlResourceRef,
)
from sqlbuild.runtime.observability.classes.operation_lifecycle import (
    OperationLifecycle,
    publish_retry_scheduled,
)
from sqlbuild.runtime.observability.classes.resource_attempt_lifecycle import (
    ResourceAttemptLifecycle,
)


def execute_ready_python_node(
    *,
    node: ExecutablePythonNode,
    upstream_results: tuple[PythonNodeExecutionResult, ...],
    runtime: PythonNodeRuntime,
    statement_recorder: StatementRecorder,
    logger: logging.Logger | None = None,
    run_state: PythonNodeRunState | None = None,
    sleep: Callable[[float], None] = time.sleep,
    monotonic: Callable[[], float] = time.monotonic,
    on_result: OwnedResultCallback | None = None,
) -> PythonNodeExecutionResult:
    """Execute one scheduler-ready task/asset node."""

    return _execute_ready_node(
        node=node,
        upstream_results=upstream_results,
        runtime=runtime,
        statement_recorder=statement_recorder,
        logger=logger,
        run_state=run_state if run_state is not None else PythonNodeRunState(),
        result_store=runtime.result_store
        if runtime.result_store is not None
        else (
            build_direct_node_result_store(
                adapter=runtime.adapter,
                connection=runtime.connection,
                database=runtime.default_database,
                schema=runtime.default_schema,
            )
            if runtime.persist_node_results
            else None
        ),
        sleep=sleep,
        monotonic=monotonic,
        on_result=on_result,
    )


def _execute_ready_node(
    *,
    node: ExecutablePythonNode,
    upstream_results: tuple[PythonNodeExecutionResult, ...],
    runtime: PythonNodeRuntime,
    statement_recorder: StatementRecorder,
    logger: logging.Logger | None,
    run_state: PythonNodeRunState,
    result_store: Any | None,
    sleep: Callable[[float], None],
    monotonic: Callable[[], float],
    on_result: OwnedResultCallback | None,
) -> PythonNodeExecutionResult:
    node_kind: PythonNodeKind = _node_kind(node)
    with CostContext.resource_scope(
        resource_type=node_kind.value,
        resource_name=node.name,
        phase="execute",
    ):
        with ResourceAttemptLifecycle(
            resource_id=f"{node_kind.value}:{node.name}",
            resource_kind=node_kind.value,
            resource_name=node.name,
            run_id=runtime.run_id,
        ) as lifecycle:
            result, skip_code = _execute_ready_node_in_cost_scope(
                node=node,
                node_kind=node_kind,
                upstream_results=upstream_results,
                runtime=runtime,
                statement_recorder=statement_recorder,
                logger=logger,
                run_state=run_state,
                result_store=result_store,
                sleep=sleep,
                monotonic=monotonic,
            )
            if on_result is not None:
                on_result(node=node, result=result)
            if result.status == PythonNodeStatus.SKIPPED:
                lifecycle.skipped(
                    skip_code=skip_code or "explicit",
                    skip_mode=result.skip_mode.value if result.skip_mode is not None else None,
                )
            elif result.status == PythonNodeStatus.FAILED:
                lifecycle.failed()
            return result


def _execute_ready_node_in_cost_scope(
    *,
    node: ExecutablePythonNode,
    node_kind: PythonNodeKind,
    upstream_results: tuple[PythonNodeExecutionResult, ...],
    runtime: PythonNodeRuntime,
    statement_recorder: StatementRecorder,
    logger: logging.Logger | None,
    run_state: PythonNodeRunState,
    result_store: Any | None,
    sleep: Callable[[float], None],
    monotonic: Callable[[], float],
) -> tuple[PythonNodeExecutionResult, str | None]:
    run_id: str = runtime.run_id
    providers: ProviderContainer | None = runtime.providers
    decision: PythonNodeFanInDecision = evaluate_python_node_fan_in(
        upstream_results=upstream_results
    )
    if decision.action == PythonNodeFanInAction.SKIP:
        result: PythonNodeExecutionResult = PythonNodeExecutionResult(
            node_name=node.name,
            kind=node_kind,
            status=PythonNodeStatus.SKIPPED,
            skip_mode=decision.skip_mode,
            skip_reason=decision.reason,
        )
        _persist_python_node_result(result_store=result_store, result=result, run_id=run_id)
        return result, "fan_in"
    if decision.action == PythonNodeFanInAction.BLOCK:
        result: PythonNodeExecutionResult = PythonNodeExecutionResult(
            node_name=node.name,
            kind=node_kind,
            status=PythonNodeStatus.FAILED,
            error_message=decision.reason,
        )
        _persist_python_node_result(result_store=result_store, result=result, run_id=run_id)
        return result, None
    warnings: list[str] = []
    context: TaskContext | AssetContext = _build_context(
        node=node,
        node_kind=node_kind,
        runtime=runtime,
        statement_recorder=statement_recorder,
        logger=logger,
        run_state=run_state,
        result_store=result_store,
        warnings=warnings,
    )
    missing: str | None = _missing_upstream_result(
        node=node, node_kind=node_kind, context=context, upstream_results=upstream_results
    )
    if missing is not None:
        result = PythonNodeExecutionResult(
            node_name=node.name,
            kind=node_kind,
            status=PythonNodeStatus.FAILED,
            error_message=missing,
        )
        _persist_python_node_result(result_store=result_store, result=result, run_id=run_id)
        return result, None
    try:
        result: PythonNodeExecutionResult = _call_node_with_retry(
            node=node,
            context=context,
            providers=providers,
            retry_policy=node.retry,
            sleep=sleep,
            monotonic=monotonic,
        )
    except Exception as error:
        result = build_python_node_failure_result(
            node_name=node.name,
            kind=node_kind,
            error=error,
        )
        result = replace(result, warning_messages=tuple(warnings))
        _persist_python_node_result(result_store=result_store, result=result, run_id=run_id)
        return result, None
    result = replace(result, warning_messages=tuple(warnings))
    _persist_python_node_result(result_store=result_store, result=result, run_id=run_id)
    return result, "explicit" if result.status == PythonNodeStatus.SKIPPED else None


def _persist_python_node_result(
    *, result_store: Any | None, result: PythonNodeExecutionResult, run_id: str
) -> None:
    if result_store is None:
        return
    record: NodeResultRecord = NodeResultRecord(
        node_type=result.kind.value,
        node_name=result.node_name,
        target_database=result_store.database,
        target_schema=result_store.schema,
        target_name=None,
        run_id=run_id,
        status=result.status.value,
        payload=result.payload,
        metadata=result.metadata,
        error_message=result.error_message or result.skip_reason,
        materialized=result.materialized,
    )
    try:
        result_store.write(record)
    except Exception as error:
        if result.status != PythonNodeStatus.SUCCESS:
            raise
        result_store.write(
            NodeResultRecord(
                node_type=result.kind.value,
                node_name=result.node_name,
                target_database=result_store.database,
                target_schema=result_store.schema,
                target_name=None,
                run_id=run_id,
                status=NodeResultStatus.FAILED.value,
                payload=None,
                metadata={},
                error_message=str(error),
                materialized=result.materialized,
            )
        )
        raise


def _call_node_with_retry(
    *,
    node: ExecutablePythonNode,
    context: TaskContext | AssetContext,
    providers: ProviderContainer | None,
    retry_policy: RetryPolicy | None,
    sleep: Callable[[float], None],
    monotonic: Callable[[], float],
) -> PythonNodeExecutionResult:
    node_kind: PythonNodeKind = _node_kind(node)
    if retry_policy is None:
        with CostContext.resource_scope(
            resource_type=node_kind.value,
            resource_name=node.name,
            phase="execute",
            attempt=1,
        ):
            with OperationLifecycle(
                operation_kind="python_node",
                operation_name=f"python_{node_kind.value}",
                metadata={"attempt_number": 1},
            ) as operation:
                returned: object = invoke_with_providers(
                    function=node.function,
                    context=context,
                    providers=providers,
                )
                result: PythonNodeExecutionResult = normalize_python_node_return(
                    node_name=node.name,
                    kind=node_kind,
                    returned=returned,
                )
                if result.status == PythonNodeStatus.FAILED:
                    operation.failed()
                return result
    start_time: float = monotonic()
    attempt: int = 1
    while True:
        retry_error: BaseException | None = None
        delay_seconds: float | None = None
        with CostContext.resource_scope(
            resource_type=node_kind.value,
            resource_name=node.name,
            phase="execute",
            attempt=attempt,
        ):
            with OperationLifecycle(
                operation_kind="python_node",
                operation_name=f"python_{node_kind.value}",
                metadata={"attempt_number": attempt},
            ) as operation:
                try:
                    returned = invoke_with_providers(
                        function=node.function,
                        context=context,
                        providers=providers,
                    )
                except retry_policy.retry_on as error:
                    retry_error = error
                    operation.failed(error=error)
                    if attempt >= retry_policy.max_attempts:
                        raise
                    delay_seconds = calculate_retry_delay(
                        retry_policy=retry_policy,
                        retry_index=attempt - 1,
                    )
                    if retry_policy.max_elapsed_seconds is not None:
                        elapsed_seconds: float = monotonic() - start_time
                        if elapsed_seconds + delay_seconds > retry_policy.max_elapsed_seconds:
                            raise
                    publish_retry_scheduled(
                        failed_attempt_number=attempt,
                        next_attempt_number=attempt + 1,
                        delay_ms=max(0, round(delay_seconds * 1000)),
                        error=error,
                    )
                else:
                    result = normalize_python_node_return(
                        node_name=node.name,
                        kind=node_kind,
                        returned=returned,
                    )
                    if result.status == PythonNodeStatus.FAILED:
                        operation.failed()
                    return result
        if retry_error is None or delay_seconds is None:
            raise ExecutorInputError("Python retry attempt ended without a retry decision")
        sleep(delay_seconds)
        attempt += 1


def _missing_upstream_result(
    *,
    node: ExecutablePythonNode,
    node_kind: PythonNodeKind,
    context: TaskContext | AssetContext,
    upstream_results: tuple[PythonNodeExecutionResult, ...],
) -> str | None:
    """Why the node cannot run: a Python upstream outside this run that has never succeeded."""

    python_dependencies: tuple[Callable[..., object], ...] = tuple(
        dependency for dependency in node.depends_on if not isinstance(dependency, SqlResourceRef)
    )
    if len(upstream_results) == len(python_dependencies):
        return None
    in_run: frozenset[str] = frozenset(result.node_name for result in upstream_results)
    for dependency in python_dependencies:
        kind, name = context.result_dependency_identity(dependency)
        if name in in_run or context.result_of(node_function=dependency, default=None) is not None:
            continue
        both: str = f"'sqb build --select +{node_kind.value}:{node.name}'"
        if kind == PythonNodeKind.LOADER.value:
            return (
                f"Upstream loader '{name}' has no successful result to read. Build it with {both}."
            )
        return (
            f"Upstream {kind} '{name}' has no successful result to read. Build it first with "
            f"'sqb build --select {kind}:{name}', or build both with {both}."
        )
    return None


def _build_context(
    *,
    node: ExecutablePythonNode,
    node_kind: PythonNodeKind,
    runtime: PythonNodeRuntime,
    statement_recorder: StatementRecorder,
    logger: logging.Logger | None,
    run_state: PythonNodeRunState,
    result_store: Any | None,
    warnings: list[str],
) -> TaskContext | AssetContext:
    context_logger: logging.Logger = logger or logging.getLogger(
        f"sqlbuild.{node_kind.value}.{node.name}"
    )
    allowed_sql_refs: frozenset[SqlResourceRef] = frozenset(
        dependency for dependency in node.depends_on if isinstance(dependency, SqlResourceRef)
    )
    providers: ProviderContainer = (
        runtime.providers if runtime.providers is not None else _empty_provider_container()
    )
    context_class: type[AssetContext] | type[TaskContext] = (
        AssetContext if node_kind == PythonNodeKind.ASSET else TaskContext
    )
    return context_class(
        adapter=runtime.adapter,
        connection_config=runtime.connection_config,
        connection=runtime.connection,
        run_id=runtime.run_id,
        target=runtime.target,
        vars=runtime.vars,
        is_reload=runtime.is_reload,
        logger=context_logger,
        statement_recorder=statement_recorder,
        run_state=run_state,
        result_store=result_store,
        default_database=runtime.default_database,
        default_schema=runtime.default_schema,
        relation_targets=runtime.resolved_relation_targets,
        allowed_sql_refs=allowed_sql_refs,
        relation_guard=build_python_node_relation_guard(
            owner_label=f"{node_kind.value} '{node.name}'", runtime=runtime, warnings=warnings
        ),
        providers=providers,
        start_cursor_ts=runtime.start_cursor_ts,
        end_cursor_ts=runtime.end_cursor_ts,
        start_cursor_int=runtime.start_cursor_int,
        end_cursor_int=runtime.end_cursor_int,
    )


def _node_kind(node: ExecutablePythonNode) -> PythonNodeKind:
    if isinstance(node, DiscoveredAssetFunction):
        return PythonNodeKind.ASSET
    return PythonNodeKind.TASK
