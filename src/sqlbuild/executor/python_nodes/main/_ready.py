"""Public executor entrypoint for one ready asset Python node."""

from __future__ import annotations

from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.executor.python_nodes._helpers.execution import execute_ready_python_node
from sqlbuild.executor.python_nodes.models import (
    PythonNodeExecutionResult,
    PythonNodeRunState,
    PythonNodeRuntime,
)
from sqlbuild.executor.python_nodes.types import ExecutablePythonNode, OwnedResultCallback


def run_ready_python_node(
    *,
    node: ExecutablePythonNode,
    upstream_results: tuple[PythonNodeExecutionResult, ...],
    runtime: PythonNodeRuntime,
    statement_recorder: StatementRecorder,
    run_state: PythonNodeRunState,
    on_result: OwnedResultCallback | None = None,
) -> PythonNodeExecutionResult:
    """Execute one scheduler-ready asset Python node."""

    return execute_ready_python_node(
        node=node,
        upstream_results=upstream_results,
        runtime=runtime,
        statement_recorder=statement_recorder,
        run_state=run_state,
        on_result=on_result,
    )
