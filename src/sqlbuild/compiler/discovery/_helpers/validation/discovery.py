"""Cross-file discovery validation, run natively over the facts of one discovery."""

from __future__ import annotations

from collections.abc import Callable
from typing import Protocol

import sqlbuild._native as _native
from sqlbuild._native import (
    DiscoveredAssetFunction,
    DiscoveredCheckFunction,
    DiscoveredLoaderFunction,
    DiscoveredProjectInputs,
)
from sqlbuild.python_nodes.main.read_asset_definition import read_asset_definition
from sqlbuild.python_nodes.main.read_check_definition import read_check_definition
from sqlbuild.python_nodes.main.read_loader_definition import read_loader_definition
from sqlbuild.python_nodes.models import DiscoveredItemRef, NodeSetRef, SqlResourceRef

type _PythonNode = DiscoveredLoaderFunction | DiscoveredAssetFunction | DiscoveredCheckFunction


class _NodeDefinition(Protocol):
    @property
    def name(self) -> str: ...


_DEFINITION_READERS: tuple[Callable[[Callable[..., object]], _NodeDefinition | None], ...] = (
    read_loader_definition,
    read_asset_definition,
    read_check_definition,
)


def validate_discovered_inputs(discovered_inputs: DiscoveredProjectInputs) -> None:
    """Validate cross-file conflicts across discovered project inputs."""

    _native.validate_discovered_project_inputs(discovered_inputs, _nodes(discovered_inputs))


def _nodes(inputs: DiscoveredProjectInputs) -> list[dict[str, object]]:
    nodes: tuple[tuple[str, _PythonNode], ...] = (
        *(("loader", node) for node in inputs.loader_functions),
        *(("asset", node) for node in inputs.asset_functions),
        *(("check", node) for node in inputs.check_functions),
    )
    owners: dict[object, int] = {node.function: index for index, (_, node) in enumerate(nodes)}
    return [_node(kind=kind, node=node, owners=owners) for kind, node in nodes]


def _node(*, kind: str, node: _PythonNode, owners: dict[object, int]) -> dict[str, object]:
    return {
        "kind": kind,
        "name": node.name,
        "path": str(node.relative_path),
        "decorator_config": isinstance(node, DiscoveredLoaderFunction)
        and _loader_has_decorator_config(node),
        "depends_on": [
            (dependency.name, None, None)
            if isinstance(dependency, SqlResourceRef)
            else (None, owners.get(dependency), _decorated_name(dependency))
            for dependency in node.depends_on
            if not isinstance(dependency, NodeSetRef)
        ],
    }


def _loader_has_decorator_config(loader_function: DiscoveredLoaderFunction) -> bool:
    return loader_function.destination is not None or bool(loader_function.columns)


def _decorated_name(dependency: object) -> str | None:
    if isinstance(dependency, DiscoveredItemRef):
        return dependency.name
    if not callable(dependency):
        return None
    for read_definition in _DEFINITION_READERS:
        definition: _NodeDefinition | None = read_definition(dependency)
        if definition is not None:
            return definition.name
    return None
