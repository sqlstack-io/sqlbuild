"""Rules keeping build observations and the plan preview's synced copy out of live decisions."""

from __future__ import annotations

from collections.abc import Callable

from fensu import AnalyzerId, Family, Fault, Project, RuleContext, RuleOption, rule

from scripts.fensu_policy._helpers.path_checks import allowed_prefix
from scripts.fensu_policy._helpers.unused_state import (
    is_rust_test_path,
    observation_reference_lines,
    reasoned_entries,
    synced_copy_reference_lines,
)
from scripts.fensu_policy.constants import (
    OBSERVATION_ALLOWED_PATHS,
    POLICY_CONFIG_FILE,
    SYNCED_COPY_ALLOWED_PATHS,
)


@rule(
    code="XSB078",
    family=Family.CUSTOM,
    slug="observation-display-only",
    message="build observations are display-only and must not reach planning or build decisions",
    remediation=(
        "Read the live warehouse fact instead; only the files in allowed_paths, which write, "
        "prune, mirror or display observations, may name observation state."
    ),
    cacheable=False,
    analyzers=(AnalyzerId.RUST,),
    options=(OBSERVATION_ALLOWED_PATHS,),
)
def observation_display_only(*, project: Project, ctx: RuleContext) -> list[Fault]:
    del project
    return _confined(
        ctx=ctx,
        option=OBSERVATION_ALLOWED_PATHS,
        references=observation_reference_lines,
        subject="observation state",
    )


@rule(
    code="XSB079",
    family=Family.CUSTOM,
    slug="synced-copy-preview-only",
    message="only sqb plan previews may plan from the synced copy",
    remediation=(
        "Plan from the live adapter; only the files in allowed_paths, which define, activate or "
        "branch on the plan preview's synced copy, may name it."
    ),
    cacheable=False,
    analyzers=(AnalyzerId.RUST,),
    options=(SYNCED_COPY_ALLOWED_PATHS,),
)
def synced_copy_preview_only(*, project: Project, ctx: RuleContext) -> list[Fault]:
    del project
    return _confined(
        ctx=ctx,
        option=SYNCED_COPY_ALLOWED_PATHS,
        references=synced_copy_reference_lines,
        subject="the synced copy",
    )


def _confined(
    *,
    ctx: RuleContext,
    option: RuleOption[tuple[str, ...]],
    references: Callable[..., list[int]],
    subject: str,
) -> list[Fault]:
    """Faults for each reference outside the allowed paths, and for stale allowed paths."""

    reasons: dict[str, str]
    malformed: list[str]
    reasons, malformed = reasoned_entries(entries=ctx.option(option))
    prefixes: list[str] = list(reasons)
    used: set[str] = set()
    faults: list[Fault] = []
    for facts in ctx.rust.files:
        path: str = facts.file.path.value
        if facts.test or is_rust_test_path(path=path):
            continue
        lines: list[int] = references(source=facts.source)
        if not lines:
            continue
        prefix: str | None = allowed_prefix(path=path, prefixes=prefixes)
        if prefix is not None:
            used.add(prefix)
            continue
        faults.extend(
            ctx.fault_for(path=ctx.repo_root / path, line=line, column=0) for line in lines
        )
    problems: list[str] = [
        f"allowed_paths entry {entry!r} needs `path=reason`" for entry in malformed
    ] + [
        f"allowed path {prefix} no longer names {subject}; remove the entry"
        for prefix in prefixes
        if prefix not in used
    ]
    return [
        ctx.fault_for(path=ctx.repo_root / POLICY_CONFIG_FILE, line=1, column=0, message=problem)
        for problem in problems
    ] + faults
