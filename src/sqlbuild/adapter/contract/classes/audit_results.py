"""Default adapter pruning for the audit results state table."""

from __future__ import annotations

from typing import Any, cast


class AuditResultsMixin:
    """Provide portable audit results pruning for first-class warehouse adapters."""

    def render_prune_audit_results_history_sql(
        self, *, database: str | None, schema: str, retain_versions: int
    ) -> str:
        """Render SQL deleting audit results beyond the newest `retain_versions` per binding."""

        from sqlbuild._native import build_audit_results_prune_sql

        adapter: Any = cast(Any, self)
        return build_audit_results_prune_sql(
            database=database,
            schema=schema,
            render_qualified_name=adapter.render_qualified_name,
            retain_versions=retain_versions,
        )
