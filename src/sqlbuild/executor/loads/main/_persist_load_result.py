"""Record one load's run as its node result, holding SQLLoad's report of each table."""

from __future__ import annotations

from sqlbuild.executor.load.models import LoadExecutionResult
from sqlbuild.executor.node_results.classes.direct_store import DirectNodeResultStore
from sqlbuild.executor.node_results.models import NodeResultRecord
from sqlbuild.runtime.contracts.types import ExecutionResourceKind


def persist_load_result(
    *, result_store: DirectNodeResultStore | None, result: LoadExecutionResult, run_id: str
) -> None:
    """Write `result` to the store's location as a `load` node; do nothing without a live store."""
    if result_store is None or result_store.connection is None:
        return
    result_store.write(
        NodeResultRecord(
            node_type=ExecutionResourceKind.LOAD.value,
            node_name=result.source_name,
            target_database=result_store.database,
            target_schema=result_store.schema,
            target_name=None,
            run_id=run_id,
            status=result.status.value,
            payload=result.result_payload,
            metadata={"rows_loaded": result.rows_loaded, **result.result_metadata},
            error_message=result.error_message,
            materialized=result.result_materialized,
        )
    )
