"""Record one load's run as its node result, holding SQLLoad's report of each table."""

from __future__ import annotations

from sqlbuild.executor.load.main._persist_node_result import persist_node_result
from sqlbuild.executor.load.models import LoadExecutionResult
from sqlbuild.executor.node_results.classes.direct_store import DirectNodeResultStore
from sqlbuild.runtime.contracts.types import ExecutionResourceKind


def persist_load_result(
    *, result_store: DirectNodeResultStore | None, result: LoadExecutionResult, run_id: str
) -> None:
    """Write `result` to the store's location as a `load` node; do nothing without a live store."""
    _ = persist_node_result(
        result_store=result_store,
        node=(ExecutionResourceKind.LOAD.value, result.source_name),
        result=result,
        run_id=run_id,
        details=(
            {"rows_loaded": result.rows_loaded, **result.result_metadata},
            result.error_message,
        ),
    )
