"""Python-node facts read from loaded user functions: identities and dependencies."""

from __future__ import annotations

from collections.abc import Callable
from contextlib import AbstractContextManager
from pathlib import Path

from sqlbuild._native import PythonNodeIdentity
from sqlbuild.compiler.python_nodes._helpers.identity import build_python_identity, identity_pass
from sqlbuild.python_nodes.main.read_asset_definition import read_asset_definition
from sqlbuild.python_nodes.main.read_check_definition import read_check_definition
from sqlbuild.python_nodes.main.read_loader_definition import read_loader_definition
from sqlbuild.python_nodes.main.read_task_definition import read_task_definition
from sqlbuild.python_nodes.models import (
    AssetDefinition,
    CheckDefinition,
    LoaderDefinition,
    TaskDefinition,
)


def python_identity_pass() -> AbstractContextManager[None]:
    """One discovery pass: identities built inside share each object's source facts."""

    return identity_pass()


def python_node_identity(
    *,
    kind: str,
    name: str,
    function: Callable[..., object],
    file_path: Path,
    decorator_config: dict[str, object],
) -> PythonNodeIdentity:
    """Read the identity of one loaded user node function."""

    return build_python_identity(
        node_type=kind,
        node_name=name,
        function=function,
        project_dir=file_path.parent,
        decorator_config=decorator_config,
    )


def python_node_dependency_key(dependency: object) -> object | tuple[str, str]:
    """The node name a decorated dependency declares, or the dependency itself."""

    loader_definition: LoaderDefinition | None = (
        read_loader_definition(dependency) if callable(dependency) else None
    )
    if loader_definition is not None:
        return ("name", loader_definition.name)
    task_definition: TaskDefinition | None = (
        read_task_definition(dependency) if callable(dependency) else None
    )
    if task_definition is not None:
        return ("name", task_definition.name)
    asset_definition: AssetDefinition | None = (
        read_asset_definition(dependency) if callable(dependency) else None
    )
    if asset_definition is not None:
        return ("name", asset_definition.name)
    check_definition: CheckDefinition | None = (
        read_check_definition(dependency) if callable(dependency) else None
    )
    if check_definition is not None:
        return ("name", check_definition.name)
    return dependency
