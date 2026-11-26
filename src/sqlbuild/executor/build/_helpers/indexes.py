"""Precomputed index construction for build execution."""

from __future__ import annotations

from sqlbuild._native import AuditPlanEntry, PlanOutput
from sqlbuild.compiler.auditing.types import AuditAttachmentKind
from sqlbuild.executor.build.models import BuildIndexes


def build_execution_indexes(plan: PlanOutput) -> BuildIndexes:
    """Build lookup indexes from a plan; a named audit on an unbuilt target runs at the end."""

    source_audits: dict[str, list[AuditPlanEntry]] = {}
    seed_audits: dict[str, list[AuditPlanEntry]] = {}
    model_audits: dict[str, list[AuditPlanEntry]] = {}
    end_audits: list[AuditPlanEntry] = []
    by_name: frozenset[str] = frozenset(plan.selected_audit_names)
    built_models: frozenset[str] = frozenset(entry.name for entry in plan.model_entries)
    built_seeds: frozenset[str] = frozenset(entry.name for entry in plan.seed_entries)

    audit: AuditPlanEntry
    for audit in plan.audit_entries:
        target: str = audit.attached_target_name or ""
        if audit.attachment_kind == AuditAttachmentKind.SOURCE:
            source_audits.setdefault(target, []).append(audit)
        elif audit.attachment_kind == AuditAttachmentKind.SEED and (
            target in built_seeds or audit.name not in by_name
        ):
            seed_audits.setdefault(target, []).append(audit)
        elif audit.attachment_kind == AuditAttachmentKind.MODEL and (
            target in built_models or audit.name not in by_name
        ):
            model_audits.setdefault(target, []).append(audit)
        else:
            end_audits.append(audit)

    return BuildIndexes(
        model_entries_by_key={entry.key: entry for entry in plan.model_entries},
        seed_entries_by_key={entry.key: entry for entry in plan.seed_entries},
        function_entries_by_key={entry.key: entry for entry in plan.function_entries},
        source_load_entries_by_key={entry.key: entry for entry in plan.source_load_entries},
        test_entries_by_key={entry.key: entry for entry in plan.test_entries},
        source_audits_by_source={k: tuple(v) for k, v in source_audits.items()},
        seed_audits_by_seed={k: tuple(v) for k, v in seed_audits.items()},
        model_audits_by_model={k: tuple(v) for k, v in model_audits.items()},
        end_audits=tuple(end_audits),
    )
