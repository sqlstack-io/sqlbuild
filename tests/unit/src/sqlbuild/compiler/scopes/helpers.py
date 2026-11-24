"""Fixtures for compiler scope unit tests."""

from __future__ import annotations

import json
from collections.abc import Callable
from pathlib import Path
from typing import cast

from sqlbuild._native import (
    ConstantMetadata,
    DeclarationIdentity,
    DeclarationKind,
    DeclarationRecord,
    DiscoveredProjectInputs,
    DiscoveredSqlModelFile,
    EnumDeclaration,
    EnumMemberMetadata,
    EnumMetadata,
    GrantRecord,
    LocalConfig,
    MacroMetadata,
    OwnershipRoot,
    OwnershipRootKind,
    ProjectConfig,
    ResourceIdentity,
    ResourceKind,
    ResourceRecord,
    ScopeCompleteness,
    ScopeIndex,
    ScopeKind,
    ScopeLookup,
    UsageRecord,
    build_scope_lookup,
    get_placement_validated_scope_index,
)

SCOPE_CACHE_PROJECT: str = 'name = "demo"\nadapter = "duckdb"\n'
SCOPE_CACHE_MODEL: str = "MODEL(description 'Test model.');\nSELECT 1 AS id\n"


def unused_declaration_index(*, enforce_placement: bool) -> ScopeIndex:
    """Build an index containing one unused global declaration diagnostic."""

    declaration_identity: DeclarationIdentity = DeclarationIdentity(
        DeclarationKind.CONSTANT, "limit"
    )
    return get_placement_validated_scope_index(
        index=ScopeIndex(
            declarations=(
                DeclarationRecord(
                    identity=declaration_identity,
                    path="sql/constants/limit.sql",
                    line=1,
                    column=1,
                    scope=ScopeKind.GLOBAL,
                    ownership_root=OwnershipRoot("sql/constants"),
                ),
            ),
            completeness=ScopeCompleteness(runtime_usage=True, placement=False),
        ),
        enforce_placement=enforce_placement,
    )


def write_scope_cache_project(
    *, root: Path, write_repo_files: Callable[[Path, dict[str, str]], None]
) -> None:
    """Write the minimal valid project used by offline scope cache tests."""

    write_repo_files(
        root,
        {
            "sqlbuild_project.toml": SCOPE_CACHE_PROJECT,
            "sql/models/orders.sql": SCOPE_CACHE_MODEL,
        },
    )


def write_non_text_cache(*, path: Path) -> None:
    """Replace a cache with non-text bytes."""

    path.write_bytes(b"\xff")


def write_oversize_cache(*, path: Path) -> None:
    """Replace a cache with an oversized payload."""

    path.write_bytes(b"x" * (16 * 1024 * 1024 + 1))


def write_wrong_version_cache(*, path: Path) -> None:
    """Replace the envelope schema version."""

    _write_cache_field(path=path, field="schema_version", value=999)


def write_wrong_fingerprint_cache(*, path: Path) -> None:
    """Replace the envelope input fingerprint."""

    _write_cache_field(path=path, field="input_fingerprint", value="wrong")


def write_wrong_digest_cache(*, path: Path) -> None:
    """Replace the envelope payload digest."""

    _write_cache_field(path=path, field="payload_sha256", value="0" * 64)


def write_malformed_payload_cache(*, path: Path) -> None:
    """Replace the canonical index payload with a malformed shape."""

    _write_cache_field(path=path, field="payload", value={"resources": "not-a-list"})


def _write_cache_field(*, path: Path, field: str, value: object) -> None:
    envelope: dict[str, object] = cast(
        dict[str, object], json.loads(path.read_text(encoding="ascii"))
    )
    envelope[field] = value
    path.write_text(json.dumps(envelope), encoding="ascii")


def discovered_inputs(**kwargs: object) -> DiscoveredProjectInputs:
    """Return minimal discovered inputs with selected record tuples."""

    return DiscoveredProjectInputs(
        project_config=ProjectConfig(name="scope_project", adapter="duckdb"),
        local_config=LocalConfig(),
        **kwargs,  # ty: ignore[invalid-argument-type]
    )


def discovered_model(
    *, name: str, path: str, enums: tuple[EnumDeclaration, ...] = ()
) -> DiscoveredSqlModelFile:
    """Return one minimal discovered model."""

    relative: Path = Path(path)
    return DiscoveredSqlModelFile(
        file_path=relative,
        relative_path=relative,
        contents="MODEL(description 'Test model.'); SELECT 1",
        header_values={"name": name},
        header_column_locations={},
        output_column_locations={},
        query_sql="SELECT 1",
        enum_declarations=enums,
    )


