"""Tests for canonical immutable lookup and safe metadata projection."""

from __future__ import annotations

from dataclasses import FrozenInstanceError
from types import MappingProxyType
from typing import Any, cast

import pytest

from sqlbuild._native import (
    DeclarationIdentity,
    DeclarationKind,
    DeclarationRecord,
    OwnershipRoot,
    ResourceIdentity,
    ResourceKind,
    ResourceRecord,
    ScopeIndex,
    ScopeKind,
    ScopeLookup,
    build_scope_lookup,
    scope_metadata_projection,
)
from tests.unit.src.sqlbuild.compiler.scopes._test_types import (
    DeclarationContainerProjectionCase,
    ExpectedBooleanCase,
    ExpectedErrorCase,
    UnicodeKeyOrderCase,
)
from tests.unit.src.sqlbuild.compiler.scopes.helpers import scope_index


@pytest.mark.parametrize(
    "test_case",
    [ExpectedBooleanCase("canonical_order", True)],
    ids=lambda case: case.description,
)
def test_given_unsorted_index_when_building_lookup_then_records_and_edges_are_canonical(
    test_case: ExpectedBooleanCase,
) -> None:
    lookup: ScopeLookup = build_scope_lookup(index=scope_index())

    assert (
        [record.identity.name for record in lookup.index.declarations]
        == [
            "minimum_value",
            "order_status",
            "normalize_status",
        ]
    ) is test_case.expected_result
    model: ResourceIdentity = ResourceIdentity(kind=ResourceKind.MODEL, name="stg_orders")
    assert [usage.declaration.name for usage in lookup.usages_by_consumer[model]] == [
        "minimum_value",
        "order_status",
    ]


