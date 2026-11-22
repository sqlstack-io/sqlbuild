"""Public executor entrypoint for Python read-side SQL-read Python tracking."""

from __future__ import annotations

from collections.abc import Callable

from sqlbuild._native import PythonNodeGraph
from sqlbuild.executor.python_nodes.classes.read_side_python_execution_tracker import (
    ReadSidePythonExecutionTracker,
)
from sqlbuild.executor.python_nodes.models import PythonNodeRuntime
from sqlbuild.executor.python_nodes.types import PythonIdentityRecorder


def create_read_side_python_execution_tracker(
    *,
    python_graph: PythonNodeGraph,
    selected_python_names: frozenset[str],
    scheduled_sql_names: frozenset[str],
    runtime: PythonNodeRuntime,
    identity_recorder: PythonIdentityRecorder | None = None,
    on_node_complete: Callable[[object], None] | None = None,
    on_asset_start: Callable[[str], None] | None = None,
    on_node_cancelled: Callable[[str], None] | None = None,
) -> ReadSidePythonExecutionTracker:
    """Create a read-side Python execution tracker."""

    return ReadSidePythonExecutionTracker(
        python_graph=python_graph,
        selected_python_names=selected_python_names,
        scheduled_sql_names=scheduled_sql_names,
        runtime=runtime,
        identity_recorder=identity_recorder,
        on_node_complete=on_node_complete,
        on_asset_start=on_asset_start,
        on_node_cancelled=on_node_cancelled,
    )
