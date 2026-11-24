"""Reference visibility classification and randomized scope projects for equivalence tests."""

from __future__ import annotations

import posixpath
import random
from collections.abc import Callable
from itertools import compress
from pathlib import PurePath

from sqlbuild._native import (
    DeclarationIdentity,
    DeclarationKind,
    DeclarationRecord,
    DeclarationVisibility,
    GrantKind,
    GrantRecord,
    InaccessibleReason,
    InaccessibleRecord,
    OwnershipRoot,
    ResourceIdentity,
    ResourceKind,
    ResourceRecord,
    ScopeIndex,
    ScopeKind,
    ScopeLookup,
    ScopeTargetQuery,
    VisibilityReason,
    VisibilityRecord,
    VisibilityResolution,
    build_scope_lookup,
    normalize_scope_path,
    query_scope_target,
    resolve_scope_declaration_visibility,
    resolve_scope_visibility,
)

_FOLDERS: tuple[str, ...] = ("sales", "sale", "finance", "daily", "orders")
_ROOTS: tuple[str, ...] = ("models", "tests/unit", "sql/functions/sql")
_NAMES: tuple[str, ...] = ("status", "region", "tier", "limit", "currency", "channel")
_NOISY_SPELLINGS: tuple[Callable[[random.Random, str], str], ...] = (
    lambda _rng, path: path.replace("/", "\\"),
    lambda _rng, path: f"./{path}/",
    lambda rng, path: f"{path}/{rng.choice(_FOLDERS)}/..",
    lambda _rng, path: path,
)
_VISIBLE_REASONS: dict[ScopeKind, VisibilityReason] = {
    ScopeKind.GLOBAL: VisibilityReason.GLOBAL,
    ScopeKind.PRIVATE: VisibilityReason.PRIVATE_OWNER,
    ScopeKind.LOCAL: VisibilityReason.LOCAL_OWNER,
    ScopeKind.INHERITED: VisibilityReason.INHERITED_ANCESTOR,
}
_GRANT_REASONS: dict[GrantKind, VisibilityReason] = {
    kind: VisibilityReason.EXPECTED_MODEL for kind in GrantKind
} | {GrantKind.TESTED_MACRO: VisibilityReason.TESTED_MACRO}


def random_scope_lookup(
    *, seed: int, resource_count: int, declaration_count: int, grant_count: int
) -> ScopeLookup:
    """Build a seeded project with every scope tier, noisy owner paths, and grants."""

    rng: random.Random = random.Random(seed)
    resources: list[ResourceRecord] = []
    for index in range(resource_count):
        directory: str = _directory(rng)
        name: str = f"model_{index % max(1, resource_count - 3)}"
        resources.append(
            ResourceRecord(
                ResourceIdentity(ResourceKind.MODEL, name),
                f"{directory}/{name}.sql",
                OwnershipRoot(directory.split("/")[0], resource_kind=ResourceKind.MODEL),
            )
        )
    identities: list[ResourceIdentity] = [resource.identity for resource in resources]
    owners: list[ResourceIdentity] = [*identities, ResourceIdentity(ResourceKind.MODEL, "other")]
    declarations: list[DeclarationRecord] = []
    for _index in range(declaration_count):
        scope: ScopeKind = rng.choice(tuple(ScopeKind))
        owner_directory: str = _directory(rng)
        owning_path: str = rng.choice(_NOISY_SPELLINGS)(rng, owner_directory)
        declarations.append(
            DeclarationRecord(
                DeclarationIdentity(
                    rng.choice((DeclarationKind.ENUM, DeclarationKind.MACRO)),
                    rng.choice(_NAMES),
                    {ScopeKind.PRIVATE: rng.choice(owners)}.get(scope),
                ),
                f"{owner_directory}/_sqlbuild/enums/{rng.randint(0, 9)}.sql",
                rng.randint(1, 5),
                1,
                scope,
                OwnershipRoot(owner_directory.split("/")[0], resource_kind=ResourceKind.MODEL),
                owning_path={True: None, False: owning_path}[
                    scope is ScopeKind.GLOBAL or rng.random() < 0.1
                ],
            )
        )
    grants: list[GrantRecord] = [
        GrantRecord(
            rng.choice(identities),
            rng.choice(declarations).identity,
            rng.choice(identities),
            rng.choice(tuple(GrantKind)),
        )
        for _index in range(grant_count * bool(declarations and identities))
    ]
    return build_scope_lookup(
        index=ScopeIndex(
            resources=tuple(resources), declarations=tuple(declarations), grants=tuple(grants)
        )
    )


def visibility_mismatches(*, lookup: ScopeLookup) -> tuple[str, ...]:
    """Return every target whose indexed resolution differs from the exhaustive reference."""

    targets: list[ResourceIdentity | str] = _targets(lookup)
    differs: list[bool] = [
        resolve_scope_visibility(lookup=lookup, target=target)
        != _reference_resolution(lookup=lookup, target=target)
        for target in targets
    ]
    return tuple(map(str, compress(targets, differs)))