@pytest.mark.parametrize(
    "test_case",
    [
        UnicodeKeyOrderCase(
            description="non_ascii_keys_follow_python_repr",
            names=("caf\u00e9", "cafe", "\u00a0x", "\U0001f600", "Zed", "e\u0301"),
            expected_names=(
                "Zed",
                "\u00a0x",
                "cafe",
                "caf\u00e9",
                "e\u0301",
                "\U0001f600",
            ),
            expected_paths=(
                "sql/models/Zed.sql",
                "sql/models/\u00a0x.sql",
                "sql/models/cafe.sql",
                "sql/models/caf\u00e9.sql",
                "sql/models/e\u0301.sql",
                "sql/models/\U0001f600.sql",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_non_ascii_keys_when_building_lookup_then_mappings_follow_python_repr_order(
    test_case: UnicodeKeyOrderCase,
) -> None:
    root: OwnershipRoot = OwnershipRoot("models", resource_kind=ResourceKind.MODEL)
    resources: tuple[ResourceRecord, ...] = tuple(
        ResourceRecord(
            identity=ResourceIdentity(kind=ResourceKind.MODEL, name=name),
            path=f"sql/models/{name}.sql",
            ownership_root=root,
        )
        for name in test_case.names
    )

    lookup: ScopeLookup = build_scope_lookup(
        index=ScopeIndex(ownership_roots=(root,), resources=resources)
    )

    assert tuple(identity.name for identity in lookup.resources) == test_case.expected_names
    assert tuple(lookup.resources_by_path) == test_case.expected_paths
    assert tuple(sorted(lookup.resources, key=repr)) == tuple(lookup.resources)


@pytest.mark.parametrize(
    "test_case",
    [ExpectedErrorCase("mapping_proxy", TypeError)],
    ids=lambda case: case.description,
)
def test_given_scope_lookup_when_mutating_mapping_then_mapping_is_immutable(
    test_case: ExpectedErrorCase,
) -> None:
    lookup: ScopeLookup = build_scope_lookup(index=scope_index())

    assert isinstance(lookup.resources, MappingProxyType)
    with pytest.raises(test_case.expected_error):
        lookup.resources[ResourceIdentity(kind=ResourceKind.MODEL, name="other")] = next(  # ty: ignore[invalid-assignment]
            iter(lookup.resources.values())
        )


@pytest.mark.parametrize(
    "test_case",
    [ExpectedErrorCase("frozen_record", FrozenInstanceError)],
    ids=lambda case: case.description,
)
def test_given_frozen_scope_record_when_mutating_field_then_record_is_immutable(
    test_case: ExpectedErrorCase,
) -> None:
    record: DeclarationRecord = scope_index().declarations[0]

    with pytest.raises(test_case.expected_error):
        record.path = "changed.sql"


@pytest.mark.parametrize(
    "test_case",
    [ExpectedBooleanCase("byte_deterministic", True)],
    ids=lambda case: case.description,
)
def test_given_equivalent_indexes_when_serializing_then_json_is_byte_deterministic(
    test_case: ExpectedBooleanCase,
) -> None:
    index: ScopeIndex = scope_index()
    reversed_index: ScopeIndex = ScopeIndex(
        resources=tuple(reversed(index.resources)),
        declarations=tuple(reversed(index.declarations)),
        usages=tuple(reversed(index.usages)),
        completeness=index.completeness,
    )

    assert (
        scope_metadata_projection(index=index) == scope_metadata_projection(index=reversed_index)
    ) is test_case.expected_result


@pytest.mark.parametrize(
    "test_case",
    [ExpectedBooleanCase("value_safe_projection", False)],
    ids=lambda case: case.description,
)
def test_given_typed_constant_when_projecting_default_metadata_then_value_is_absent(
    test_case: ExpectedBooleanCase,
) -> None:
    projection: dict[str, Any] = cast(
        dict[str, Any], scope_metadata_projection(index=scope_index())
    )
    constant: dict[str, Any] = projection["declarations"][0]

    assert projection["schema_version"] == 3
    assert constant["role"] == "constants"
    assert constant["visibility"] == "descendant_public"
    assert constant["role_root"] == "sql/models/staging/constants"
    assert constant["bucket_path"] is None
    assert constant["metadata"]["constant"] == {
        "logical_type": "integer",
        "collection_kind": None,
        "item_count": None,
        "nullable": False,
        "render_as": "value_list",
    }
    assert projection["complete"] is test_case.expected_result
    assert projection["completeness"]["runtime_usage"] is test_case.expected_result


@pytest.mark.parametrize(
    "test_case",
    (
        DeclarationContainerProjectionCase(
            description="legacy declaration role has no navigation bucket",
            path="sql/models/orders/constants/policy.sql",
            owning_path="sql/models/orders",
            expected_role_root="sql/models/orders/constants",
            expected_bucket_path=None,
        ),
        DeclarationContainerProjectionCase(
            description="grouped declaration role has no navigation bucket",
            path="sql/models/orders/_sqlbuild/constants/policy.sql",
            owning_path="sql/models/orders",
            expected_role_root="sql/models/orders/_sqlbuild/constants",
            expected_bucket_path=None,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_declaration_path_when_projecting_then_role_container_is_canonical(
    test_case: DeclarationContainerProjectionCase,
) -> None:
    declaration: DeclarationRecord = DeclarationRecord(
        identity=DeclarationIdentity(DeclarationKind.CONSTANT, "policy"),
        path=test_case.path,
        line=1,
        column=1,
        scope=ScopeKind.INHERITED,
        ownership_root=OwnershipRoot("models", resource_kind=ResourceKind.MODEL),
        owning_path=test_case.owning_path,
    )

    projection: dict[str, Any] = cast(
        dict[str, Any], scope_metadata_projection(index=ScopeIndex(declarations=(declaration,)))
    )
    declaration_projection: dict[str, Any] = projection["declarations"][0]

    assert declaration_projection["role_root"] == test_case.expected_role_root
    assert declaration_projection["bucket_path"] == test_case.expected_bucket_path


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
