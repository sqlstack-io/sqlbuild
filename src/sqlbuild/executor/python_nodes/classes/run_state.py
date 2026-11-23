"""Mutable Python node run state."""

from __future__ import annotations

from sqlbuild.executor.python_nodes.models import PythonNodeExecutionResult


class PythonNodeRunState:
    """Same-run result store for Python DAG nodes."""

    def __init__(
        self,
        results_by_name: dict[str, PythonNodeExecutionResult] | None = None,
    ) -> None:
        self.results_by_name = results_by_name or {}

    def record_result(self, *, result: PythonNodeExecutionResult) -> None:
        """Record `result` under its node's name, as the items of one asset share its function."""

        self.results_by_name[result.node_name] = result
