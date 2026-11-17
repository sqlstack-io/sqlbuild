from __future__ import annotations

import pytest

from sqlbuild._native import (
    AuditPlanEntry,
    CompiledObjectKey,
    CompiledRelationLocation,
    CompileSqlReference,
    Dialect,
    SourceEntry,
    SqlReferenceKind,
    plan_audit,
)
from sqlbuild.compiler.auditing.types import AuditAttachmentKind, AuditRunScope
from sqlbuild.compiler.compile.types import AttachedAuditTargetKind, CompiledAuditView
from tests.unit.src.sqlbuild.compiler.planner._helpers._test_types import (
    PlanAttachedAuditTestCase,
    PlanAuditTestCase,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.helpers import (
    PlannerTestAdapter,
    build_audit_from_test_case,
    build_audit_model_locations,
    build_audit_source_map,
    build_scheduling_audit,
    build_scheduling_graph,
)


@pytest.mark.parametrize(
    "test_case",
    [
        PlanAttachedAuditTestCase(
            description="safe attached audit retains model lifecycle scheduling",
            attached_target_name="orders",
            referenced_model_names=("orders", "stg_orders"),
            upstream_edges={"orders": ("stg_orders",), "stg_orders": ()},
            expected_attachment_kind=AuditAttachmentKind.MODEL,
            expected_reads_outside_target_lineage=False,
        ),
        PlanAttachedAuditTestCase(
            description="attached audit reading outside its lineage still gates its model",
            attached_target_name="stg_orders",
            referenced_model_names=("stg_orders", "orders"),
            upstream_edges={"orders": ("stg_orders",), "stg_orders": ()},
            expected_attachment_kind=AuditAttachmentKind.MODEL,
            expected_reads_outside_target_lineage=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_logically_attached_audit_when_planning_then_scheduling_and_identity_are_independent(
    test_case: PlanAttachedAuditTestCase,
) -> None:
    references: tuple[CompileSqlReference, ...] = tuple(
        CompileSqlReference(ref_kind=SqlReferenceKind.REF, ref_name=name)
        for name in test_case.referenced_model_names
    )
    audit: CompiledAuditView = build_scheduling_audit(
        references=references,
        attached_target_kind=AttachedAuditTargetKind.MODEL,
        attached_target_name=test_case.attached_target_name,
    )
    upstream: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]
    downstream: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]
    upstream, downstream = build_scheduling_graph(test_case.upstream_edges)

    result: AuditPlanEntry = plan_audit(
        audit=audit,
        model_locations=build_audit_model_locations(
            {name: f"analytics.{name}" for name in test_case.referenced_model_names}
        ),
        seed_locations={},
        source_map={},
        dialect=Dialect(PlannerTestAdapter()),
        lineage_upstream_deps=upstream,
        lineage_downstream_deps=downstream,
        model_materializations={test_case.attached_target_name: "incremental"},
    )

    assert result.attachment_kind == test_case.expected_attachment_kind
    assert result.attached_target_kind == AttachedAuditTargetKind.MODEL
    assert result.attached_target_name == test_case.attached_target_name
    assert result.effective_run_scope == AuditRunScope.FINAL
    assert result.reads_outside_target_lineage is test_case.expected_reads_outside_target_lineage


@pytest.mark.parametrize(
    "test_case",
    [
        PlanAuditTestCase(
            description="resolves ref to qualified model name",
            sql_body=('SELECT id FROM __ref("orders") WHERE id IS NULL'),
            model_locations={"orders": "staging.orders"},
            source_map_entries={},
            expected_sql_fragment=("SELECT id FROM staging.orders WHERE id IS NULL"),
        ),
        PlanAuditTestCase(
            description="resolves source to qualified name",
            sql_body=('SELECT id FROM __source("raw_orders") WHERE id IS NULL'),
            model_locations={},
            source_map_entries={
                "raw_orders": (None, "public", "orders"),
            },
            expected_sql_fragment=("SELECT id FROM public.orders WHERE id IS NULL"),
        ),
        PlanAuditTestCase(
            description="resolves both ref and source in same query",
            sql_body=(
                'SELECT a.id FROM __ref("orders") a JOIN __source("raw_orders") b ON a.id = b.id'
            ),
            model_locations={"orders": "staging.orders"},
            source_map_entries={
                "raw_orders": (None, "public", "orders"),
            },
            expected_sql_fragment=(
                "SELECT a.id FROM staging.orders a JOIN public.orders b ON a.id = b.id"
            ),
        ),
        PlanAuditTestCase(
            description=("source with database includes database in qualified name"),
            sql_body='SELECT id FROM __source("raw_orders")',
            model_locations={},
            source_map_entries={
                "raw_orders": ("raw_db", "public", "orders"),
            },
            expected_sql_fragment=("SELECT id FROM raw_db.public.orders"),
        ),
        PlanAuditTestCase(
            description="propagates always_run to audit plan entry",
            sql_body=('SELECT id FROM __ref("orders") WHERE id IS NULL'),
            model_locations={"orders": "staging.orders"},
            source_map_entries={},
            always_run=True,
            expected_sql_fragment=("SELECT id FROM staging.orders WHERE id IS NULL"),
            expected_always_run=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_audit_when_planning_then_resolves_sql(
    test_case: PlanAuditTestCase,
) -> None:
    audit: CompiledAuditView = build_audit_from_test_case(test_case)
    model_locations: dict[str, CompiledRelationLocation] = build_audit_model_locations(
        test_case.model_locations
    )
    source_map: dict[str, SourceEntry] = build_audit_source_map(test_case.source_map_entries)

    result: AuditPlanEntry = plan_audit(
        audit=audit,
        model_locations=model_locations,
        seed_locations={},
        source_map=source_map,
        dialect=Dialect(PlannerTestAdapter()),
        lineage_upstream_deps={},
        lineage_downstream_deps={},
        model_materializations={},
    )

    assert test_case.expected_sql_fragment in result.resolved_sql
    assert result.always_run is test_case.expected_always_run


@pytest.mark.parametrize(
    "test_case",
    [
        PlanAuditTestCase(
            description="unknown ref raises clear error",
            sql_body=('SELECT id FROM __ref("missing") WHERE id IS NULL'),
            model_locations={},
            source_map_entries={},
            expected_sql_fragment="",
            expected_error_fragment=r"still contains unresolved __ref\(\) markers",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_audit_with_unresolved_marker_when_planning_then_it_raises_clear_error(
    test_case: PlanAuditTestCase,
) -> None:
    audit: CompiledAuditView = build_audit_from_test_case(test_case)
    model_locations: dict[str, CompiledRelationLocation] = build_audit_model_locations(
        test_case.model_locations
    )
    source_map: dict[str, SourceEntry] = build_audit_source_map(test_case.source_map_entries)

    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        plan_audit(
            audit=audit,
            model_locations=model_locations,
            seed_locations={},
            source_map=source_map,
            dialect=Dialect(PlannerTestAdapter()),
            lineage_upstream_deps={},
            lineage_downstream_deps={},
            model_materializations={},
        )
