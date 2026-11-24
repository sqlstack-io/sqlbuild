"""Project-local adapter discovery helpers."""

from __future__ import annotations

import importlib.util
import inspect
import sys
from importlib.machinery import ModuleSpec
from pathlib import Path
from types import ModuleType
from typing import Any, cast

from sqlbuild._native import ADAPTERS_PATH
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.strict_adapter import StrictAdapter
from sqlbuild.adapter.contract.constants import PYTHON_IDENTIFIER_REPLACEMENT, PYTHON_INIT_FILE_NAME
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.discovery.models import DiscoveredAdapter
from sqlbuild.runtime.user_code.main.user_code_spec import user_code_spec


def discover_project_adapters(
    *,
    project_dir: Path,
    reserved_names: frozenset[str] = frozenset(),
) -> dict[str, type[StrictAdapter]]:
    """Discover adapter classes from a project's python/adapters directory."""

    adapters_dir: Path = project_dir / ADAPTERS_PATH
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


def _iter_adapter_files(adapters_dir: Path) -> tuple[Path, ...]:
    return tuple(
        sorted(
            file_path
            for file_path in adapters_dir.rglob("*.py")
            if _is_public_adapter_file(adapters_dir=adapters_dir, file_path=file_path)
        )
    )


def _is_public_adapter_file(*, adapters_dir: Path, file_path: Path) -> bool:
    relative_path: Path = file_path.relative_to(adapters_dir)
    if file_path.name == PYTHON_INIT_FILE_NAME:
        return False
    return not any(part.startswith("_") for part in relative_path.parts)


def _load_adapter_module(*, project_dir: Path, file_path: Path) -> ModuleType:
    module_name: str = _module_name_for_path(project_dir=project_dir, file_path=file_path)
    spec: ModuleSpec | None = user_code_spec(
        name=module_name, file_path=file_path, root=project_dir
    )
    if spec is None or spec.loader is None:
        raise AdapterUserError(
            message=f"Could not load project-local adapter module from {file_path}"
        )
    module: ModuleType = importlib.util.module_from_spec(spec)
    original_path: list[str] = list(sys.path)
    sys.modules[module_name] = module
    try:
        sys.path.insert(0, str(project_dir))
        spec.loader.exec_module(module)
    except Exception as error:
        raise AdapterUserError(
            message=f"Error importing project-local adapter module {file_path}: {error}"
        ) from error
    finally:
        sys.path = original_path
    return module


def _module_name_for_path(*, project_dir: Path, file_path: Path) -> str:
    relative_path: Path = file_path.relative_to(project_dir).with_suffix("")
    normalized_parts: tuple[str, ...] = tuple(
        _normalize_module_part(part) for part in relative_path.parts
    )
    return "sqlbuild_project_adapters." + ".".join(normalized_parts)


def _normalize_module_part(part: str) -> str:
    return "".join(
        character
        if character.isalnum() or character == PYTHON_IDENTIFIER_REPLACEMENT
        else PYTHON_IDENTIFIER_REPLACEMENT
        for character in part
    )


def _discover_module_adapters(
    *, module: ModuleType, file_path: Path
) -> tuple[DiscoveredAdapter, ...]:
    discovered: list[DiscoveredAdapter] = []
    class_name: str
    adapter_class: type[Any]
    for class_name, adapter_class in inspect.getmembers(module, inspect.isclass):
        if adapter_class.__module__ != module.__name__:
            continue
        adapter_name: Any = adapter_class.__dict__.get("adapter_name")
        is_adapter_class: bool = issubclass(adapter_class, StrictAdapter)
        if adapter_name is None and not is_adapter_class:
            continue
        if adapter_name is not None and not is_adapter_class:
            raise AdapterUserError(
                message=f"Class '{class_name}' in {file_path} defines adapter_name but does not "
                "subclass StrictAdapter"
            )
        if not isinstance(adapter_name, str) or not adapter_name:
            raise AdapterUserError(
                message=f"Adapter class '{class_name}' in {file_path} must define a non-empty "
                "string adapter_name"
            )
        discovered.append(
            DiscoveredAdapter(
                adapter_name=adapter_name,
                adapter_class=cast(type[BaseAdapter], adapter_class),
                file_path=file_path,
            )
        )
    return tuple(discovered)
