"""Final model audit execution against the staged relation."""

from __future__ import annotations

from typing import Any

from sqlbuild._native import AuditPlanEntry, CompiledRelationLocation, Fingerprint, SourceEntry
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.compiler.auditing.types import AuditRunScope
from sqlbuild.executor.auditing.main._execute import execute_audit
from sqlbuild.executor.auditing.models import AuditExecutionResult
from sqlbuild.executor.run._helpers.reuse.audit import (
    audit_plan_binding_key,
    reused_final_audit_results_by_binding_key,
)
from sqlbuild.executor.run.models import FinalAuditRun, ModelMaterializationContext


def run_final_model_audits(
    *,
    relation_overrides: dict[str, str] | None,
    model_audits: tuple[AuditPlanEntry, ...],
    reuse_origin_fingerprint: Fingerprint | None,
    adapter: BaseAdapter,
    connection: Any,
    model_locations: dict[str, CompiledRelationLocation],
    seed_locations: dict[str, CompiledRelationLocation],
    source_map: dict[str, SourceEntry],
) -> FinalAuditRun:
    """Run final audits, reusing proven results when possible."""

    reused_audit_results_by_binding_key: dict[str, AuditExecutionResult] = (
        reused_final_audit_results_by_binding_key(
            metadata_json=reuse_origin_fingerprint.metadata_json,
            model_audits=model_audits,
        )
        if reuse_origin_fingerprint is not None
        else {}
    )
    results: list[AuditExecutionResult] = []
    has_error: bool = False
    audit: AuditPlanEntry
    for audit in model_audits:
        reused_result: AuditExecutionResult | None = reused_audit_results_by_binding_key.get(
            audit_plan_binding_key(audit)
        )
        if reused_result is not None:
            results.append(reused_result)
            continue
        result: AuditExecutionResult = execute_audit(
            audit=audit,
            adapter=adapter,
            connection=connection,
            model_locations=model_locations,
            seed_locations=seed_locations,
            source_map=source_map,
            relation_overrides=relation_overrides,
            run_scope_phase=AuditRunScope.FINAL,
            quality_scope="final",
        )
        results.append(result)
        if result.blocks:
            has_error = True
    return FinalAuditRun(results=tuple(results), has_error=has_error)


def run_final_scope_audits(
    *, context: ModelMaterializationContext, relation_override: str | None = None
) -> FinalAuditRun:
    """Run all model audits at FINAL scope against promoted relations."""

    return run_final_model_audits(
        relation_overrides=(
            None if relation_override is None else {context.entry.name: relation_override}
        ),
        model_audits=context.model_audits,
        reuse_origin_fingerprint=None,
        adapter=context.adapter,
        connection=context.connection,
        model_locations=context.model_locations,
        seed_locations=context.seed_locations,
        source_map=context.source_map,
    )


def run_delta_scope_audits(
    *, context: ModelMaterializationContext, delta_qualified: str
) -> FinalAuditRun:
    """Run DELTA_AND_FINAL scoped audits against the delta relation."""

    results: list[AuditExecutionResult] = []
    has_error: bool = False
    audit: AuditPlanEntry
    for audit in context.model_audits:
        if audit.effective_run_scope != AuditRunScope.DELTA_AND_FINAL:
            continue
        result: AuditExecutionResult = execute_audit(
            audit=audit,
            adapter=context.adapter,
            connection=context.connection,
            model_locations=context.model_locations,
            seed_locations=context.seed_locations,
            source_map=context.source_map,
            relation_overrides={context.entry.name: delta_qualified},
            run_scope_phase=AuditRunScope.DELTA_AND_FINAL,
            quality_scope="delta",
        )
        results.append(result)
        if result.blocks:
            has_error = True
    return FinalAuditRun(results=tuple(results), has_error=has_error)
