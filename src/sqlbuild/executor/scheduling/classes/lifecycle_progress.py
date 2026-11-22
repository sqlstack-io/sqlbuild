"""What a lifecycle scheduler has finished, and which nodes that leaves ready."""

from __future__ import annotations

from collections.abc import Callable

from sqlbuild.executor.scheduling.models import LifecycleExecutionNode, LifecycleNodeResult


class LifecycleProgress:
    """The scheduler's ready nodes and results; only the scheduler's thread uses it."""

    def __init__(
        self,
        *,
        node_by_name: dict[str, LifecycleExecutionNode],
        upstream_names: dict[str, tuple[str, ...]],
        downstream_names: dict[str, tuple[str, ...]],
        skip_result: Callable[..., LifecycleNodeResult | None],
    ) -> None:
        self._node_by_name: dict[str, LifecycleExecutionNode] = node_by_name
        self._downstream_names: dict[str, tuple[str, ...]] = downstream_names
        self._skip_result: Callable[..., LifecycleNodeResult | None] = skip_result
        self._in_degree: dict[str, int] = {
            name: len(upstreams) for name, upstreams in upstream_names.items()
        }
        self._results_by_name: dict[str, LifecycleNodeResult] = {}
        self.ready: list[str] = sorted(
            name for name, degree in self._in_degree.items() if degree == 0
        )
        self.results: list[LifecycleNodeResult] = []

    def skip(self, node: LifecycleExecutionNode) -> LifecycleNodeResult | None:
        """The result of `node` when its upstream results skip it."""

        return self._skip_result(node=node, results_by_name=self._results_by_name)

    def complete(self, *, node: LifecycleExecutionNode, result: LifecycleNodeResult) -> None:
        """Record `result` once, as a cancel may offer it again, and ready what waited on `node`."""

        if node.name not in self.ready:
            return
        self.ready.remove(node.name)
        self._results_by_name[node.name] = result
        self.results.append(result)
        downstream_name: str
        for downstream_name in self._downstream_names[node.name]:
            self._in_degree[downstream_name] -= 1
            if self._in_degree[downstream_name] == 0:
                self.ready.append(downstream_name)
        self.ready.sort()

    def startable(
        self, *, eligible: Callable[[LifecycleExecutionNode], bool]
    ) -> tuple[LifecycleExecutionNode, ...]:
        """The eligible ready nodes, once those their upstream results skip are recorded."""

        while True:
            candidates: list[LifecycleExecutionNode] = []
            skipped: bool = False
            name: str
            for name in tuple(self.ready):
                node: LifecycleExecutionNode = self._node_by_name[name]
                if not eligible(node):
                    continue
                result: LifecycleNodeResult | None = self.skip(node)
                if result is None:
                    candidates.append(node)
                    continue
                self.complete(node=node, result=result)
                skipped = True
            if not skipped:
                return tuple(candidates)
