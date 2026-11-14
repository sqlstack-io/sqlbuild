"""Reads of a Python node's upstream results, looked up by the upstream's decorated function."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any

from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.node_results.constants import RESULT_DEPENDENCY_ATTRIBUTES
from sqlbuild.executor.node_results.models import NodeResultEnvelope
from sqlbuild.executor.python_nodes.constants import MISSING_DEFAULT


class DependencyResultReader:
    """Mixin for node contexts that read their upstream results from a `result_store`."""

    result_store: Any | None

    def result_of(
        self,
        *,
        node_function: Callable[..., object],
        run_id: str | None = None,
        default: object = MISSING_DEFAULT,
    ) -> NodeResultEnvelope | object:
        """Return the latest persisted upstream result by Python node function reference."""

        if self.result_store is None:
            if default is not MISSING_DEFAULT:
                return default
            raise ExecutorInputError("No Python node result store is available")
        node_type, node_name = self.result_dependency_identity(node_function)
        return self.result_store.result_of(
            node_type=node_type, node_name=node_name, run_id=run_id, default=default
        )

    def results_of(
        self,
        *,
        node_function: Callable[..., object],
        limit: int,
    ) -> tuple[NodeResultEnvelope, ...]:
        """Return persisted successful upstream result history, newest first."""

        if self.result_store is None:
            return ()
        node_type, node_name = self.result_dependency_identity(node_function)
        return self.result_store.results_of(node_type=node_type, node_name=node_name, limit=limit)

    def result_dependency_identity(self, node_function: Callable[..., object]) -> tuple[str, str]:
        """Return the `(kind, name)` a dependency's results are stored under."""

        for attribute, kind in RESULT_DEPENDENCY_ATTRIBUTES:
            definition: object = getattr(node_function, attribute, None)
            if definition is None:
                continue
            name: object = getattr(definition, "name", None)
            if not isinstance(name, str):
                raise ExecutorInputError("Python node result dependency has no resolved name")
            return kind, name
        raise ExecutorInputError("Python node result dependency must be a task, asset, or loader")
