"""Public operation for writing a loader's or a load's result as a node result."""

from __future__ import annotations

from sqlbuild.executor.load.models import LoadExecutionResult
from sqlbuild.executor.node_results.classes.direct_store import DirectNodeResultStore
from sqlbuild.executor.node_results.models import NodeResultRecord


def persist_node_result(
    *,
    result_store: DirectNodeResultStore | None,
    node: tuple[str, str],
    result: LoadExecutionResult,
    run_id: str,
    details: tuple[dict[str, object], str | None],
) -> None:
    """Write `result` as `node` (type, name) with `details` (metadata, error) to a live store."""

    if result_store is None or result_store.connection is None:
        return
    node_type, node_name = node
    metadata, error_message = details
    result_store.write(
        NodeResultRecord(
            node_type=node_type,
            node_name=node_name,
            target_database=result_store.database,
            target_schema=result_store.schema,
            target_name=None,
            run_id=run_id,
            status=result.status.value,
            payload=result.result_payload,
            metadata=metadata,
            error_message=error_message,
            materialized=result.result_materialized,
        )
    )
