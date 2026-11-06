"""Import project Python modules and read live definitions for native discovery."""

from __future__ import annotations

import ast
import importlib.util
import inspect
import sys
from collections.abc import Callable
from importlib.machinery import ModuleSpec
from pathlib import Path
from types import ModuleType
from typing import get_type_hints

from sqlbuild._native import (
    PYTHON_INIT_MODULE_STEM,
    PYTHON_NODE_ROOT,
    DiscoveredProvider,
    DiscoveredProviderUsage,
    EventExporterDiscoveryError,
    ProviderDiscoveryError,
    PythonNodeDiscoveryError,
    project_relative_path,
)
from sqlbuild.runtime.event_exporting.constants import EVENT_EXPORTER_EVENT_PARAMETER_NAME
from sqlbuild.runtime.observability.models import LifecycleEvent
from sqlbuild.runtime.user_code.main.register_user_code_root import register_user_code_root
from sqlbuild.runtime.user_code.main.user_code_spec import user_code_spec

_PROJECT_PYTHON_PACKAGE_MARKER: str = "__sqlbuild_project_python__"


def module_functions(module: ModuleType) -> tuple[Callable[..., object], ...]:
    """Functions defined in one loaded module, in `inspect.getmembers` order."""

    return tuple(
        value
        for _, value in inspect.getmembers(module, inspect.isfunction)
        if value.__module__ == module.__name__
    )


def module_provider_classes(module: ModuleType) -> tuple[type, ...]:
    """Concrete provider classes defined in one loaded module."""

    from sqlbuild.provider.classes.provider import Provider

    return tuple(
        value
        for _, value in inspect.getmembers(module, inspect.isclass)
        if value.__module__ == module.__name__
        and value is not Provider
        and issubclass(value, Provider)
        and not inspect.isabstract(value)
    )


def requires_arguments(function: Callable[..., object]) -> bool:
    """Whether a loaded user function declares any parameter."""

    return bool(inspect.signature(function).parameters)


def source_line(function: Callable[..., object]) -> int:
    """The first source line of a loaded user function."""

    return inspect.getsourcelines(function)[1]


def parameter_annotations(function: Callable[..., object]) -> tuple[tuple[str, object], ...]:
    """Each parameter name of a loaded user function with its resolved annotation."""

    parameters: tuple[inspect.Parameter, ...] = tuple(
        inspect.signature(function).parameters.values()
    )
    type_hints: dict[str, object] = get_type_hints(function)
    return tuple(
        (parameter.name, type_hints.get(parameter.name, parameter.annotation))
        for parameter in parameters
    )


def validate_event_exporter_signature(
    *, function: Callable[..., object], exporter_name: str, relative_path: Path
) -> None:
    """Reject lifecycle sink signatures the runtime cannot call."""

    if inspect.iscoroutinefunction(function):
        raise EventExporterDiscoveryError(
            f"Event exporter '{exporter_name}' in {relative_path} must be synchronous"
        )
    parameters: tuple[inspect.Parameter, ...] = tuple(
        inspect.signature(function).parameters.values()
    )
    if not parameters or parameters[0].name != EVENT_EXPORTER_EVENT_PARAMETER_NAME:
        raise EventExporterDiscoveryError(
            f"Event exporter '{exporter_name}' in {relative_path} must declare event first"
        )
    if any(
        parameter.kind
        in {
            inspect.Parameter.POSITIONAL_ONLY,
            inspect.Parameter.VAR_POSITIONAL,
            inspect.Parameter.VAR_KEYWORD,
        }
        for parameter in parameters
    ):
        raise EventExporterDiscoveryError(
            f"Event exporter '{exporter_name}' in {relative_path} must use named parameters"
        )
    if any(parameter.default is not inspect.Parameter.empty for parameter in parameters):
        raise EventExporterDiscoveryError(
            f"Event exporter '{exporter_name}' in {relative_path} parameters must not have defaults"
        )
    try:
        type_hints: dict[str, object] = get_type_hints(function)
    except (NameError, TypeError) as error:
        raise EventExporterDiscoveryError(
            f"Event exporter '{exporter_name}' in {relative_path} has invalid annotations"
        ) from error
    event_annotation: object = type_hints.get("event", parameters[0].annotation)
    if event_annotation not in {inspect.Parameter.empty, LifecycleEvent}:
        raise EventExporterDiscoveryError(
            f"Event exporter '{exporter_name}' in {relative_path} event must be LifecycleEvent"
        )
    return_annotation: object = type_hints.get(
        "return", inspect.signature(function).return_annotation
    )
    if return_annotation not in {inspect.Signature.empty, None, type(None)}:
        raise EventExporterDiscoveryError(
            f"Event exporter '{exporter_name}' in {relative_path} return annotation must be None"
        )


