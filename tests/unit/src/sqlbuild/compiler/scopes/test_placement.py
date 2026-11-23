"""Behavior tests for complete declaration usage placement validation."""

from __future__ import annotations

import pytest

from sqlbuild._native import (
    DeclarationIdentity,
    DeclarationKind,
    DeclarationRecord,
    OwnershipRoot,
    ResourceIdentity,
    ResourceKind,
    ResourceRecord,
    ScopeCompleteness,
    ScopeDiagnostic,
    ScopeDiagnosticCode,
    ScopeIndex,
    ScopeKind,
    ScopeValidationError,
    UsageRecord,
    get_placement_validated_scope_index,
    validate_scope_index,
)
from sqlbuild._native import ScopeDiagnosticSeverity as DiagnosticSeverity
from tests.unit.src.sqlbuild.compiler.scopes._test_types import (
    DeclarationChainPlacementCase,
    DiamondLadderPlacementCase,
    ExpectedBooleanCase,
    PlacementEnforcementCase,
    PlacementValidationCase,
)
from tests.unit.src.sqlbuild.compiler.scopes.helpers import unused_declaration_index


@pytest.mark.parametrize(
    "test_case",
    (
        PlacementEnforcementCase("placement enforcement enabled", True, DiagnosticSeverity.ERROR),
        PlacementEnforcementCase(
            "placement enforcement disabled", False, DiagnosticSeverity.WARNING
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_placement_policy_when_validating_then_diagnostic_severity_matches_policy(
    test_case: PlacementEnforcementCase,
) -> None:
    result: ScopeIndex = unused_declaration_index(enforce_placement=test_case.enforce_placement)

    assert result.diagnostics[0].code is ScopeDiagnosticCode.UNUSED_DECLARATION
    assert result.diagnostics[0].severity is test_case.expected_severity


@pytest.mark.parametrize(
    "test_case",
    (ExpectedBooleanCase("test usage through scoped macro preserves production anchor", True),),
    ids=lambda case: case.description,
)
def test_given_test_usage_through_macro_when_validating_placement_then_test_path_is_not_anchor(
    test_case: ExpectedBooleanCase,
) -> None:
    macro_identity: DeclarationIdentity = DeclarationIdentity(DeclarationKind.MACRO, "order_policy")
    model_identity: ResourceIdentity = ResourceIdentity(ResourceKind.MODEL, "orders")
    test_identity: ResourceIdentity = ResourceIdentity(
        ResourceKind.TEST, "order_policy__returns_quantity"
    )
    ownership_root: OwnershipRoot = OwnershipRoot("models", resource_kind=ResourceKind.MODEL)
    declaration: DeclarationRecord = DeclarationRecord(
        identity=macro_identity,
        path="sql/models/orders/_sqlbuild/_macros/python/policy.py",
        line=1,
        column=1,
        scope=ScopeKind.LOCAL,
        ownership_root=ownership_root,
        owning_path="sql/models/orders",
    )
    index: ScopeIndex = ScopeIndex(
        resources=(
            ResourceRecord(model_identity, "sql/models/orders/orders.sql", ownership_root),
            ResourceRecord(
                test_identity,
                "tests/unit/macros/models/orders/test_order_policy__returns_quantity.sql",
                OwnershipRoot("tests/unit", resource_kind=ResourceKind.TEST),
            ),
        ),
        declarations=(declaration,),
        usages=(
            UsageRecord(model_identity, macro_identity),
            UsageRecord(test_identity, macro_identity, through=macro_identity),
        ),
        completeness=ScopeCompleteness(runtime_usage=True, placement=False),
    )

    result: ScopeIndex = get_placement_validated_scope_index(index=index)

    assert (not result.diagnostics) is test_case.expected_result


@pytest.mark.parametrize(
    "test_case",
    (PlacementEnforcementCase("advisory placement", False, DiagnosticSeverity.WARNING),),
    ids=lambda case: case.description,
)
def test_given_warning_placement_diagnostic_when_validating_then_validation_succeeds(
    test_case: PlacementEnforcementCase,
) -> None:
    index: ScopeIndex = unused_declaration_index(enforce_placement=test_case.enforce_placement)

    validate_scope_index(index=index)
    assert index.diagnostics[0].severity is test_case.expected_severity


@pytest.mark.parametrize(
    "test_case",
    (PlacementEnforcementCase("enforced placement", True, DiagnosticSeverity.ERROR),),
    ids=lambda case: case.description,
)
def test_given_error_placement_diagnostic_when_validating_then_validation_raises(
    test_case: PlacementEnforcementCase,
) -> None:
    index: ScopeIndex = unused_declaration_index(enforce_placement=test_case.enforce_placement)

    with pytest.raises(ScopeValidationError) as error:
        validate_scope_index(index=index)
    assert error.value.diagnostics[0].severity is test_case.expected_severity


@pytest.mark.parametrize(
    "test_case",
    (PlacementEnforcementCase("non-placement error", False, DiagnosticSeverity.ERROR),),
    ids=lambda case: case.description,
)
def test_given_non_placement_error_when_placement_enforcement_disabled_then_validation_raises(
    test_case: PlacementEnforcementCase,
) -> None:
    index: ScopeIndex = unused_declaration_index(enforce_placement=test_case.enforce_placement)
    non_placement_error: ScopeDiagnostic = ScopeDiagnostic(
        ScopeDiagnosticCode.DUPLICATE_DECLARATION, "duplicate"
    )

    with pytest.raises(ScopeValidationError) as error:
        validate_scope_index(
            index=ScopeIndex(diagnostics=(*index.diagnostics, non_placement_error))
        )
    assert error.value.diagnostics[0].severity is test_case.expected_severity


@pytest.mark.parametrize(
    "test_case",
    (
        PlacementValidationCase(
            "unused global is rejected",
            ScopeKind.GLOBAL,
            None,
            "sql/constants/limit.sql",
            "constants",
            None,
            (),
            (ScopeDiagnosticCode.UNUSED_DECLARATION,),
        ),
        PlacementValidationCase(
            "global used by one owner is over broad",
            ScopeKind.GLOBAL,
            None,
            "sql/constants/limit.sql",
            "constants",
            None,
            ("sql/models/domain/orders.sql",),
            (ScopeDiagnosticCode.OVER_BROAD_GLOBAL,),
            expected_message_fragment="sql/models/domain/_sqlbuild/_constants/",
        ),
        PlacementValidationCase(
            "global across authored root children is accepted",
            ScopeKind.GLOBAL,
            None,
            "sql/constants/limit.sql",
            "constants",
            None,
            ("sql/models/orders/orders.sql", "sql/models/products/products.sql"),
            (),
        ),
        PlacementValidationCase(
            "global used by authored root model recommends compatible legacy role",
            ScopeKind.GLOBAL,
            None,
            "sql/constants/limit.sql",
            "constants",
            None,
            ("sql/models/orders.sql",),
            (ScopeDiagnosticCode.OVER_BROAD_GLOBAL,),
            expected_message_fragment="sql/models/_constants/",
        ),
        PlacementValidationCase(
            "authored root inherited scope must be project global",
            ScopeKind.INHERITED,
            "models",
            "sql/models/constants/limit.sql",
            "models",
            ResourceKind.MODEL,
            ("sql/models/orders/orders.sql", "sql/models/products/products.sql"),
            (ScopeDiagnosticCode.REQUIRES_GLOBAL_PLACEMENT,),
            expected_message_fragment="top-level sql/constants/",
        ),
        PlacementValidationCase(
            "exact local is accepted",
            ScopeKind.LOCAL,
            "sql/models/domain",
            "sql/models/domain/_constants/limit.sql",
            "models",
            ResourceKind.MODEL,
            ("sql/models/domain/orders.sql",),
            (),
        ),
        PlacementValidationCase(
            "grouped exact local is accepted",
            ScopeKind.LOCAL,
            "sql/models/domain",
            "sql/models/domain/_sqlbuild/_constants/limit.sql",
            "models",
            ResourceKind.MODEL,
            ("sql/models/domain/orders.sql",),
            (),
        ),
        PlacementValidationCase(
            "local needed by descendant",
            ScopeKind.LOCAL,
            "sql/models/domain",
            "sql/models/domain/_constants/limit.sql",
            "models",
            ResourceKind.MODEL,
            ("sql/models/domain/child/orders.sql",),
            (ScopeDiagnosticCode.LOCAL_NEEDED_BY_DESCENDANT,),
        ),
        PlacementValidationCase(
            "inherited multi child lca is accepted",
            ScopeKind.INHERITED,
            "sql/models/domain",
            "sql/models/domain/constants/limit.sql",
            "models",
            ResourceKind.MODEL,
            ("sql/models/domain/a/orders.sql", "sql/models/domain/b/customers.sql"),
            (),
        ),
        PlacementValidationCase(
            "grouped inherited multi child lca is accepted",
            ScopeKind.INHERITED,
            "sql/models/domain",
            "sql/models/domain/_sqlbuild/constants/limit.sql",
            "models",
            ResourceKind.MODEL,
            ("sql/models/domain/a/orders.sql", "sql/models/domain/b/customers.sql"),
            (),
        ),
        PlacementValidationCase(
            "inherited at one consumer directory is over broad",
            ScopeKind.INHERITED,
            "sql/models/domain",
            "sql/models/domain/constants/limit.sql",
            "models",
            ResourceKind.MODEL,
            ("sql/models/domain/orders.sql",),
            (ScopeDiagnosticCode.OVER_BROAD_INHERITED,),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_complete_usage_when_validating_then_exact_placement_is_enforced(
    test_case: PlacementValidationCase,
) -> None:
    declaration_identity: DeclarationIdentity = DeclarationIdentity(
        DeclarationKind.CONSTANT, "limit"
    )
    declaration: DeclarationRecord = DeclarationRecord(
        identity=declaration_identity,
        path=test_case.declaration_path,
        line=1,
        column=1,
        scope=test_case.declaration_scope,
        ownership_root=OwnershipRoot(
            test_case.ownership_root, resource_kind=test_case.root_resource_kind
        ),
        owning_path=test_case.declaration_owner,
    )
    resources: tuple[ResourceRecord, ...] = tuple(
        ResourceRecord(
            identity=ResourceIdentity(ResourceKind.MODEL, f"model_{index}"),
            path=path,
            ownership_root=OwnershipRoot("models", resource_kind=ResourceKind.MODEL),
        )
        for index, path in enumerate(test_case.consumer_paths)
    )
    usages: tuple[UsageRecord, ...] = tuple(
        UsageRecord(resource.identity, declaration_identity) for resource in resources
    )

    result: ScopeIndex = get_placement_validated_scope_index(
        index=ScopeIndex(
            resources=resources,
            declarations=(declaration,),
            usages=usages,
            completeness=ScopeCompleteness(runtime_usage=True, placement=False),
        )
    )

    assert tuple(diagnostic.code for diagnostic in result.diagnostics) == test_case.expected_codes
    assert all(
        test_case.expected_message_fragment in diagnostic.message
        for diagnostic in result.diagnostics
    )
    assert result.completeness.runtime_usage
    assert result.completeness.placement


@pytest.mark.parametrize(
    "test_case",
    (
        DeclarationChainPlacementCase(
            description="diamond chain shares an intermediate consumer",
            declarations=(
                (
                    "base",
                    ScopeKind.LOCAL,
                    "sql/models/domain",
                    "sql/models/domain/_macros/python/base.py",
                ),
                (
                    "left",
                    ScopeKind.LOCAL,
                    "sql/models/domain",
                    "sql/models/domain/_macros/python/left.py",
                ),
                (
                    "right",
                    ScopeKind.LOCAL,
                    "sql/models/domain",
                    "sql/models/domain/_macros/python/right.py",
                ),
            ),
            declaration_usages=(("base", "left"), ("base", "right")),
            model_consumers=(("sql/models/domain/orders.sql", ("left", "right")),),
            expected_codes=(),
        ),
        DeclarationChainPlacementCase(
            description="cyclic chain still anchors to model consumers",
            declarations=(
                (
                    "alpha",
                    ScopeKind.LOCAL,
                    "sql/models/domain",
                    "sql/models/domain/_macros/python/alpha.py",
                ),
                (
                    "beta",
                    ScopeKind.LOCAL,
                    "sql/models/domain",
                    "sql/models/domain/_macros/python/beta.py",
                ),
            ),
            declaration_usages=(("alpha", "beta"), ("beta", "alpha")),
            model_consumers=(("sql/models/domain/orders.sql", ("beta",)),),
            expected_codes=(),
        ),
        DeclarationChainPlacementCase(
            description="over broad inherited is detected through the chain",
            declarations=(
                (
                    "base",
                    ScopeKind.INHERITED,
                    "sql/models/domain",
                    "sql/models/domain/macros/python/base.py",
                ),
                (
                    "mid",
                    ScopeKind.LOCAL,
                    "sql/models/domain",
                    "sql/models/domain/_macros/python/mid.py",
                ),
            ),
            declaration_usages=(("base", "mid"),),
            model_consumers=(("sql/models/domain/orders.sql", ("mid",)),),
            expected_codes=(ScopeDiagnosticCode.OVER_BROAD_INHERITED,),
        ),
        DeclarationChainPlacementCase(
            description="siblings reuse the shared consumer subtree",
            declarations=(
                (
                    "base_one",
                    ScopeKind.LOCAL,
                    "sql/models/domain",
                    "sql/models/domain/_macros/python/base_one.py",
                ),
                (
                    "base_two",
                    ScopeKind.LOCAL,
                    "sql/models/domain",
                    "sql/models/domain/_macros/python/base_two.py",
                ),
                (
                    "mid",
                    ScopeKind.LOCAL,
                    "sql/models/domain",
                    "sql/models/domain/_macros/python/mid.py",
                ),
            ),
            declaration_usages=(("base_one", "mid"), ("base_two", "mid")),
            model_consumers=(("sql/models/domain/orders.sql", ("mid",)),),
            expected_codes=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_declaration_consumption_chains_when_validating_then_anchors_resolve_transitively(
    test_case: DeclarationChainPlacementCase,
) -> None:
    identities: dict[str, DeclarationIdentity] = {
        name: DeclarationIdentity(DeclarationKind.MACRO, name)
        for name, _scope, _owning_path, _path in test_case.declarations
    }
    declarations: tuple[DeclarationRecord, ...] = tuple(
        DeclarationRecord(
            identity=identities[name],
            path=path,
            line=1,
            column=1,
            scope=scope,
            ownership_root=OwnershipRoot("models", resource_kind=ResourceKind.MODEL),
            owning_path=owning_path,
        )
        for name, scope, owning_path, path in test_case.declarations
    )
    resources: list[ResourceRecord] = []
    usages: list[UsageRecord] = []
    for model_path, declaration_names in test_case.model_consumers:
        model_identity: ResourceIdentity = ResourceIdentity(
            ResourceKind.MODEL, f"model_{len(resources)}"
        )
        resources.append(
            ResourceRecord(
                identity=model_identity,
                path=model_path,
                ownership_root=OwnershipRoot("models", resource_kind=ResourceKind.MODEL),
            )
        )
        for declaration_name in declaration_names:
            usages.append(UsageRecord(model_identity, identities[declaration_name]))
    for declaration_name, consumer_name in test_case.declaration_usages:
        usages.append(UsageRecord(identities[consumer_name], identities[declaration_name]))

    result: ScopeIndex = get_placement_validated_scope_index(
        index=ScopeIndex(
            resources=tuple(resources),
            declarations=declarations,
            usages=tuple(usages),
            completeness=ScopeCompleteness(runtime_usage=True, placement=False),
        )
    )

    assert tuple(diagnostic.code for diagnostic in result.diagnostics) == test_case.expected_codes
    assert result.completeness.placement


@pytest.mark.parametrize(
    "test_case",
    (
        DiamondLadderPlacementCase(
            description="forty layer ladder", layers=40, width=2, expected_codes=()
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_deep_diamond_ladder_when_validating_then_resolution_completes(
    test_case: DiamondLadderPlacementCase,
) -> None:
    identities: dict[str, DeclarationIdentity] = {}
    for layer in range(test_case.layers):
        for position in range(test_case.width):
            name: str = f"d_{layer}_{position}"
            identities[name] = DeclarationIdentity(DeclarationKind.MACRO, name)
    declarations: tuple[DeclarationRecord, ...] = tuple(
        DeclarationRecord(
            identity=identity,
            path=f"sql/models/domain/_macros/{name}.py",
            line=1,
            column=1,
            scope=ScopeKind.LOCAL,
            ownership_root=OwnershipRoot("models", resource_kind=ResourceKind.MODEL),
            owning_path="sql/models/domain",
        )
        for name, identity in identities.items()
    )
    model_identity: ResourceIdentity = ResourceIdentity(ResourceKind.MODEL, "orders")
    usages: list[UsageRecord] = []
    for layer in range(test_case.layers - 1):
        for position in range(test_case.width):
            for consumer_position in range(test_case.width):
                usages.append(
                    UsageRecord(
                        identities[f"d_{layer + 1}_{consumer_position}"],
                        identities[f"d_{layer}_{position}"],
                    )
                )
    for position in range(test_case.width):
        usages.append(
            UsageRecord(model_identity, identities[f"d_{test_case.layers - 1}_{position}"])
        )

    result: ScopeIndex = get_placement_validated_scope_index(
        index=ScopeIndex(
            resources=(
                ResourceRecord(
                    identity=model_identity,
                    path="sql/models/domain/orders.sql",
                    ownership_root=OwnershipRoot("models", resource_kind=ResourceKind.MODEL),
                ),
            ),
            declarations=declarations,
            usages=tuple(usages),
            completeness=ScopeCompleteness(runtime_usage=True, placement=False),
        )
    )

    assert tuple(diagnostic.code for diagnostic in result.diagnostics) == test_case.expected_codes
    assert result.completeness.placement


@pytest.mark.parametrize(
    "test_case",
    (
        PlacementValidationCase(
            "relationship",
            ScopeKind.LOCAL,
            "sql/models/domain",
            "sql/models/domain/_enums/state.sql",
            "models",
            ResourceKind.MODEL,
            (),
            (),
            (ScopeDiagnosticCode.REQUIRES_GLOBAL_PLACEMENT,),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_relationship_and_cross_root_usages_when_validating_then_through_models_anchor_placement(
    test_case: PlacementValidationCase,
) -> None:
    declaration_identity: DeclarationIdentity = DeclarationIdentity(DeclarationKind.ENUM, "state")
    test_identity: ResourceIdentity = ResourceIdentity(ResourceKind.TEST, "check")
    model_identity: ResourceIdentity = ResourceIdentity(ResourceKind.MODEL, "orders")
    declaration: DeclarationRecord = DeclarationRecord(
        declaration_identity,
        test_case.declaration_path,
        1,
        1,
        ScopeKind.LOCAL,
        OwnershipRoot("models", resource_kind=ResourceKind.MODEL),
        owning_path="sql/models/domain",
    )
    resources: tuple[ResourceRecord, ...] = (
        ResourceRecord(
            test_identity,
            "tests/unit/check.sql",
            OwnershipRoot("tests/unit", resource_kind=ResourceKind.TEST),
        ),
        ResourceRecord(
            model_identity,
            "sql/models/domain/orders.sql",
            OwnershipRoot("models", resource_kind=ResourceKind.MODEL),
        ),
    )

    relationship_only: ScopeIndex = get_placement_validated_scope_index(
        index=ScopeIndex(
            resources=resources,
            declarations=(declaration,),
            usages=(UsageRecord(test_identity, declaration_identity, through=model_identity),),
        )
    )
    direct_and_relationship: ScopeIndex = get_placement_validated_scope_index(
        index=ScopeIndex(
            resources=resources,
            declarations=(declaration,),
            usages=(
                UsageRecord(test_identity, declaration_identity, through=model_identity),
                UsageRecord(test_identity, declaration_identity),
            ),
        )
    )

    assert relationship_only.diagnostics == ()
    assert (
        tuple(item.code for item in direct_and_relationship.diagnostics)
        == test_case.expected_direct_codes
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
