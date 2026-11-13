"""Rules keeping build observations and the plan preview's synced copy out of live decisions."""

from __future__ import annotations

from fensu import AnalyzerId, Family, Fault, Project, RuleContext, rule

from scripts.fensu_policy._helpers.unused_state import (
    confined_references,
    observation_reference_lines,
    synced_copy_reference_lines,
)
from scripts.fensu_policy.constants import OBSERVATION_ALLOWED_PATHS, SYNCED_COPY_ALLOWED_PATHS


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
    return confined_references(
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
    return confined_references(
        ctx=ctx,
        option=SYNCED_COPY_ALLOWED_PATHS,
        references=synced_copy_reference_lines,
        subject="the synced copy",
    )
