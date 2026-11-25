"""Execution graph concurrency sizing, and whether a build's SQL phase runs anything."""

from __future__ import annotations

from sqlbuild._native import PlanOutput


def runnable_graph_width(*, plan: PlanOutput) -> int:
    """Return a safe worker cap that cannot underestimate asynchronous overlap."""

    executable_node_count: int = len(plan.execution_order)
    microbatch_width: int = max(
        (entry.batch_concurrency for entry in plan.model_entries),
        default=1,
    )
    return max(1, executable_node_count, microbatch_width)


def sql_phase_has_work(*, plan: PlanOutput) -> bool:
    """Whether the SQL phase runs a node, audit, test, migration, retention or table change."""

    return any(
        (
            plan.execution_order,
            plan.audit_entries,
            plan.test_entries,
            plan.migration_entries,
            plan.column_migration_entries,
            plan.retention_entries,
            plan.old_name_view_entries,
            plan.table_type_entries,
        )
    )


def sql_phase_concurrency(*, plan: PlanOutput, max_concurrency: int) -> int:
    """The SQL phase's worker count: the requested limit, at most the plan's runnable width."""

    return min(max(1, max_concurrency), runnable_graph_width(plan=plan))
