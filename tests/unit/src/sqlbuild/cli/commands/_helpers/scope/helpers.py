"""Scope output test builders."""

from __future__ import annotations

from dataclasses import replace

from sqlbuild._native import (
    OwnershipRoot,
    ResourceIdentity,
    ResourceKind,
    ResourceRecord,
    ScopeIndex,
    ScopeLookup,
    ScopeReport,
    ScopeReportFilters,
    build_scope_lookup,
    preview_scope_move,
    query_scope_report,
)
from tests.unit.src.sqlbuild.compiler.scopes.helpers import report_scope_lookup


def orders_move_report() -> ScopeReport:
    """Return the orders report with a move preview into sql/models/marts."""

    lookup: ScopeLookup = report_scope_lookup()
    report: ScopeReport = query_scope_report(
        lookup=lookup, target="model:orders", at=None, directory=False, filters=ScopeReportFilters()
    )
    move, diagnostics = preview_scope_move(
        lookup=lookup, resource="model:orders", destination="sql/models/marts/orders.sql"
    )
    return replace(report, move_preview=move, diagnostics=(*report.diagnostics, *diagnostics))


def bare_target_scope_lookup() -> ScopeLookup:
    """Return the report lookup plus a source, seed, function, test and macro-named model."""

    index: ScopeIndex = report_scope_lookup().index
    extra: tuple[ResourceRecord, ...] = (
        ResourceRecord(
            ResourceIdentity(ResourceKind.SOURCE, "raw__orders"),
            "sql/sources/raw.yml",
            OwnershipRoot("sources", resource_kind=ResourceKind.SOURCE),
        ),
        ResourceRecord(
            ResourceIdentity(ResourceKind.SEED, "waffle_types"),
            "sql/seeds/waffle_types.csv",
            OwnershipRoot("seeds", resource_kind=ResourceKind.SEED),
        ),
        ResourceRecord(
            ResourceIdentity(ResourceKind.FUNCTION, "udf__is_completed_order"),
            "sql/functions/sql/udf__is_completed_order.sql",
            OwnershipRoot("sql/functions/sql", resource_kind=ResourceKind.FUNCTION),
        ),
        ResourceRecord(
            ResourceIdentity(ResourceKind.TEST, "orders_are_unique"),
            "tests/unit/orders_are_unique.sql",
            OwnershipRoot("tests/unit", resource_kind=ResourceKind.TEST),
        ),
        ResourceRecord(
            ResourceIdentity(ResourceKind.MODEL, "normalize"),
            "sql/models/staging/normalize.sql",
            OwnershipRoot("models", resource_kind=ResourceKind.MODEL),
        ),
    )
    return build_scope_lookup(index=replace(index, resources=(*index.resources, *extra)))


def bare_target_scope_index(**_options: object) -> ScopeIndex:
    """Load the bare-target fixture index as `sqb scope` loads an offline index."""

    return bare_target_scope_lookup().index