def install_python_root_package(*, node_root: Path, project_dir: Path) -> dict[str, ModuleType]:
    """Bind python to this project's python/ folder and return displaced unrelated modules."""

    existing: ModuleType | None = sys.modules.get(PYTHON_NODE_ROOT)
    owned: bool = existing is None or bool(getattr(existing, _PROJECT_PYTHON_PACKAGE_MARKER, False))
    saved_modules: dict[str, ModuleType] = {}
    module_name: str
    for module_name in _python_root_module_names():
        module: ModuleType | None = sys.modules.pop(module_name, None)
        if not owned and module is not None:
            saved_modules[module_name] = module
    importlib.invalidate_caches()
    _ = register_user_code_root(project_dir)
    init_file: Path = node_root / f"{PYTHON_INIT_MODULE_STEM}.py"
    spec: ModuleSpec | None = (
        user_code_spec(
            name=PYTHON_NODE_ROOT,
            file_path=init_file,
            root=project_dir,
            submodule_search_locations=[str(node_root)],
        )
        if init_file.is_file()
        else ModuleSpec(PYTHON_NODE_ROOT, None, is_package=True)
    )
    if spec is None:
        raise PythonNodeDiscoveryError(f"Could not load Python node package {node_root}")
    spec.submodule_search_locations = [str(node_root)]
    package: ModuleType = importlib.util.module_from_spec(spec)
    setattr(package, _PROJECT_PYTHON_PACKAGE_MARKER, True)
    sys.modules[PYTHON_NODE_ROOT] = package
    if spec.loader is None:
        return saved_modules
    old_path: list[str] = list(sys.path)
    sys.path.insert(0, str(project_dir))
    try:
        spec.loader.exec_module(package)
    except Exception as error:
        restore_python_root_modules(saved_modules=saved_modules)
        raise PythonNodeDiscoveryError(
            f"Failed to import Python node file {init_file.relative_to(project_dir)}: {error}"
        ) from error
    finally:
        sys.path = old_path
    return saved_modules


def restore_python_root_modules(*, saved_modules: dict[str, ModuleType]) -> None:
    """Put back an unrelated python package that discovery displaced."""

    if not saved_modules:
        return
    module_name: str
    for module_name in _python_root_module_names():
        sys.modules.pop(module_name, None)
    sys.modules.update(saved_modules)


def import_python_root_module(*, file_path: Path, project_dir: Path) -> ModuleType:
    """Import a python/ module under its package name, reusing it within a discovery pass."""

    relative_path: Path = project_relative_path(path=file_path, project_dir=project_dir)
    module_name: str = ".".join(relative_path.with_suffix("").parts)
    _ = register_user_code_root(project_dir)
    old_path: list[str] = list(sys.path)
    sys.path.insert(0, str(project_dir))
    try:
        return importlib.import_module(module_name)
    except Exception as error:
        raise PythonNodeDiscoveryError(
            f"Failed to import Python node file {relative_path}: {error}"
        ) from error
    finally:
        sys.path = old_path


def load_python_node_module(*, file_path: Path, project_dir: Path) -> ModuleType:
    """Execute one hook module under a private module name."""

    return _exec_project_module(
        module_name="sqlbuild_project_python_node_"
        + "_".join(
            project_relative_path(path=file_path, project_dir=project_dir).with_suffix("").parts
        ),
        file_path=file_path,
        project_dir=project_dir,
        file_label="Python node",
        error_type=PythonNodeDiscoveryError,
    )


def load_provider_module(*, file_path: Path, project_dir: Path) -> ModuleType:
    """Import one providers/ module under its package name."""

    return _load_project_package_module(
        file_path=file_path,
        project_dir=project_dir,
        file_label="provider",
        error_type=ProviderDiscoveryError,
    )


def load_sink_module(*, file_path: Path, project_dir: Path) -> ModuleType:
    """Import one sinks/ module under its package name."""

    return _load_project_package_module(
        file_path=file_path,
        project_dir=project_dir,
        file_label="sink",
        error_type=EventExporterDiscoveryError,
    )


def load_orchestration_module(*, file_path: Path, project_dir: Path) -> ModuleType:
    """Execute an orchestration module with the project's normal package imports."""

    return _load_project_package_module(
        file_path=file_path,
        project_dir=project_dir,
        file_label="orchestration",
        error_type=PythonNodeDiscoveryError,
    )


