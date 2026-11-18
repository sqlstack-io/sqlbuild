"""Adapter implementations for the runs table every build and seed appends to."""

from __future__ import annotations

from typing import Any, cast


class RunsTableMixin:
    """Provide runs-table DDL and the warehouse identity a run records."""

    def render_current_warehouse_user(self) -> str | None:
        """Render the session's warehouse identity as a SQL expression; None without one."""

        return None

    def render_create_runs_table_sql(self, *, database: str | None, schema: str) -> str:
        """Render DDL that creates the runs table when it is missing."""

        from sqlbuild._native import build_runs_create_table_sql

        adapter: Any = cast(Any, self)
        return build_runs_create_table_sql(
            database=database,
            schema=schema,
            render_qualified_name=adapter.render_qualified_name,
            render_framework_type=adapter.render_framework_type,
        )
