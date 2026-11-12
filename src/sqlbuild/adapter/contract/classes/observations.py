"""Default adapter implementations for the display-only observations state table."""

from __future__ import annotations

from typing import Any, cast


class ObservationsMixin:
    """Provide portable observations-table DDL and pruning for first-class warehouse adapters."""

    def render_create_observations_table_sql(self, *, database: str | None, schema: str) -> str:
        """Render DDL that creates the observations table when it is missing."""

        from sqlbuild._native import build_observations_create_table_sql

        adapter: Any = cast(Any, self)
        return build_observations_create_table_sql(
            database=database,
            schema=schema,
            render_qualified_name=adapter.render_qualified_name,
            render_framework_type=adapter.render_framework_type,
        )

    def render_prune_observations_history_sql(
        self, *, database: str | None, schema: str, retain_versions: int
    ) -> str:
        """Render SQL deleting observations beyond the newest `retain_versions` per node."""

        from sqlbuild._native import build_observations_prune_sql

        adapter: Any = cast(Any, self)
        return build_observations_prune_sql(
            database=database,
            schema=schema,
            render_qualified_name=adapter.render_qualified_name,
            retain_versions=retain_versions,
        )