def load_materialize_function(
    *, file_path: Path, project_dir: Path
) -> Callable[..., object] | None:
    """Execute one materializations/ module and return its callable `materialize`, if any."""

    removed_hook_name: str = "prepare_version"
    removed_hook_message: str = (
        f"materialization '{file_path.stem}' at {file_path} defines "
        "'prepare_version', which was removed with virtual environments; "
        "projects run in direct mode"
    )
    try:
        module_ast: ast.Module = ast.parse(file_path.read_text(encoding="utf-8"))
    except SyntaxError as error:
        raise PythonNodeDiscoveryError(
            f"Failed to import materialization file {file_path.relative_to(project_dir)}: {error}"
        ) from error
    if any(
        isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name == removed_hook_name
        for node in module_ast.body
    ):
        raise PythonNodeDiscoveryError(removed_hook_message)
    module: ModuleType = _exec_project_module(
        module_name=".".join(
            project_relative_path(path=file_path, project_dir=project_dir).with_suffix("").parts
        ),
        file_path=file_path,
        project_dir=project_dir,
        file_label="materialization",
        error_type=PythonNodeDiscoveryError,
    )
    if hasattr(module, removed_hook_name):
        raise PythonNodeDiscoveryError(removed_hook_message)
    materialize_fn: object = getattr(module, "materialize", None)
    return materialize_fn if callable(materialize_fn) else None


def _python_root_module_names() -> tuple[str, ...]:
    return tuple(
        module_name
        for module_name in sys.modules
        if module_name == PYTHON_NODE_ROOT or module_name.startswith(f"{PYTHON_NODE_ROOT}.")
    )


def _load_project_package_module(
    *,
    file_path: Path,
    project_dir: Path,
    file_label: str,
    error_type: type[Exception],
) -> ModuleType:
    module_name: str = ".".join(
        project_relative_path(path=file_path, project_dir=project_dir).with_suffix("").parts
    )
    _evict_stale_project_package_modules(
        root_module=module_name.split(".", maxsplit=1)[0],
        project_dir=project_dir,
    )
    existing_module: ModuleType | None = sys.modules.get(module_name)
    if existing_module is not None:
        existing_file: object = getattr(existing_module, "__file__", None)
        if isinstance(existing_file, str) and Path(existing_file).resolve() == file_path.resolve():
            return existing_module
        sys.modules.pop(module_name, None)
    return _exec_project_module(
        module_name=module_name,
        file_path=file_path,
        project_dir=project_dir,
        file_label=file_label,
        error_type=error_type,
    )


def _exec_project_module(
    *,
    module_name: str,
    file_path: Path,
    project_dir: Path,
    file_label: str,
    error_type: type[Exception],
) -> ModuleType:
    spec: ModuleSpec | None = user_code_spec(
        name=module_name, file_path=file_path, root=project_dir
    )
    if spec is None or spec.loader is None:
        raise error_type(f"Could not load {file_label} file {file_path}")
    module: ModuleType = importlib.util.module_from_spec(spec)
    old_path: list[str] = list(sys.path)
    sys.path.insert(0, str(project_dir))
    try:
        sys.modules[module_name] = module
        spec.loader.exec_module(module)
    except Exception as error:
        if sys.modules.get(module_name) is module:
            sys.modules.pop(module_name, None)
        raise error_type(
            f"Failed to import {file_label} file {file_path.relative_to(project_dir)}: {error}"
        ) from error
    finally:
        sys.path = old_path
    return module


def _evict_stale_project_package_modules(*, root_module: str, project_dir: Path) -> None:
    root_path: Path = (project_dir / root_module).resolve()
    module_name: str
    module: ModuleType
    for module_name, module in tuple(sys.modules.items()):
        if module_name != root_module and not module_name.startswith(f"{root_module}."):
            continue
        module_file: object = getattr(module, "__file__", None)
        if isinstance(module_file, str):
            try:
                Path(module_file).resolve().relative_to(project_dir.resolve())
                continue
            except ValueError:
                sys.modules.pop(module_name, None)
                continue
        module_paths: object = getattr(module, "__path__", None)
        if module_paths is None:
            sys.modules.pop(module_name, None)
            continue
        if any(Path(path).resolve() == root_path for path in module_paths):
            continue
        sys.modules.pop(module_name, None)


def provider_usages(
    *, function: Callable[..., object], provider_by_name: dict[str, DiscoveredProvider]
) -> tuple[DiscoveredProviderUsage, ...]:
    """Provider parameters of a loaded user function; `TypeError` hints fall back to raw ones."""

    usages: list[DiscoveredProviderUsage] = []
    try:
        type_hints: dict[str, object] = get_type_hints(function)
    except TypeError:
        type_hints = {}
    parameter: inspect.Parameter
    for parameter in inspect.signature(function).parameters.values():
        discovered_provider: DiscoveredProvider | None = provider_by_name.get(parameter.name)
        if discovered_provider is None:
            continue
        from sqlbuild.provider.classes.provider import Provider

        annotation: object = type_hints.get(parameter.name, parameter.annotation)
        annotation_class_name: str | None = None
        annotation_module: str | None = None
        if isinstance(annotation, type) and issubclass(annotation, Provider):
            annotation_class_name = annotation.__name__
            annotation_module = annotation.__module__
        usages.append(
            DiscoveredProviderUsage(
                provider_name=discovered_provider.name,
                parameter_name=parameter.name,
                annotation_class_name=annotation_class_name,
                annotation_module=annotation_module,
            )
        )
    return tuple(usages)
