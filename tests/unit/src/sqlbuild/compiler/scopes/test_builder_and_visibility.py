"""Tests for canonical scope index construction and static visibility."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    ConstantDeclaration,
    DeclarationIdentity,
    DeclarationKind,
    DeclarationRecord,
    DiscoveredAuditBlock,
    DiscoveredAuditFile,
    DiscoveredConstantFile,
    DiscoveredEnumFile,
    DiscoveredMacroFile,
    DiscoveredSourceFile,
    DiscoveredSqlFunctionFile,
    DiscoveredSqlHookFile,
    DiscoveredSqlScenarioFile,
    DiscoveredSqlTestBlock,
    DiscoveredSqlTestFile,
    EnumDeclaration,
    EnumMember,
    InaccessibleReason,
    LoadedMacro,
    OwnershipRoot,
    ResourceIdentity,
    ResourceKind,
    ResourceRecord,
    ScopeDiagnosticCode,
    ScopeIndex,
    ScopeKind,
    ScopeLookup,
    ScopeTargetQuery,
    ScopeValidationError,
    SourceEntry,
    VisibilityReason,
    VisibilityResolution,
    build_scope_index,
    build_scope_lookup,
    normalize_sql_value,
    query_scope_target,
    resolve_scope_visibility,
    validate_scope_index,
)
from tests.unit.src.sqlbuild.compiler.scopes._test_types import (
    ExpectedBooleanCase,
    ResourceExpectation,
    VisibilityExpectation,
)
from tests.unit.src.sqlbuild.compiler.scopes.helpers import (
    declaration_record,
    discovered_inputs,
    discovered_model,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ResourceExpectation("model", "model", "orders", "sql/models/orders.sql", "sql/models"),
        ResourceExpectation("test", "test", "orders_test", "tests/unit/orders.sql", "tests/unit"),
        ResourceExpectation(
            "scenario", "scenario", "checkout", "tests/scenarios/checkout.sql", "tests/scenarios"
        ),
        ResourceExpectation(
            "function", "function", "tax", "sql/functions/sql/tax.sql", "sql/functions/sql"
        ),
        ResourceExpectation("source", "source", "raw_orders", "sql/sources/raw.yml", "sql/sources"),
    ],
    ids=lambda case: case.description,
)
def test_given_every_authored_resource_kind_when_building_then_uses_canonical_root(
    test_case: ResourceExpectation,
) -> None:
    test_path: Path = Path("tests/unit/orders.sql")
    audit_path: Path = Path("sql/audits/fresh.sql")
    index: ScopeIndex = build_scope_index(
        discovered_inputs=discovered_inputs(
            model_files=(discovered_model(name="orders", path="sql/models/orders.sql"),),
            test_files=(
                DiscoveredSqlTestFile(
                    test_path,
                    test_path,
                    "TEST(); SELECT 1",
                    (DiscoveredSqlTestBlock(1, {}, "SELECT 1", "orders_test"),),
                ),
            ),
            scenario_files=(
                DiscoveredSqlScenarioFile(
                    Path("tests/scenarios/checkout.sql"),
                    Path("tests/scenarios/checkout.sql"),
                    "",
                    {},
                    "SELECT 1",
                    "checkout",
                ),
            ),
            sql_hook_files=(
                DiscoveredSqlHookFile(
                    Path("sql/hooks/sql/grant.sql"),
                    Path("sql/hooks/sql/grant.sql"),
                    "",
                    {},
                    "SELECT 1",
                    "grant",
                ),
            ),
            sql_function_files=(
                DiscoveredSqlFunctionFile(
                    Path("sql/functions/sql/tax.sql"),
                    Path("sql/functions/sql/tax.sql"),
                    "",
                    {"name": "tax"},
                    "SELECT 1",
                ),
            ),
            audit_files=(
                DiscoveredAuditFile(
                    audit_path,
                    audit_path,
                    "",
                    (DiscoveredAuditBlock(1, {}, "SELECT 1", "fresh"),),
                ),
            ),
            source_files=(
                DiscoveredSourceFile(
                    Path("sql/sources/raw.yml"),
                    Path("sql/sources/raw.yml"),
                    "",
                    (SourceEntry(name="raw_orders"),),
                ),
            ),
        )
    )

    lookup: ScopeLookup = build_scope_lookup(index=index)
    identity: ResourceIdentity = ResourceIdentity(
        ResourceKind(test_case.expected_kind), test_case.expected_name
    )
    record: ResourceRecord = lookup.resources[identity][0]
    assert (record.path, record.ownership_root.path) == (
        test_case.expected_path,
        test_case.expected_root,
    )
    assert record.ownership_root.resource_kind is identity.kind


@pytest.mark.parametrize(
    "test_case",
    [
        VisibilityExpectation("global", "global", VisibilityReason.GLOBAL),
        VisibilityExpectation("root_ancestor", "root", VisibilityReason.INHERITED_ANCESTOR),
        VisibilityExpectation("near_ancestor", "near", VisibilityReason.INHERITED_ANCESTOR),
        VisibilityExpectation(
            "noncanonical_ancestor", "noncanonical", VisibilityReason.INHERITED_ANCESTOR
        ),
        VisibilityExpectation("local", "local", VisibilityReason.LOCAL_OWNER),
        VisibilityExpectation("private", "_private", VisibilityReason.PRIVATE_OWNER),
    ],
    ids=lambda case: case.description,
)
def test_given_all_scope_tiers_when_resolving_then_classifies_without_shadow_reset(
    test_case: VisibilityExpectation,
) -> None:
    target: ResourceIdentity = ResourceIdentity(ResourceKind.MODEL, "orders")
    other: ResourceIdentity = ResourceIdentity(ResourceKind.MODEL, "other")
    resource: ResourceRecord = ResourceRecord(
        target,
        "sql/models/sales/daily/orders.sql",
        OwnershipRoot("sql/models", resource_kind=ResourceKind.MODEL),
    )
    declarations: tuple[DeclarationRecord, ...] = (
        declaration_record(name="global", scope=ScopeKind.GLOBAL, owner_path=None),
        declaration_record(name="root", scope=ScopeKind.INHERITED, owner_path="sql/models"),
        declaration_record(name="near", scope=ScopeKind.INHERITED, owner_path="sql/models/sales"),
        declaration_record(
            name="noncanonical",
            scope=ScopeKind.INHERITED,
            owner_path=r"sql\models\.\sales\daily\..",
        ),
        declaration_record(
            name="local", scope=ScopeKind.LOCAL, owner_path="sql/models/sales/daily"
        ),
        declaration_record(
            name="_private",
            scope=ScopeKind.PRIVATE,
            owner_path="sql/models/sales/daily",
            owner=target,
        ),
        declaration_record(
            name="parent_local", scope=ScopeKind.LOCAL, owner_path="sql/models/sales"
        ),
        declaration_record(
            name="sibling", scope=ScopeKind.INHERITED, owner_path="sql/models/finance"
        ),
        declaration_record(
            name="descendant", scope=ScopeKind.INHERITED, owner_path="sql/models/sales/daily/child"
        ),
        declaration_record(name="prefix", scope=ScopeKind.INHERITED, owner_path="sql/models/sale"),
        declaration_record(
            name="_other", scope=ScopeKind.PRIVATE, owner_path="sql/models/sales/daily", owner=other
        ),
    )
    resolution: VisibilityResolution = resolve_scope_visibility(
        lookup=build_scope_lookup(
            index=ScopeIndex(resources=(resource,), declarations=declarations)
        ),
        target="sql\\models\\sales\\daily\\orders.sql",
    )
    visible: dict[str, VisibilityReason] = {
        item.declaration.name: item.reason for item in resolution.visible
    }
    assert visible[test_case.expected_name] is test_case.expected_visible_reason


@pytest.mark.parametrize(
    "test_case",
    [
        VisibilityExpectation(
            "local_parent_boundary",
            "parent_local",
            expected_inaccessible_reason=InaccessibleReason.LOCAL_BOUNDARY,
        ),
        VisibilityExpectation(
            "sibling",
            "sibling",
            expected_inaccessible_reason=InaccessibleReason.SIBLING_SCOPE,
        ),
        VisibilityExpectation(
            "descendant",
            "descendant",
            expected_inaccessible_reason=InaccessibleReason.DESCENDANT_SCOPE,
        ),
        VisibilityExpectation(
            "textual_prefix",
            "prefix",
            expected_inaccessible_reason=InaccessibleReason.SIBLING_SCOPE,
        ),
        VisibilityExpectation(
            "other_private",
            "_other",
            expected_inaccessible_reason=InaccessibleReason.PRIVATE_OWNER,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_out_of_scope_declarations_when_resolving_then_returns_specific_reason(
    test_case: VisibilityExpectation,
) -> None:
    target: ResourceIdentity = ResourceIdentity(ResourceKind.MODEL, "orders")
    other: ResourceIdentity = ResourceIdentity(ResourceKind.MODEL, "other")
    resource: ResourceRecord = ResourceRecord(
        target,
        "sql/models/sales/daily/orders.sql",
        OwnershipRoot("sql/models", resource_kind=ResourceKind.MODEL),
    )
    declarations: tuple[DeclarationRecord, ...] = (
        declaration_record(
            name="parent_local", scope=ScopeKind.LOCAL, owner_path="sql/models/sales"
        ),
        declaration_record(
            name="sibling", scope=ScopeKind.INHERITED, owner_path="sql/models/finance"
        ),
        declaration_record(
            name="descendant",
            scope=ScopeKind.INHERITED,
            owner_path="sql/models/sales/daily/child",
        ),
        declaration_record(name="prefix", scope=ScopeKind.INHERITED, owner_path="sql/models/sale"),
        declaration_record(
            name="_other",
            scope=ScopeKind.PRIVATE,
            owner_path="sql/models/sales/daily",
            owner=other,
        ),
    )
    resolution: VisibilityResolution = resolve_scope_visibility(
        lookup=build_scope_lookup(
            index=ScopeIndex(resources=(resource,), declarations=declarations)
        ),
        target=target,
    )
    inaccessible: dict[str, InaccessibleReason] = {
        item.declaration.name: item.reason for item in resolution.inaccessible
    }
    assert inaccessible[test_case.expected_name] is test_case.expected_inaccessible_reason


@pytest.mark.parametrize(
    "test_case",
    [ExpectedBooleanCase("unknown_target", True)],
    ids=lambda case: case.description,
)
def test_given_unknown_qualified_target_when_querying_then_distinguishes_unknown_from_inaccessible(
    test_case: ExpectedBooleanCase,
) -> None:
    lookup: ScopeLookup = build_scope_lookup(index=ScopeIndex())

    query: ScopeTargetQuery = query_scope_target(lookup=lookup, target="model:missing")
    resolution: VisibilityResolution = resolve_scope_visibility(
        lookup=lookup, target="model:missing"
    )

    assert query.unknown is test_case.expected_result
    assert resolution.target.unknown
    assert resolution.inaccessible == ()


@pytest.mark.parametrize(
    "test_case",
    [ExpectedBooleanCase("declaration_target", False)],
    ids=lambda case: case.description,
)
def test_given_declaration_qualified_target_when_querying_then_returns_declaration_records(
    test_case: ExpectedBooleanCase,
) -> None:
    declaration: DeclarationRecord = declaration_record(
        name="minimum_value",
        scope=ScopeKind.GLOBAL,
        owner_path=None,
        kind=DeclarationKind.CONSTANT,
    )
    lookup: ScopeLookup = build_scope_lookup(index=ScopeIndex(declarations=(declaration,)))

    query: ScopeTargetQuery = query_scope_target(
        lookup=lookup,
        target="constant:minimum_value",
    )

    assert query.unknown is test_case.expected_result
    assert query.matches == ()
    assert query.declaration_matches == (declaration,)


@pytest.mark.parametrize(
    "test_case",
    [ExpectedBooleanCase("duplicates_retained", True)],
    ids=lambda case: case.description,
)
def test_given_public_duplicates_across_scopes_when_building_then_retains_all_and_aggregates(
    test_case: ExpectedBooleanCase,
) -> None:
    declaration: EnumDeclaration = EnumDeclaration(
        "status", (EnumMember("OPEN", "OPEN"),), "VARCHAR", Path("sql/enums/status.sql")
    )
    files: tuple[DiscoveredEnumFile, ...] = (
        DiscoveredEnumFile(
            Path("sql/enums/status.sql"),
            Path("sql/enums/status.sql"),
            "",
            (declaration,),
        ),
        DiscoveredEnumFile(
            Path("sql/models/enums/status.sql"),
            Path("sql/models/enums/status.sql"),
            "",
            (declaration,),
            ScopeKind.INHERITED,
            Path("sql/models"),
            Path("sql/models"),
        ),
        DiscoveredEnumFile(
            Path("sql/models/sales/_enums/status.sql"),
            Path("sql/models/sales/_enums/status.sql"),
            "",
            (declaration,),
            ScopeKind.LOCAL,
            Path("sql/models"),
            Path("sql/models/sales"),
        ),
    )
    index: ScopeIndex = build_scope_index(discovered_inputs=discovered_inputs(enum_files=files))
    lookup: ScopeLookup = build_scope_lookup(index=index)

    assert (len(index.declarations) == 3) is test_case.expected_result
    assert len(lookup.declarations[DeclarationIdentity(DeclarationKind.ENUM, "status")]) == 3
    assert [item.code for item in index.diagnostics] == [
        ScopeDiagnosticCode.DUPLICATE_DECLARATION,
        ScopeDiagnosticCode.DUPLICATE_DECLARATION,
        ScopeDiagnosticCode.DUPLICATE_DECLARATION,
    ]
    with pytest.raises(ScopeValidationError) as error:
        validate_scope_index(index=index)
    assert len(error.value.diagnostics) == 3


@pytest.mark.parametrize(
    "test_case",
    [ExpectedBooleanCase("duplicate_resources", True)],
    ids=lambda case: case.description,
)
def test_given_duplicate_resource_identity_when_building_then_lookup_retains_every_record(
    test_case: ExpectedBooleanCase,
) -> None:
    index: ScopeIndex = build_scope_index(
        discovered_inputs=discovered_inputs(
            model_files=(
                discovered_model(name="orders", path="sql/models/a.sql"),
                discovered_model(name="orders", path="sql/models/b.sql"),
            )
        )
    )
    lookup: ScopeLookup = build_scope_lookup(index=index)
    identity: ResourceIdentity = ResourceIdentity(ResourceKind.MODEL, "orders")

    assert (len(lookup.resources[identity]) == 2) is test_case.expected_result
    assert [item.code for item in index.diagnostics] == [
        ScopeDiagnosticCode.DUPLICATE_RESOURCE,
        ScopeDiagnosticCode.DUPLICATE_RESOURCE,
    ]


@pytest.mark.parametrize(
    "test_case",
    [ExpectedBooleanCase("private_owner_names", True)],
    ids=lambda case: case.description,
)
def test_given_cross_kind_and_same_private_name_across_owners_when_building_then_all_are_valid(
    test_case: ExpectedBooleanCase,
) -> None:
    private: EnumDeclaration = EnumDeclaration(
        "_status", (EnumMember("OPEN", "OPEN"),), "VARCHAR", Path("sql/models/a.sql"), "a"
    )
    enum_file: DiscoveredEnumFile = DiscoveredEnumFile(
        Path("sql/enums/status.sql"),
        Path("sql/enums/status.sql"),
        "",
        (
            EnumDeclaration(
                "status", (EnumMember("OPEN", "OPEN"),), "VARCHAR", Path("sql/enums/status.sql")
            ),
        ),
    )
    constant_file: DiscoveredConstantFile = DiscoveredConstantFile(
        Path("sql/constants/status.sql"),
        Path("sql/constants/status.sql"),
        "",
        (
            ConstantDeclaration(
                "status",
                normalize_sql_value(raw_value=1, context="test"),
                Path("sql/constants/status.sql"),
            ),
        ),
    )
    index: ScopeIndex = build_scope_index(
        discovered_inputs=discovered_inputs(
            model_files=(
                discovered_model(name="a", path="sql/models/a.sql", enums=(private,)),
                discovered_model(name="b", path="sql/models/b.sql", enums=(private,)),
            ),
            enum_files=(enum_file,),
            constant_files=(constant_file,),
        )
    )

    lookup: ScopeLookup = build_scope_lookup(index=index)
    owner_a: ResourceIdentity = ResourceIdentity(ResourceKind.MODEL, "a")
    owner_b: ResourceIdentity = ResourceIdentity(ResourceKind.MODEL, "b")
    private_a: DeclarationIdentity = DeclarationIdentity(DeclarationKind.ENUM, "_status", owner_a)
    private_b: DeclarationIdentity = DeclarationIdentity(DeclarationKind.ENUM, "_status", owner_b)
    constant_identity: DeclarationIdentity = DeclarationIdentity(DeclarationKind.CONSTANT, "status")
    constant: DeclarationRecord = lookup.declarations[constant_identity][0]
    assert (index.diagnostics == ()) is test_case.expected_result
    assert len(lookup.declarations[private_a]) == 1
    assert len(lookup.declarations[private_b]) == 1
    assert constant.constant is not None
    assert constant.constant.logical_type == "integer"
    assert not hasattr(constant.constant, "value")


@pytest.mark.parametrize(
    "test_case",
    [ExpectedBooleanCase("reserved_private_name", True)],
    ids=lambda case: case.description,
)
def test_given_reserved_private_name_when_building_then_emits_stable_name_diagnostic(
    test_case: ExpectedBooleanCase,
) -> None:
    private: EnumDeclaration = EnumDeclaration(
        "__status", (EnumMember("OPEN", "OPEN"),), "VARCHAR", Path("sql/models/a.sql"), "a"
    )

    index: ScopeIndex = build_scope_index(
        discovered_inputs=discovered_inputs(
            model_files=(discovered_model(name="a", path="sql/models/a.sql", enums=(private,)),)
        )
    )

    assert (
        [item.code for item in index.diagnostics] == [ScopeDiagnosticCode.RESERVED_DECLARATION_NAME]
    ) is test_case.expected_result


@pytest.mark.parametrize(
    "test_case",
    [ExpectedBooleanCase("safe_macro_metadata", True)],
    ids=lambda case: case.description,
)
def test_given_loaded_scoped_macro_when_building_then_metadata_is_safe_and_normalized(
    test_case: ExpectedBooleanCase,
) -> None:
    def normalize_status(value: object, fallback: str = "unknown") -> str:
        return str(value or fallback)

    path: Path = Path("sql/models/sales/macros/python/status.py")
    source = "def normalize_status(value, fallback='unknown'):\n    return str(value or fallback)\n"
    index: ScopeIndex = build_scope_index(
        discovered_inputs=discovered_inputs(
            macro_files=(
                DiscoveredMacroFile(
                    path,
                    path,
                    source,
                    ScopeKind.INHERITED,
                    Path("sql/models"),
                    Path("sql/models/sales"),
                    Path("sql/models/sales/macros"),
                ),
            )
        ),
        loaded_macros={
            "normalize_status": LoadedMacro(
                "normalize_status", path, path, source, normalize_status
            )
        },
    )

    record: DeclarationRecord = index.declarations[0]
    assert (record.scope is ScopeKind.INHERITED) is test_case.expected_result
    assert record.owning_path == "sql/models/sales"
    assert record.ownership_root.resource_kind is ResourceKind.MODEL
    assert record.macro is not None
    assert record.macro.parameters == ("value", "fallback")
    assert len(record.macro.source_digest) == 64
    assert not hasattr(record.macro, "raw_source")
    assert index.completeness.static_visibility
    assert not index.completeness.runtime_usage


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
