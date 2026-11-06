"""Default adapter implementations for the consumption-record state table."""

from __future__ import annotations

from typing import Any, cast


class ConsumptionMixin:
    """Provide portable consumption-table DDL and pruning for first-class warehouse adapters."""

    def render_create_consumption_table_sql(self, *, database: str | None, schema: str) -> str:
        """Render DDL that creates the consumption table when it is missing."""

        from sqlbuild._native import build_consumption_create_table_sql

        adapter: Any = cast(Any, self)
        return build_consumption_create_table_sql(
            database=database,
            schema=schema,
            render_qualified_name=adapter.render_qualified_name,
            render_framework_type=adapter.render_framework_type,
        )

    def render_create_consumption_index_sqls(
        self, *, database: str | None, schema: str
    ) -> tuple[str, ...]:
        """Return no index DDL unless the warehouse adapter explicitly opts in."""

        del database, schema
        return ()

    def render_prune_consumption_history_sql(
        self, *, database: str | None, schema: str, retain_versions: int
    ) -> str:
        """Render SQL deleting whole executions beyond the newest `retain_versions` per relation."""

        from sqlbuild._native import build_consumption_prune_sql

        adapter: Any = cast(Any, self)
        return build_consumption_prune_sql(
            database=database,
            schema=schema,
            render_qualified_name=adapter.render_qualified_name,
            retain_versions=retain_versions,
        )
