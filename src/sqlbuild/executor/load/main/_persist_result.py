"""Public operation for recording a loader's result in the node-result store."""

from __future__ import annotations

from sqlbuild.compiler.python_nodes.types import PythonNodeKind
from sqlbuild.executor.load.models import LoadExecutionResult
from sqlbuild.executor.node_results.classes.direct_store import DirectNodeResultStore
from sqlbuild.executor.node_results.models import NodeResultRecord


def persist_loader_result(
    *,
    result_store: DirectNodeResultStore | None,
    loader_name: str,
    result: LoadExecutionResult,
    run_id: str,
) -> None:
    """Write one loader result to the store's location; do nothing without a live store."""

    if result_store is None or result_store.connection is None:
        return
    result_store.write(
        NodeResultRecord(
            node_type=PythonNodeKind.LOADER.value,
            node_name=loader_name,
            target_database=result_store.database,
            target_schema=result_store.schema,
            target_name=None,
            run_id=run_id,
            status=result.status.value,
            payload=result.result_payload,
            metadata={
                "source_name": result.source_name,
                "loader_name": result.loader_name,
                "rows_loaded": result.rows_loaded,
                "target": result.target,
                **result.result_metadata,
            },
            error_message=result.error_message or result.skip_reason,
            materialized=result.result_materialized,
        )
    )
