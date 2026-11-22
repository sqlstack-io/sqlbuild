"""Generic serial scheduler for lifecycle-aware mixed execution graphs."""

from __future__ import annotations

from collections.abc import Callable

from sqlbuild.compiler.python_nodes.types import SkipMode
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.scheduling.models import (
    LifecycleConcurrency,
    LifecycleExecutionNode,
    LifecycleNodeResult,
    LifecycleSchedulerResult,
)
from sqlbuild.executor.scheduling.types import LifecycleNodeStatus


def run_lifecycle_scheduler(
    *,
    nodes: tuple[LifecycleExecutionNode, ...],
    handler: Callable[[LifecycleExecutionNode], LifecycleNodeResult],
    concurrency: LifecycleConcurrency | None = None,
) -> LifecycleSchedulerResult:
    """Run lifecycle nodes serially in topological order, except those `concurrency` runs."""

    node_by_name: dict[str, LifecycleExecutionNode] = {node.name: node for node in nodes}
    if len(node_by_name) != len(nodes):
        raise ExecutorInputError("Lifecycle scheduler node names must be unique")
    upstream_names: dict[str, tuple[str, ...]] = _build_upstream_names(
        nodes=nodes,
        node_by_name=node_by_name,
    )
    downstream_names: dict[str, tuple[str, ...]] = _build_downstream_names(
        node_names=tuple(node.name for node in nodes),
        upstream_names=upstream_names,
    )
    in_degree: dict[str, int] = {
        node_name: len(upstreams) for node_name, upstreams in upstream_names.items()
    }
    ready: list[str] = sorted(name for name, degree in in_degree.items() if degree == 0)
    results_by_name: dict[str, LifecycleNodeResult] = {}
    ordered_results: list[LifecycleNodeResult] = []

    def complete(node: LifecycleExecutionNode, result: LifecycleNodeResult) -> None:
        ready.remove(node.name)
        results_by_name[node.name] = result
        ordered_results.append(result)
        downstream_name: str
        for downstream_name in downstream_names[node.name]:
            in_degree[downstream_name] -= 1
            if in_degree[downstream_name] == 0:
                ready.append(downstream_name)
        ready.sort()

    def startable() -> tuple[LifecycleExecutionNode, ...]:
        if concurrency is None:
            return ()
        while True:
            candidates: tuple[LifecycleExecutionNode, ...] = tuple(
                node_by_name[name] for name in ready if concurrency.eligible(node_by_name[name])
            )
            skips: list[tuple[LifecycleExecutionNode, LifecycleNodeResult]] = []
            candidate: LifecycleExecutionNode
            for candidate in candidates:
                skip: LifecycleNodeResult | None = _skip_result(
                    node=candidate, results_by_name=results_by_name
                )
                if skip is not None:
                    skips.append((candidate, skip))
            if not skips:
                return candidates
            for candidate, skip in skips:
                complete(candidate, skip)

    while ready:
        node: LifecycleExecutionNode = node_by_name[ready[0]]
        skipped: LifecycleNodeResult | None = _skip_result(
            node=node, results_by_name=results_by_name
        )
        if skipped is not None:
            complete(node, skipped)
        elif concurrency is not None and concurrency.eligible(node):
            concurrency.run(startable, complete)
        else:
            complete(node, handler(node))

    if len(ordered_results) != len(nodes):
        raise ExecutorInputError("Lifecycle scheduler could not resolve all dependencies")
    return LifecycleSchedulerResult(results=tuple(ordered_results))


def _skip_result(
    *, node: LifecycleExecutionNode, results_by_name: dict[str, LifecycleNodeResult]
) -> LifecycleNodeResult | None:
    """The result of a node its upstream results skip, or `None` when it runs."""

    blocking: LifecycleNodeResult | None = _blocking_dependency_result(
        upstream_names=node.upstream_names, results_by_name=results_by_name
    )
    if blocking is not None:
        return LifecycleNodeResult(
            name=node.name,
            kind=node.kind,
            status=LifecycleNodeStatus.SKIPPED,
            skip_reason=_dependency_skip_reason(blocking),
            skip_mode=SkipMode.HARD,
        )
    if _should_soft_skip_due_to_all_skipped_dependencies(
        upstream_names=node.upstream_names, results_by_name=results_by_name
    ):
        return LifecycleNodeResult(
            name=node.name,
            kind=node.kind,
            status=LifecycleNodeStatus.SKIPPED,
            skip_reason="All upstream nodes were skipped",
            skip_mode=SkipMode.SOFT,
        )
    return None


def _build_upstream_names(
    *,
    nodes: tuple[LifecycleExecutionNode, ...],
    node_by_name: dict[str, LifecycleExecutionNode],
) -> dict[str, tuple[str, ...]]:
    upstream_names: dict[str, tuple[str, ...]] = {}
    node: LifecycleExecutionNode
    for node in nodes:
        upstream_name: str
        for upstream_name in node.upstream_names:
            if upstream_name not in node_by_name:
                raise ExecutorInputError(
                    f"Lifecycle node '{node.name}' depends on unknown node '{upstream_name}'"
                )
        upstream_names[node.name] = node.upstream_names
    return upstream_names


def _build_downstream_names(
    *, node_names: tuple[str, ...], upstream_names: dict[str, tuple[str, ...]]
) -> dict[str, tuple[str, ...]]:
    downstream: dict[str, list[str]] = {node_name: [] for node_name in node_names}
    node_name: str
    upstreams: tuple[str, ...]
    for node_name, upstreams in upstream_names.items():
        upstream_name: str
        for upstream_name in upstreams:
            downstream[upstream_name].append(node_name)
    return {node_name: tuple(names) for node_name, names in downstream.items()}


def _blocking_dependency_result(
    *,
    upstream_names: tuple[str, ...],
    results_by_name: dict[str, LifecycleNodeResult],
) -> LifecycleNodeResult | None:
    upstream_name: str
    for upstream_name in upstream_names:
        result: LifecycleNodeResult = results_by_name[upstream_name]
        if result.status == LifecycleNodeStatus.FAILED:
            return result
        if result.status == LifecycleNodeStatus.SKIPPED and result.skip_mode == SkipMode.HARD:
            return result
    return None


def _should_soft_skip_due_to_all_skipped_dependencies(
    *,
    upstream_names: tuple[str, ...],
    results_by_name: dict[str, LifecycleNodeResult],
) -> bool:
    if not upstream_names:
        return False
    return all(
        results_by_name[upstream_name].status == LifecycleNodeStatus.SKIPPED
        and results_by_name[upstream_name].skip_mode == SkipMode.SOFT
        for upstream_name in upstream_names
    )


def _dependency_skip_reason(result: LifecycleNodeResult) -> str:
    if result.status == LifecycleNodeStatus.SKIPPED:
        return f"Upstream node hard-skipped: {result.name}"
    return f"Upstream node did not succeed: {result.name}"