def declaration_visibility_mismatches(*, lookup: ScopeLookup) -> tuple[str, ...]:
    """Return every target whose reason-free resolution differs from the exhaustive reference."""

    targets: list[ResourceIdentity | str] = _targets(lookup)
    differs: list[bool] = [
        resolve_scope_declaration_visibility(lookup=lookup, target=target)
        != _reference_declaration_visibility(lookup=lookup, target=target)
        for target in targets
    ]
    return tuple(map(str, compress(targets, differs)))


def _directory(rng: random.Random) -> str:
    root: str = rng.choice(_ROOTS)
    depth: int = rng.randint(0, 3)
    return "/".join([root, *(rng.choice(_FOLDERS) for _ in range(depth))])


def _targets(lookup: ScopeLookup) -> list[ResourceIdentity | str]:
    return [
        *lookup.resources,
        *(resource.path for resource in lookup.index.resources),
        "sql/models/missing.sql",
        "model:missing",
    ]


def _parent(path: str) -> str:
    return posixpath.dirname(normalize_scope_path(path=path)) or "."


def _is_descendant(*, path: str, ancestor: str) -> bool:
    return ancestor == "." or path == ancestor or path.startswith(f"{ancestor}/")


def _reference_reason(
    *, resource: ResourceRecord, declaration: DeclarationRecord
) -> VisibilityReason | None:
    owner: str = normalize_scope_path(path=declaration.owning_path or ".")
    parent: str = _parent(resource.path)
    visible: dict[ScopeKind, bool] = {
        ScopeKind.GLOBAL: True,
        ScopeKind.PRIVATE: declaration.identity.owner == resource.identity,
        ScopeKind.LOCAL: parent == owner,
        ScopeKind.INHERITED: _is_descendant(path=parent, ancestor=owner),
    }
    return {True: _VISIBLE_REASONS[declaration.scope], False: None}[visible[declaration.scope]]


def _reference_inaccessible_reason(
    *, resource: ResourceRecord, declaration: DeclarationRecord
) -> InaccessibleReason:
    owner: str = normalize_scope_path(path=declaration.owning_path or ".")
    candidates: tuple[tuple[bool, InaccessibleReason], ...] = (
        (declaration.scope is ScopeKind.PRIVATE, InaccessibleReason.PRIVATE_OWNER),
        (declaration.scope is ScopeKind.LOCAL, InaccessibleReason.LOCAL_BOUNDARY),
        (
            _is_descendant(path=owner, ancestor=_parent(resource.path)),
            InaccessibleReason.DESCENDANT_SCOPE,
        ),
        (
            declaration.ownership_root.path == resource.ownership_root.path,
            InaccessibleReason.SIBLING_SCOPE,
        ),
        (True, InaccessibleReason.UNRELATED_SCOPE),
    )
    return next(
        compress(
            (reason for _matches, reason in candidates),
            (matches for matches, _reason in candidates),
        )
    )


def _reference_resolution(
    *, lookup: ScopeLookup, target: ResourceIdentity | str | PurePath
) -> VisibilityResolution:
    """Classify every declaration against every matched resource, as before indexing."""

    query: ScopeTargetQuery = query_scope_target(lookup=lookup, target=target)
    visible: list[VisibilityRecord] = []
    inaccessible: list[InaccessibleRecord] = []
    for resource in query.matches:
        grants_by_declaration: dict[DeclarationIdentity, list[GrantRecord]] = {}
        for grant in lookup.grants_by_resource.get(resource.identity, ()):
            grants_by_declaration.setdefault(grant.declaration, []).append(grant)
        for declaration in lookup.index.declarations:
            positive: VisibilityReason | None = _reference_reason(
                resource=resource, declaration=declaration
            )
            granted: list[GrantRecord] | None = grants_by_declaration.get(declaration.identity)
            visible.extend(
                VisibilityRecord(resource.identity, declaration.identity, reason)
                for reason in filter(None, (positive,))
            )
            visible.extend(
                VisibilityRecord(
                    resource.identity,
                    declaration.identity,
                    _GRANT_REASONS[grant.kind],
                    grant.through,
                )
                for grant in granted or ()
            )
            inaccessible.extend(
                compress(
                    [
                        InaccessibleRecord(
                            resource.identity,
                            declaration.identity,
                            _reference_inaccessible_reason(
                                resource=resource, declaration=declaration
                            ),
                        )
                    ],
                    [granted is None and positive is None],
                )
            )
    return VisibilityResolution(query, tuple(visible), tuple(inaccessible))


def _reference_declaration_visibility(
    *, lookup: ScopeLookup, target: ResourceIdentity | str | PurePath
) -> DeclarationVisibility:
    expected: VisibilityResolution = _reference_resolution(lookup=lookup, target=target)
    return DeclarationVisibility(
        expected.target,
        expected.visible,
        tuple(record.declaration for record in expected.inaccessible),
    )
