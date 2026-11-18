"""Rule keeping planner phases on `WarehouseReads` and `Dialect`, away from the adapter."""

from __future__ import annotations

from fensu import AnalyzerId, Family, Fault, Project, RuleContext, rule

from scripts.fensu_policy._helpers.unused_state import (
    path_entry_faults,
    warehouse_access_lines,
)
from scripts.fensu_policy.constants import PLANNER_READ_PATHS


@rule(
    code="XSB080",
    family=Family.CUSTOM,
    slug="planner-warehouse-reads",
    message="planner phases read the warehouse only through WarehouseReads",
    remediation=(
        "Write SQL through `Dialect` and read warehouse state through `WarehouseReads`; add the "
        "read to `WarehouseReads`, with its live and synced answers, instead of reaching the "
        "adapter or a connection."
    ),
    cacheable=False,
    analyzers=(AnalyzerId.RUST,),
    options=(PLANNER_READ_PATHS,),
)
def planner_warehouse_reads(*, project: Project, ctx: RuleContext) -> list[Fault]:
    del project
    return path_entry_faults(
        ctx=ctx,
        entries=(PLANNER_READ_PATHS, "planner_paths", True),
        lines=warehouse_access_lines,
        stale=lambda prefix: f"planner path {prefix} matches no Rust file; remove the entry",
    )
