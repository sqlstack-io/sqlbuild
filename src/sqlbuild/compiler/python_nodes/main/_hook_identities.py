"""Identity helpers for discovered Python lifecycle hooks."""

from __future__ import annotations

from pathlib import Path

from sqlbuild._native import DiscoveredHookFunction, PythonNodeIdentity
from sqlbuild.compiler.fingerprints.constants import NODE_TYPE_HOOK
from sqlbuild.compiler.python_nodes._helpers.identity import identity_pass
from sqlbuild.compiler.python_nodes.main.identity import build_python_node_identity


def build_hook_identities(
    hook_functions: tuple[DiscoveredHookFunction, ...],
) -> dict[str, PythonNodeIdentity]:
    """Build current source identities for discovered Python hooks."""

    identities: dict[str, PythonNodeIdentity] = {}
    hook_function: DiscoveredHookFunction
    with identity_pass():
        for hook_function in hook_functions:
            identities[hook_function.name] = build_python_node_identity(
                node_type=NODE_TYPE_HOOK,
                node_name=hook_function.name,
                function=hook_function.function,
                project_dir=_project_dir(hook_function),
                decorator_config={"description": hook_function.description},
            )
    return identities


def _project_dir(hook_function: DiscoveredHookFunction) -> Path:
    return hook_function.file_path.parents[len(hook_function.relative_path.parts) - 1]
