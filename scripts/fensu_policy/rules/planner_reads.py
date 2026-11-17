"""Rule keeping planner phases on `WarehouseReads` and `Dialect`, away from the adapter."""

from __future__ import annotations

from fensu import AnalyzerId, Family, Fault, Project, RuleContext, rule

from scripts.fensu_policy._helpers.path_checks import allowed_prefix
from scripts.fensu_policy._helpers.unused_state import (
    is_rust_test_path,
    reasoned_entries,
    warehouse_access_lines,
)
from scripts.fensu_policy.constants import (
    PLANNER_READ_PATHS,
    POLICY_CONFIG_FILE,
)


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
    reasons: dict[str, str]
    malformed: list[str]
    reasons, malformed = reasoned_entries(entries=ctx.option(PLANNER_READ_PATHS))
    prefixes: list[str] = list(reasons)
    covered: set[str] = set()
    faults: list[Fault] = []
    for facts in ctx.rust.files:
        path: str = facts.file.path.value
        if facts.test or is_rust_test_path(path=path):
            continue
        prefix: str | None = allowed_prefix(path=path, prefixes=prefixes)
        if prefix is None:
            continue
        covered.add(prefix)
        faults.extend(
            ctx.fault_for(path=ctx.repo_root / path, line=line, column=0)
            for line in warehouse_access_lines(source=facts.source)
        )
    problems: list[str] = [
        f"planner_paths entry {entry!r} needs `path=reason`" for entry in malformed
    ] + [
        f"planner path {prefix} matches no Rust file; remove the entry"
        for prefix in prefixes
        if prefix not in covered
    ]
    return [
        ctx.fault_for(path=ctx.repo_root / POLICY_CONFIG_FILE, line=1, column=0, message=problem)
        for problem in problems
    ] + faults
