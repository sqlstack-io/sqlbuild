"""Adapter implementations for the display-only resource costs table of cost-aware adapters."""

from __future__ import annotations

from typing import Any, cast


class ResourceCostsMixin:
    """Provide resource-costs-table DDL and pruning for adapters that collect run cost."""

    def render_create_resource_costs_table_sql(self, *, database: str | None, schema: str) -> str:
        """Render DDL that creates the resource costs table when it is missing."""

        from sqlbuild._native import build_resource_costs_create_table_sql

        adapter: Any = cast(Any, self)
        return build_resource_costs_create_table_sql(
            database=database,
            schema=schema,
            render_qualified_name=adapter.render_qualified_name,
            render_framework_type=adapter.render_framework_type,
        )

    def render_prune_resource_costs_history_sql(
        self, *, database: str | None, schema: str, retain_versions: int
    ) -> str:
        """Render SQL deleting cost observations beyond the newest `retain_versions` per key."""

        from sqlbuild._native import build_resource_costs_prune_sql

        adapter: Any = cast(Any, self)
        return build_resource_costs_prune_sql(
            database=database,
            schema=schema,
            render_qualified_name=adapter.render_qualified_name,
            retain_versions=retain_versions,
        )
