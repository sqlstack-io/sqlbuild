"""Tests for the pure compiler-owned scope query and report layer."""

from __future__ import annotations

import json
from dataclasses import replace
from time import perf_counter
from typing import cast

import pytest

from sqlbuild._native import (
    DeclarationKind,
    OwnershipRoot,
    ResourceKind,
    ResourceRecord,
    ScopeBrowseResult,
    ScopeDiagnosticCode,
    ScopeIndex,
    ScopeListResult,
    ScopeLookup,
    ScopeReport,
    ScopeReportFilters,
    browse_scope_folders,
    build_scope_lookup,
    explain_scope_declaration,
    list_scope_declarations,
    preview_scope_move,
    query_scope_report,
    serialize_scope_report,
)
from tests.unit.src.sqlbuild.compiler.scopes._test_types import (
    ExpectedBooleanCase,
    ScopeReportTargetCase,
)
from tests.unit.src.sqlbuild.compiler.scopes.helpers import report_scope_lookup


@pytest.mark.parametrize(
    "test_case",
    (
        ScopeReportTargetCase("qualified", "model:orders", "model:orders", ()),
        ScopeReportTargetCase("exact_path", "sql/models/staging/orders.sql", "model:orders", ()),
        ScopeReportTargetCase("windows_path", r"sql\models\staging\orders.sql", "model:orders", ()),
        ScopeReportTargetCase(
            "bare", "orders", None, (ScopeDiagnosticCode.UNQUALIFIED_TARGET.value,)
        ),
        ScopeReportTargetCase(
            "unknown", "model:missing", None, (ScopeDiagnosticCode.UNKNOWN_TARGET.value,)
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_target_when_querying_report_then_resolution_is_kind_qualified(
    test_case: ScopeReportTargetCase,
) -> None:
    report: ScopeReport = query_scope_report(lookup=report_scope_lookup(), target=test_case.target)

    assert report.resource.identity == test_case.expected_identity
    assert tuple(item.code.value for item in report.diagnostics) == test_case.expected_codes


@pytest.mark.parametrize(
    "test_case", (ExpectedBooleanCase("default", True),), ids=lambda case: case.description
)
def test_given_resource_when_querying_then_sections_and_scope_chain_are_separate(
    test_case: ExpectedBooleanCase,
) -> None:
    report: ScopeReport = query_scope_report(
        lookup=report_scope_lookup(),
        target="model:orders",
        filters=ScopeReportFilters(include_nearby=True, globals="all"),
    )

    assert [item.identity for item in report.used] == [
        "enum:mart_status",
        "enum:order_status",
        "macro:normalize",
    ]
    assert [item.identity for item in report.relationship_scope] == ["enum:mart_status"]
    assert [item.path for item in report.scope_chain] == [
        "sql/models/staging",
        "sql/models",
        "global",
    ]
    assert [item.identity for item in report.nearby_unavailable] == ["enum:other_status"]
    assert test_case.expected_result


@pytest.mark.parametrize(
    "test_case", (ExpectedBooleanCase("default", True),), ids=lambda case: case.description
)
def test_given_prospective_paths_when_querying_then_visibility_is_pure_and_partial(
    test_case: ExpectedBooleanCase,
) -> None:
    file_report: ScopeReport = query_scope_report(
        lookup=report_scope_lookup(), at=r"sql\models\staging\new.sql"
    )
    directory_report: ScopeReport = query_scope_report(
        lookup=report_scope_lookup(), at="sql/models/staging", directory=True
    )
    invalid_report: ScopeReport = query_scope_report(
        lookup=report_scope_lookup(), at="../outside.sql"
    )
    invalid_suffix: ScopeReport = query_scope_report(
        lookup=report_scope_lookup(), at="sql/models/staging/new.txt"
    )

    assert "enum:order_status" in {item.identity for item in file_report.available}
    assert directory_report.resource.directory
    assert not file_report.complete
    assert ScopeDiagnosticCode.INCOMPLETE_USAGE in {item.code for item in file_report.diagnostics}
    assert invalid_report.diagnostics[-1].code is ScopeDiagnosticCode.INVALID_PROSPECTIVE_PATH
    assert invalid_suffix.diagnostics[-1].code is ScopeDiagnosticCode.INVALID_PROSPECTIVE_PATH
    assert test_case.expected_result


@pytest.mark.parametrize(
    "test_case", (ExpectedBooleanCase("empty_root", True),), ids=lambda case: case.description
)
def test_given_empty_authored_root_when_querying_prospective_path_then_root_facts_resolve_it(
    test_case: ExpectedBooleanCase,
) -> None:
    lookup: ScopeLookup = build_scope_lookup(
        index=ScopeIndex(
            ownership_roots=(OwnershipRoot("sql/models", resource_kind=ResourceKind.MODEL),)
        )
    )

    report: ScopeReport = query_scope_report(lookup=lookup, at="sql/models/new.sql")

    assert report.resource.path == "sql/models/new.sql"
    assert report.resource.prospective is test_case.expected_result
    assert report.diagnostics[-1].code is ScopeDiagnosticCode.INCOMPLETE_USAGE


@pytest.mark.parametrize(
    "test_case", (ExpectedBooleanCase("default", True),), ids=lambda case: case.description
)
def test_given_combined_filters_when_querying_then_order_is_deterministic(
    test_case: ExpectedBooleanCase,
) -> None:
    report: ScopeReport = query_scope_report(
        lookup=report_scope_lookup(),
        target="model:orders",
        filters=ScopeReportFilters(
            kinds=(DeclarationKind.MACRO, DeclarationKind.ENUM),
            glob="*norm*",
            used_only=True,
            globals="all",
        ),
    )

    assert [item.identity for item in report.available] == ["macro:normalize"]
    assert [item.identity for item in report.used] == ["macro:normalize"]
    assert test_case.expected_result


@pytest.mark.parametrize(
    "test_case", (ExpectedBooleanCase("default", True),), ids=lambda case: case.description
)
def test_given_dependency_depth_when_querying_used_then_dependencies_expand_exactly(
    test_case: ExpectedBooleanCase,
) -> None:
    without_dependencies: ScopeReport = query_scope_report(
        lookup=report_scope_lookup(),
        target="model:orders",
        filters=ScopeReportFilters(glob="normalize", dependency_depth=0, globals="all"),
    )
    with_dependencies: ScopeReport = query_scope_report(
        lookup=report_scope_lookup(),
        target="model:orders",
        filters=ScopeReportFilters(glob="normalize", dependency_depth=1, globals="all"),
    )

    assert [item.identity for item in without_dependencies.used] == ["macro:normalize"]
    assert [item.identity for item in with_dependencies.used] == [
        "enum:order_status",
        "macro:normalize",
    ]
    assert test_case.expected_result


@pytest.mark.parametrize(
    "test_case", (ExpectedBooleanCase("default", True),), ids=lambda case: case.description
)
def test_given_cursor_when_global_is_inserted_before_it_then_continuation_is_stable(
    test_case: ExpectedBooleanCase,
) -> None:
    first: ScopeReport = query_scope_report(
        lookup=report_scope_lookup(extra_globals=4),
        target="model:orders",
        filters=ScopeReportFilters(page_size=2, globals="all"),
    )
    cursor: str | None = first.sections[0].next_cursor
    second: ScopeReport = query_scope_report(
        lookup=report_scope_lookup(extra_globals=5),
        target="model:orders",
        filters=ScopeReportFilters(page_size=2, cursor=cursor, globals="all"),
    )

    assert cursor is not None
    assert all(item.identity > cursor for item in second.available)
    invalid: ScopeReport = query_scope_report(
        lookup=report_scope_lookup(),
        target="model:orders",
        filters=ScopeReportFilters(cursor="not_unique_or_known", globals="all"),
    )
    assert ScopeDiagnosticCode.INVALID_CURSOR in {item.code for item in invalid.diagnostics}
    assert test_case.expected_result


@pytest.mark.parametrize(
    "test_case", (ExpectedBooleanCase("default", True),), ids=lambda case: case.description
)
@pytest.mark.performance
def test_given_folder_tree_when_browsing_and_listing_then_counts_are_exact(
    test_case: ExpectedBooleanCase,
) -> None:
    started: float = perf_counter()
    lookup: ScopeLookup = report_scope_lookup(extra_globals=10_000)
    root: ScopeBrowseResult = browse_scope_folders(lookup=lookup)
    constants: ScopeBrowseResult = browse_scope_folders(lookup=lookup, folder="global/constants")
    listed: ScopeListResult = list_scope_declarations(
        lookup=lookup,
        folder="global/constants/generated",
        filters=ScopeReportFilters(page_size=10_001),
    )

    assert [item.path for item in root.folders] == ["global", "sql"]
    assert constants.folders[0].descendant_count == 10_000
    assert listed.section.total == 10_000
    assert len(listed.declarations) == 10_000
    assert perf_counter() - started < 5.0
    assert test_case.expected_result


@pytest.mark.parametrize(
    "test_case",
    (ExpectedBooleanCase("target_usage", True),),
    ids=lambda case: case.description,
)
def test_given_target_when_browsing_and_listing_used_declarations_then_counts_are_target_scoped(
    test_case: ExpectedBooleanCase,
) -> None:
    lookup: ScopeLookup = report_scope_lookup()
    browse: ScopeBrowseResult = browse_scope_folders(
        lookup=lookup,
        folder="sql/models/marts",
        target="model:orders",
    )
    listed: ScopeListResult = list_scope_declarations(
        lookup=lookup,
        folder="sql/models/marts",
        target="model:orders",
        filters=ScopeReportFilters(used_only=True),
    )

    assert browse.folders[0].used_count == 1
    assert [item.identity for item in listed.declarations] == ["enum:mart_status"]
    assert listed.section.complete is test_case.expected_result


@pytest.mark.parametrize(
    "test_case", (ExpectedBooleanCase("default", True),), ids=lambda case: case.description
)
def test_given_declaration_when_explaining_then_metadata_is_typed_and_value_safe(
    test_case: ExpectedBooleanCase,
) -> None:
    explanation, diagnostics = explain_scope_declaration(
        lookup=report_scope_lookup(),
        declaration="constant:warehouse_password",
        target="model:orders",
    )

    assert diagnostics == ()
    assert explanation.declaration is not None
    assert dict(explanation.declaration.metadata) == {
        "logical_type": "string",
        "collection_kind": None,
        "item_count": None,
        "nullable": False,
        "render_as": None,
        "role": "constants",
        "visibility_label": "project",
        "role_root": "sql/constants",
        "bucket_path": None,
    }
    assert test_case.expected_result


@pytest.mark.parametrize(
    "test_case", (ExpectedBooleanCase("bounded_enum", True),), ids=lambda case: case.description
)
def test_given_large_enum_when_explaining_then_member_preview_is_bounded(
    test_case: ExpectedBooleanCase,
) -> None:
    explanation, diagnostics = explain_scope_declaration(
        lookup=report_scope_lookup(),
        declaration="enum:order_status",
        target="model:orders",
    )

    assert diagnostics == ()
    assert explanation.declaration is not None
    metadata: dict[str, object] = dict(explanation.declaration.metadata)
    assert len(cast(tuple[str, ...], metadata["members"])) == 20
    assert metadata["member_count"] == 25
    assert metadata["members_truncated"] is test_case.expected_result


@pytest.mark.parametrize(
    "test_case", (ExpectedBooleanCase("default", True),), ids=lambda case: case.description
)
def test_given_resource_move_when_previewing_then_deltas_and_grants_are_separate(
    test_case: ExpectedBooleanCase,
) -> None:
    preview, diagnostics = preview_scope_move(
        lookup=report_scope_lookup(),
        resource="model:orders",
        destination="sql/models/marts/orders.sql",
    )
    invalid, invalid_diagnostics = preview_scope_move(
        lookup=report_scope_lookup(),
        resource="model:orders",
        destination="outside/orders.sql",
    )
    occupied, occupied_diagnostics = preview_scope_move(
        lookup=report_scope_lookup(),
        resource="model:orders",
        destination="sql/models/marts/expected_orders.sql",
    )
    base_lookup: ScopeLookup = report_scope_lookup()
    source: ResourceRecord = base_lookup.index.resources[0]
    duplicate_path: str = "sql/models/marts/duplicate_orders.sql"
    duplicate_lookup: ScopeLookup = build_scope_lookup(
        index=replace(
            base_lookup.index,
            resources=(*base_lookup.index.resources, replace(source, path=duplicate_path)),
        )
    )
    duplicate, duplicate_diagnostics = preview_scope_move(
        lookup=duplicate_lookup,
        resource="sql/models/staging/orders.sql",
        destination=duplicate_path,
    )

    assert diagnostics == ()
    assert preview is not None
    assert [item.identity for item in preview.lost] == ["enum:order_status"]
    assert [item.identity for item in preview.gained] == ["enum:mart_status"]
    assert preview.invalidated_usages == ("enum:order_status",)
    assert [item.identity for item in preview.relationship_retained] == ["enum:mart_status"]
    assert invalid is None
    assert invalid_diagnostics[0].code is ScopeDiagnosticCode.INVALID_PROSPECTIVE_PATH
    assert occupied is None
    assert occupied_diagnostics[0].code is ScopeDiagnosticCode.INVALID_PROSPECTIVE_PATH
    assert duplicate is None
    assert duplicate_diagnostics[0].code is ScopeDiagnosticCode.INVALID_PROSPECTIVE_PATH
    assert test_case.expected_result


@pytest.mark.parametrize(
    "test_case", (ExpectedBooleanCase("default", True),), ids=lambda case: case.description
)
def test_given_report_when_serializing_then_bytes_are_deterministic_safe_and_canonical(
    test_case: ExpectedBooleanCase,
) -> None:
    report: ScopeReport = query_scope_report(
        lookup=report_scope_lookup(),
        target="model:orders",
        filters=ScopeReportFilters(globals="all", include_nearby=True),
    )

    first: str = serialize_scope_report(report=report)
    second: str = serialize_scope_report(report=report)
    payload: dict[str, object] = json.loads(first)

    assert first == second
    assert first.endswith("\n")
    assert first.isascii()
    assert payload["schema_version"] == 2
    sections: list[dict[str, object]] = cast(list[dict[str, object]], payload["sections"])
    assert isinstance(sections[0]["collapsed"], bool)
    assert isinstance(sections[0]["collapsed_count"], int)
    assert "secret-source-digest" not in first
    assert test_case.expected_result


@pytest.mark.parametrize(
    "test_case", (ExpectedBooleanCase("default", True),), ids=lambda case: case.description
)
def test_given_partial_index_when_querying_then_section_and_aggregate_completeness_differ(
    test_case: ExpectedBooleanCase,
) -> None:
    lookup: ScopeLookup = report_scope_lookup()
    partial: ScopeLookup = build_scope_lookup(
        index=replace(
            lookup.index,
            completeness=replace(lookup.index.completeness, runtime_usage=False),
        )
    )
    report: ScopeReport = query_scope_report(lookup=partial, target="model:orders")

    assert not report.sections[1].complete
    assert report.sections[0].complete
    assert not report.complete
    assert test_case.expected_result


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
