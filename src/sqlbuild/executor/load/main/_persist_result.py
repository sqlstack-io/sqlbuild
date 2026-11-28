"""Public operation for recording a loader's result in the node-result store."""

from __future__ import annotations

from sqlbuild.compiler.python_nodes.types import PythonNodeKind
from sqlbuild.executor.load.main._persist_node_result import persist_node_result
from sqlbuild.executor.load.models import LoadExecutionResult
from sqlbuild.executor.node_results.classes.direct_store import DirectNodeResultStore


def persist_loader_result(
    *,
    result_store: DirectNodeResultStore | None,
    loader_name: str,
    result: LoadExecutionResult,
    run_id: str,
) -> None:
    """Write one loader result to the store's location; do nothing without a live store."""

    _ = persist_node_result(
        result_store=result_store,
        node=(PythonNodeKind.LOADER.value, loader_name),
        result=result,
        run_id=run_id,
        details=(
            {
                "source_name": result.source_name,
                "loader_name": result.loader_name,
                "rows_loaded": result.rows_loaded,
                "target": result.target,
                **result.result_metadata,
            },
            result.error_message or result.skip_reason,
        ),
    )