def declaration_record(
    *,
    name: str,
    scope: ScopeKind,
    owner_path: str | None,
    owner: ResourceIdentity | None = None,
    path: str | None = None,
    kind: DeclarationKind = DeclarationKind.ENUM,
) -> DeclarationRecord:
    """Return one minimal declaration record."""

    return DeclarationRecord(
        DeclarationIdentity(kind, name, owner),
        path or f"sql/enums/{name}.sql",
        1,
        1,
        scope,
        OwnershipRoot("sql/models", resource_kind=ResourceKind.MODEL),
        owning_path=owner_path,
    )


def scope_index() -> ScopeIndex:
    """Return representative unsorted canonical scope facts."""

    model: ResourceIdentity = ResourceIdentity(kind=ResourceKind.MODEL, name="stg_orders")
    model_root: OwnershipRoot = OwnershipRoot(
        path="sql/models",
        kind=OwnershipRootKind.RESOURCE,
        resource_kind=ResourceKind.MODEL,
    )
    enum: DeclarationRecord = DeclarationRecord(
        DeclarationIdentity(DeclarationKind.ENUM, "order_status"),
        "sql/enums/order_status.sql",
        2,
        1,
        ScopeKind.GLOBAL,
        OwnershipRoot("sql/enums", OwnershipRootKind.GLOBAL),
        enum=EnumMetadata(
            (
                EnumMemberMetadata("OPEN"),
                EnumMemberMetadata("CLOSED"),
            ),
            "VARCHAR",
        ),
    )
    constant: DeclarationRecord = DeclarationRecord(
        DeclarationIdentity(DeclarationKind.CONSTANT, "minimum_value"),
        "sql/models/staging/constants/limits.sql",
        8,
        3,
        ScopeKind.INHERITED,
        OwnershipRoot("sql/models", resource_kind=ResourceKind.MODEL),
        owning_path="sql/models/staging",
        constant=ConstantMetadata("integer", render_as="value_list"),
    )
    macro: DeclarationRecord = DeclarationRecord(
        DeclarationIdentity(DeclarationKind.MACRO, "normalize_status"),
        "sql/macros/python/normalize_status.py",
        1,
        1,
        ScopeKind.GLOBAL,
        OwnershipRoot("sql/macros", OwnershipRootKind.GLOBAL),
        macro=MacroMetadata(("status",), (enum.identity,)),
    )
    return ScopeIndex(
        resources=(ResourceRecord(model, "sql/models/staging/stg_orders.sql", model_root),),
        declarations=(macro, constant, enum),
        usages=(UsageRecord(model, constant.identity), UsageRecord(model, enum.identity)),
        completeness=ScopeCompleteness(runtime_usage=False),
    )


