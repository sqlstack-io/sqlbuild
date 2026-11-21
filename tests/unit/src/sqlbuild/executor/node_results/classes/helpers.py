from __future__ import annotations

from datetime import datetime

from sqlbuild.executor.node_results.classes.node_result_store import NodeResultStore
from sqlbuild.executor.node_results.models import (
    NodeResultEnvelope,
    NodeResultQuery,
    NodeResultRecord,
)


class InMemoryNodeResultStore(NodeResultStore):
    def __init__(self, *, persisted_results: tuple[NodeResultEnvelope, ...] = ()) -> None:
        super().__init__(database="analytics", schema="state", target_name="dev")
        self.persisted_results: tuple[NodeResultEnvelope, ...] = persisted_results
        self.queries: list[NodeResultQuery] = []

    def write(self, record: NodeResultRecord) -> None:
        self._cache_record(record=record)

    def _read_persisted(self, *, query: NodeResultQuery) -> tuple[NodeResultEnvelope, ...]:
        self.queries.append(query)
        return self.persisted_results


def build_node_result_record(
    *,
    run_id: str,
    status: str,
    payload: object | None = None,
    error_message: str | None = None,
    materialized: bool | None = None,
) -> NodeResultRecord:
    return NodeResultRecord(
        node_type="asset",
        node_name="orders",
        target_database="analytics",
        target_schema="state",
        target_name="dev",
        run_id=run_id,
        status=status,
        payload=payload,
        metadata={"run_id": run_id},
        error_message=error_message,
        materialized=materialized,
        ts=datetime(2026, 7, 19, 12, 0),
    )


def envelope_from_record(*, record: NodeResultRecord) -> NodeResultEnvelope:
    return NodeResultEnvelope(
        node_type=record.node_type,
        node_name=record.node_name,
        run_id=record.run_id,
        status=record.status,
        payload=record.payload,
        metadata=record.metadata,
        error_message=record.error_message,
        materialized=record.materialized,
        ts=record.ts,
    )
