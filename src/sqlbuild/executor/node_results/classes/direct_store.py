"""Direct-mode runtime node result store."""

from __future__ import annotations

from dataclasses import replace
from typing import Any

from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.executor.node_results.classes.node_result_store import NodeResultStore
from sqlbuild.executor.node_results.main._read import read_node_results
from sqlbuild.executor.node_results.main._write import write_node_result_record
from sqlbuild.executor.node_results.models import (
    NodeResultEnvelope,
    NodeResultQuery,
    NodeResultRecord,
)


class DirectNodeResultStore(NodeResultStore):
    """Warehouse-backed node result store for direct-mode execution."""

    def __init__(
        self,
        *,
        adapter: BaseAdapter,
        connection: Any,
        database: str | None,
        schema: str | None,
        previous_location: tuple[str | None, str | None] | None = None,
    ) -> None:
        self.adapter: BaseAdapter = adapter
        self.connection: Any = connection
        self.previous_location: tuple[str | None, str | None] | None = previous_location
        super().__init__(database=database, schema=schema)

    def write(self, record: NodeResultRecord) -> None:
        if self.schema is None:
            return
        self.adapter.ensure_schema(
            connection=self.connection,
            database=self.database,
            schema=self.schema,
            statement_recorder=StatementRecorder(),
        )
        write_node_result_record(
            connection=self.connection,
            execute=self.adapter.execute,
            database=self.database,
            schema=self.schema,
            record=record,
            render_qualified_name=self.adapter.render_qualified_name,
            render_framework_type=self.adapter.render_framework_type,
            render_create_table_sql=self.adapter.render_create_node_result_table_sql,
            render_create_index_sqls=self.adapter.render_create_node_result_index_sqls,
        )
        self._cache_record(record=record)

    def _read_persisted(self, *, query: NodeResultQuery) -> tuple[NodeResultEnvelope, ...]:
        results: tuple[NodeResultEnvelope, ...] = self._read_at(
            database=self.database, schema=self.schema, query=query
        )
        if results or self.previous_location is None:
            return results
        # Results written before this release's location move: read them for one release only,
        # and remove this fallback with the control-store migrations.
        database, schema = self.previous_location
        return self._read_at(
            database=database,
            schema=schema,
            query=replace(query, target_database=database, target_schema=schema),
        )

    def _read_at(
        self, *, database: str | None, schema: str | None, query: NodeResultQuery
    ) -> tuple[NodeResultEnvelope, ...]:
        if schema is None:
            return ()
        return read_node_results(
            connection=self.connection,
            execute=self.adapter.execute,
            relation_exists=self.adapter.relation_exists,
            database=database,
            schema=schema,
            query=query,
            render_qualified_name=self.adapter.render_qualified_name,
        )
