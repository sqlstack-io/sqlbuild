"""Read the native Cargo workspace and report crate layering violations."""

from __future__ import annotations

import tomllib
from pathlib import Path
from typing import Any

from scripts.native_layering.constants import (
    DEPENDENCY_TABLES,
    LAYERS_METADATA_KEY,
    POLYGLOT_PACKAGE,
    PYO3_PACKAGE,
)
from scripts.native_layering.models import NativeLayers


def get_native_layering_errors(root: Path) -> tuple[str, ...]:
    """Return every layering violation of the Cargo workspace rooted at `root`."""
    workspace: dict[str, Any] = _read_toml(root / "Cargo.toml")
    layers: NativeLayers | None = _declared_layers(workspace)
    if layers is None:
        return (f"Cargo.toml must declare [workspace.metadata.{LAYERS_METADATA_KEY}].",)
    members: dict[str, set[str]] = _member_dependencies(root=root, workspace=workspace)
    errors: list[str] = _order_errors(layers=layers, members=members)
    if errors:
        return tuple(errors)
    rank: dict[str, int] = {name: index for index, name in enumerate(layers.order)}
    for name in layers.order:
        for dependency in sorted(members[name] & members.keys()):
            if rank[dependency] >= rank[name]:
                errors.append(f"{name} depends on {dependency}, which is not in an earlier layer.")
    lock_graph: dict[str, set[str]] = _lock_graph(root / "Cargo.lock")
    for name in layers.order:
        reachable: set[str] = _reachable(
            direct=members[name] | lock_graph.get(name, set()), graph=lock_graph
        )
        if name not in layers.python_boundary and _reaches(
            reachable=reachable, package=PYO3_PACKAGE
        ):
            allowed: str = ", ".join(layers.python_boundary)
            errors.append(f"{name} depends on {PYO3_PACKAGE}; only {allowed} may.")
        if rank[name] < rank[layers.polyglot_floor] and _reaches(
            reachable=reachable, package=POLYGLOT_PACKAGE
        ):
            errors.append(
                f"{name} depends on {POLYGLOT_PACKAGE}, which starts at {layers.polyglot_floor}."
            )
    return tuple(errors)


def _declared_layers(workspace: dict[str, Any]) -> NativeLayers | None:
    metadata: Any = workspace.get("workspace", {}).get("metadata", {}).get(LAYERS_METADATA_KEY)
    if not isinstance(metadata, dict):
        return None
    return NativeLayers(
        order=tuple(metadata.get("order", ())),
        python_boundary=tuple(metadata.get("python-boundary", ())),
        polyglot_floor=str(metadata.get("polyglot-floor", "")),
    )


def _order_errors(*, layers: NativeLayers, members: dict[str, set[str]]) -> list[str]:
    errors: list[str] = []
    if len(set(layers.order)) != len(layers.order):
        errors.append("The native layer order lists a crate more than once.")
    for name in sorted(members.keys() - set(layers.order)):
        errors.append(f"Workspace crate {name} is missing from the native layer order.")
    for name in sorted(set(layers.order) - members.keys()):
        errors.append(f"Native layer order lists {name}, which is not a workspace crate.")
    if not layers.python_boundary:
        errors.append("The native layers must list at least one python-boundary crate.")
    for role, name in (
        *(("python-boundary", name) for name in layers.python_boundary),
        ("polyglot-floor", layers.polyglot_floor),
    ):
        if name not in layers.order:
            errors.append(f"Native layer {role} {name!r} is not in the layer order.")
    return errors


def _member_dependencies(*, root: Path, workspace: dict[str, Any]) -> dict[str, set[str]]:
    members: dict[str, set[str]] = {}
    shared: dict[str, Any] = workspace.get("workspace", {}).get("dependencies", {})
    for pattern in workspace.get("workspace", {}).get("members", ()):
        for directory in sorted(root.glob(pattern)):
            manifest: dict[str, Any] = _read_toml(directory / "Cargo.toml")
            members[manifest["package"]["name"]] = _direct_dependencies(
                manifest=manifest, shared=shared
            )
    return members


def _direct_dependencies(*, manifest: dict[str, Any], shared: dict[str, Any]) -> set[str]:
    tables: list[dict[str, Any]] = [manifest.get(table, {}) for table in DEPENDENCY_TABLES]
    for target in manifest.get("target", {}).values():
        tables.extend(target.get(table, {}) for table in DEPENDENCY_TABLES)
    names: set[str] = set()
    for table in tables:
        for key, spec in table.items():
            names.add(_package_name(key=key, spec=spec, shared=shared))
    return names


def _package_name(*, key: str, spec: Any, shared: dict[str, Any]) -> str:
    """Resolve a dependency key to its package, following `package` renames and inheritance."""
    if isinstance(spec, dict) and spec.get("workspace") is True:
        spec = shared.get(key)
    renamed: Any = spec.get("package") if isinstance(spec, dict) else None
    return renamed if isinstance(renamed, str) else key


def _lock_graph(lockfile: Path) -> dict[str, set[str]]:
    graph: dict[str, set[str]] = {}
    for package in _read_toml(lockfile).get("package", ()):
        dependencies: set[str] = {entry.split(" ")[0] for entry in package.get("dependencies", ())}
        graph.setdefault(package["name"], set()).update(dependencies)
    return graph


def _reachable(*, direct: set[str], graph: dict[str, set[str]]) -> set[str]:
    seen: set[str] = set()
    pending: list[str] = list(direct)
    while pending:
        current: str = pending.pop()
        if current not in seen:
            seen.add(current)
            pending.extend(graph.get(current, ()))
    return seen


def _reaches(*, reachable: set[str], package: str) -> bool:
    return any(name == package or name.startswith(f"{package}-") for name in reachable)


def _read_toml(path: Path) -> dict[str, Any]:
    with path.open("rb") as handle:
        return tomllib.load(handle)