def report_scope_lookup(*, extra_globals: int = 0) -> ScopeLookup:
    """Return representative complete facts for scope report tests."""

    model_root: OwnershipRoot = OwnershipRoot("sql/models", resource_kind=ResourceKind.MODEL)
    global_root: OwnershipRoot = OwnershipRoot("sql/constants", OwnershipRootKind.GLOBAL)
    orders: ResourceIdentity = ResourceIdentity(ResourceKind.MODEL, "orders")
    expected: ResourceIdentity = ResourceIdentity(ResourceKind.MODEL, "expected_orders")
    resources: tuple[ResourceRecord, ...] = (
        ResourceRecord(orders, "sql/models/staging/orders.sql", model_root),
        ResourceRecord(expected, "sql/models/marts/expected_orders.sql", model_root),
    )
    global_constant: DeclarationRecord = DeclarationRecord(
        DeclarationIdentity(DeclarationKind.CONSTANT, "warehouse_password"),
        "sql/constants/warehouse_password.yml",
        2,
        3,
        ScopeKind.GLOBAL,
        global_root,
        constant=ConstantMetadata("string"),
    )
    inherited: DeclarationRecord = DeclarationRecord(
        DeclarationIdentity(DeclarationKind.ENUM, "order_status"),
        "sql/models/staging/enums/order_status.sql",
        1,
        1,
        ScopeKind.INHERITED,
        model_root,
        owning_path="sql/models/staging",
        enum=EnumMetadata(
            tuple(EnumMemberMetadata(f"STATUS_{index:02d}") for index in range(25)),
            "VARCHAR",
        ),
    )
    sibling: DeclarationRecord = DeclarationRecord(
        DeclarationIdentity(DeclarationKind.ENUM, "mart_status"),
        "sql/models/marts/_enums/mart_status.sql",
        1,
        1,
        ScopeKind.LOCAL,
        model_root,
        owning_path="sql/models/marts",
    )
    nearby_sibling: DeclarationRecord = DeclarationRecord(
        DeclarationIdentity(DeclarationKind.ENUM, "other_status"),
        "sql/models/other/_enums/other_status.sql",
        1,
        1,
        ScopeKind.LOCAL,
        model_root,
        owning_path="sql/models/other",
    )
    private: DeclarationRecord = DeclarationRecord(
        DeclarationIdentity(DeclarationKind.ENUM, "expected_status", expected),
        "sql/models/marts/expected_orders.sql",
        4,
        2,
        ScopeKind.PRIVATE,
        model_root,
        owning_path="sql/models/marts",
    )
    macro: DeclarationRecord = DeclarationRecord(
        DeclarationIdentity(DeclarationKind.MACRO, "normalize"),
        "sql/macros/python/normalize.py",
        1,
        1,
        ScopeKind.GLOBAL,
        OwnershipRoot("sql/macros", OwnershipRootKind.GLOBAL),
        macro=MacroMetadata(("value",), (inherited.identity,), "secret-source-digest"),
    )
    generated: tuple[DeclarationRecord, ...] = tuple(
        DeclarationRecord(
            DeclarationIdentity(DeclarationKind.CONSTANT, f"global_{index:05d}"),
            f"sql/constants/generated/{index:05d}.yml",
            1,
            1,
            ScopeKind.GLOBAL,
            global_root,
            constant=ConstantMetadata("integer"),
        )
        for index in range(extra_globals)
    )
    index: ScopeIndex = ScopeIndex(
        ownership_roots=(
            model_root,
            OwnershipRoot("tests/unit", resource_kind=ResourceKind.TEST),
            OwnershipRoot("tests/scenarios", resource_kind=ResourceKind.SCENARIO),
            OwnershipRoot("sql/functions/sql", resource_kind=ResourceKind.FUNCTION),
            OwnershipRoot("sql/sources", resource_kind=ResourceKind.SOURCE),
        ),
        resources=resources,
        declarations=(
            global_constant,
            inherited,
            sibling,
            nearby_sibling,
            private,
            macro,
            *generated,
        ),
        usages=(
            UsageRecord(orders, inherited.identity),
            UsageRecord(orders, macro.identity),
            UsageRecord(orders, sibling.identity, through=expected),
            UsageRecord(expected, private.identity),
        ),
        grants=(GrantRecord(orders, sibling.identity, expected),),
        completeness=ScopeCompleteness(),
    )
    return build_scope_lookup(index=index)


_RELOCATION_ROOT: OwnershipRoot = OwnershipRoot("sql/models", resource_kind=ResourceKind.MODEL)
_CENTS_MACRO: DeclarationIdentity = DeclarationIdentity(DeclarationKind.MACRO, "to_cents")
_ORDER_SHAPE: DeclarationIdentity = DeclarationIdentity(DeclarationKind.SCHEMA, "order_shape")


def relocation_index(consumer_paths: tuple[str, ...]) -> ScopeIndex:
    """Return models sharing a staging schema that uses a staging macro."""

    resources: tuple[ResourceRecord, ...] = tuple(
        ResourceRecord(
            identity=ResourceIdentity(ResourceKind.MODEL, f"orders_{position}"),
            path=path,
            ownership_root=_RELOCATION_ROOT,
        )
        for position, path in enumerate(consumer_paths)
    )
    return ScopeIndex(
        ownership_roots=(_RELOCATION_ROOT,),
        resources=resources,
        declarations=(
            DeclarationRecord(
                identity=_ORDER_SHAPE,
                path="sql/models/staging/_sqlbuild/_schemas/order_shape.sql",
                line=1,
                column=1,
                scope=ScopeKind.LOCAL,
                ownership_root=_RELOCATION_ROOT,
                owning_path="sql/models/staging",
            ),
            DeclarationRecord(
                identity=_CENTS_MACRO,
                path="sql/models/staging/_sqlbuild/_macros/python/cents.py",
                line=1,
                column=1,
                scope=ScopeKind.LOCAL,
                ownership_root=_RELOCATION_ROOT,
                owning_path="sql/models/staging",
            ),
        ),
        usages=(
            *(UsageRecord(consumer=item.identity, declaration=_ORDER_SHAPE) for item in resources),
            UsageRecord(consumer=_ORDER_SHAPE, declaration=_CENTS_MACRO),
        ),
    )
