"""Scratch copy for the D136 negative test."""

from __future__ import annotations
import importlib.util
import inspect
import sys
from importlib.machinery import ModuleSpec
from pathlib import Path
from types import ModuleType
from typing import Any, cast
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.strict_adapter import StrictAdapter
from sqlbuild.adapter.contract.constants import PYTHON_IDENTIFIER_REPLACEMENT, PYTHON_INIT_FILE_NAME
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.discovery.models import DiscoveredAdapter
from sqlbuild.runtime.user_code.main.user_code_spec import user_code_spec


def discover_project_adapters_copy(
    *,
    project_dir: Path,
    reserved_names: frozenset[str] = frozenset(),
) -> dict[str, type[StrictAdapter]]:
    """Discover adapter classes from a project's adapters directory."""

    adapters_dir: Path = project_dir / "adapters"
    if not adapters_dir.is_dir():
        return {}

    discovered: dict[str, DiscoveredAdapter] = {}
    file_path: Path
    for file_path in _iter_adapter_files(adapters_dir):
        module: ModuleType = _load_adapter_module(project_dir=project_dir, file_path=file_path)
        adapter: DiscoveredAdapter
        for adapter in _discover_module_adapters(module=module, file_path=file_path):
            if adapter.adapter_name in reserved_names:
                raise AdapterUserError(
                    message=f"Project-local adapter '{adapter.adapter_name}' in {file_path} "
                    "shadows a built-in adapter name. Choose a distinct adapter_name."
                )
            previous: DiscoveredAdapter | None = discovered.get(adapter.adapter_name)
            if previous is not None:
                raise AdapterUserError(
                    message=f"Duplicate project-local adapter_name '{adapter.adapter_name}' in "
                    f"{file_path} and {previous.file_path}"
                )
            discovered[adapter.adapter_name] = adapter

    return {name: adapter.adapter_class for name, adapter in discovered.items()}
