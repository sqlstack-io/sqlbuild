"""Helpers for read-only Python-node identity computation."""

from __future__ import annotations

import ast
import hashlib
import inspect
import json
import textwrap
import weakref
from collections.abc import Callable, Iterable, Mapping
from dataclasses import asdict
from functools import lru_cache
from pathlib import Path
from types import ModuleType
from typing import Any, cast

from sqlbuild._native import PythonIdentityDependency, PythonNodeIdentity

_SOURCE_PATHS: weakref.WeakKeyDictionary[object, Path | None] = weakref.WeakKeyDictionary()
_NORMALIZED_SOURCES: weakref.WeakKeyDictionary[object, str] = weakref.WeakKeyDictionary()

_IGNORED_PATH_PARTS: frozenset[str] = frozenset(
    {
        ".venv",
        "venv",
        "env",
        ".tox",
        ".nox",
        "site-packages",
        "dist-packages",
        "__pycache__",
    }
)


def build_python_identity(
    *,
    node_type: str,
    node_name: str,
    function: Callable[..., object],
    project_dir: Path,
    decorator_config: Mapping[str, object],
) -> PythonNodeIdentity:
    """Build read-only identity metadata for one Python node callable."""

    allowed_roots: tuple[Path, ...] = _allowed_roots(project_dir=project_dir)
    source: str = _normalized_source(function)
    source_path: Path | None = _source_path(function)
    source_hash: str = _hash_text(source)
    normalized_config: str = _stable_json(dict(decorator_config))
    definition_hash: str = _hash_text(
        _stable_json(
            {
                "decorator_config": normalized_config,
                "source_hash": source_hash,
            }
        )
    )
    dependencies: tuple[PythonIdentityDependency, ...] = _collect_dependencies(
        function=function,
        allowed_roots=allowed_roots,
    )
    definition_payload: dict[str, object] = {
        "decorator_config": dict(decorator_config),
        "node_name": node_name,
        "node_type": node_type,
        "object_module": function.__module__,
        "object_qualname": _object_qualname(function),
        "source_hash": source_hash,
        "source_path": _display_path(source_path=source_path, roots=allowed_roots),
        "source_text": source,
    }
    metadata_payload: dict[str, object] = {
        "dependencies": tuple(asdict(dependency) for dependency in dependencies)
    }
    version_hash: str = _hash_text(
        _stable_json(
            {
                "definition_hash": definition_hash,
                "dependencies": tuple(asdict(dependency) for dependency in dependencies),
            }
        )
    )
    return PythonNodeIdentity(
        node_type=node_type,
        node_name=node_name,
        source_path=_display_path(source_path=source_path, roots=allowed_roots),
        source_hash=source_hash,
        definition_hash=definition_hash,
        version_hash=version_hash,
        dependencies=dependencies,
        definition_json=_stable_json(definition_payload),
        metadata_json=_stable_json(metadata_payload),
    )


def _allowed_roots(*, project_dir: Path) -> tuple[Path, ...]:
    roots: list[Path] = [project_dir.resolve()]
    git_root: Path | None = _nearest_git_root(project_dir.resolve())
    if git_root is not None and git_root not in roots:
        roots.append(git_root)
    return tuple(roots)


def _nearest_git_root(path: Path) -> Path | None:
    current: Path = path if path.is_dir() else path.parent
    for candidate in (current, *current.parents):
        if (candidate / ".git").exists():
            return candidate.resolve()
    return None


def _collect_dependencies(
    *,
    function: Callable[..., object],
    allowed_roots: tuple[Path, ...],
) -> tuple[PythonIdentityDependency, ...]:
    dependencies: dict[tuple[str, str], PythonIdentityDependency] = {}
    visited: set[tuple[str, str]] = set()
    _visit_object(
        obj=function,
        allowed_roots=allowed_roots,
        visited=visited,
        dependencies=dependencies,
        include_current=False,
    )
    return tuple(
        dependencies[key]
        for key in sorted(
            dependencies,
            key=lambda item: (dependencies[item].source_path, dependencies[item].module, item[1]),
        )
    )


def _visit_object(
    *,
    obj: object,
    allowed_roots: tuple[Path, ...],
    visited: set[tuple[str, str]],
    dependencies: dict[tuple[str, str], PythonIdentityDependency],
    include_current: bool,
) -> None:
    source_path: Path | None = _source_path(obj)
    qualname: str = _object_qualname(obj)
    if source_path is None:
        return
    key: tuple[str, str] = (str(source_path.resolve()), qualname)
    if key in visited:
        return
    visited_objects: set[tuple[str, str]] = visited
    visited_objects.add(key)

    if include_current:
        dependency: PythonIdentityDependency | None = _dependency_for_object(
            obj=obj,
            source_path=source_path,
            allowed_roots=allowed_roots,
        )
        if dependency is not None:
            collected_dependencies: dict[tuple[str, str], PythonIdentityDependency] = dependencies
            collected_dependencies[key] = dependency

    source: str = _normalized_source(obj)
    module: ModuleType | None = inspect.getmodule(obj)
    if module is None:
        return
    globals_map: dict[str, object] = vars(module)
    for ref in _called_references(source):
        target: object | None = _resolve_reference(ref=ref, globals_map=globals_map)
        if target is None or target is obj:
            continue
        target_path: Path | None = _source_path(target)
        if target_path is None or not _is_first_party_source(
            source_path=target_path,
            allowed_roots=allowed_roots,
        ):
            continue
        _visit_object(
            obj=target,
            allowed_roots=allowed_roots,
            visited=visited,
            dependencies=dependencies,
            include_current=True,
        )


def _dependency_for_object(
    *, obj: object, source_path: Path, allowed_roots: tuple[Path, ...]
) -> PythonIdentityDependency | None:
    if not _is_first_party_source(source_path=source_path, allowed_roots=allowed_roots):
        return None
    return PythonIdentityDependency(
        kind=_object_kind(obj),
        module=_object_module(obj),
        qualname=_object_qualname(obj),
        source_path=_display_path(source_path=source_path, roots=allowed_roots),
        source_hash=_hash_text(_normalized_source(obj)),
        source_text=_normalized_source(obj),
    )


@lru_cache(maxsize=4096)
def _called_references(source: str) -> tuple[tuple[str, ...], ...]:
    try:
        tree: ast.AST = ast.parse(source)
    except SyntaxError:
        return ()
    refs: set[tuple[str, ...]] = set()
    node: ast.AST
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue
        ref: tuple[str, ...] | None = _reference_parts(node.func)
        if ref is not None:
            refs.add(ref)
    return tuple(sorted(refs))


def _reference_parts(node: ast.AST) -> tuple[str, ...] | None:
    if isinstance(node, ast.Name):
        return (node.id,)
    if isinstance(node, ast.Attribute):
        parent: tuple[str, ...] | None = _reference_parts(node.value)
        if parent is None:
            return None
        return (*parent, node.attr)
    return None


def _resolve_reference(*, ref: tuple[str, ...], globals_map: Mapping[str, object]) -> object | None:
    if not ref:
        return None
    current: object | None = globals_map.get(ref[0])
    if current is None:
        return None
    attr: str
    for attr in ref[1:]:
        current = getattr(current, attr, None)
        if current is None:
            return None
    return current


def _is_first_party_source(*, source_path: Path, allowed_roots: tuple[Path, ...]) -> bool:
    return _first_party_source(source_path, allowed_roots)


@lru_cache(maxsize=4096)
def _first_party_source(source_path: Path, allowed_roots: tuple[Path, ...]) -> bool:
    resolved: Path = source_path.resolve()
    if any(part in _IGNORED_PATH_PARTS for part in resolved.parts):
        return False
    return any(_is_relative_to(path=resolved, root=root) for root in allowed_roots)


def _source_path(obj: object) -> Path | None:
    """The object's source file, read once per object: nodes share most of their dependencies."""
    return _object_cached(_SOURCE_PATHS, obj, _uncached_source_path)


def _uncached_source_path(obj: object) -> Path | None:
    try:
        raw_path: str | None = inspect.getsourcefile(cast(Any, obj))
    except TypeError:
        return None
    if raw_path is None:
        return None
    return Path(raw_path)


def _normalized_source(obj: object) -> str:
    """The object's dedented source, read once per object: `inspect.getsource` rescans its file."""
    return _object_cached(_NORMALIZED_SOURCES, obj, _uncached_normalized_source)


def _uncached_normalized_source(obj: object) -> str:
    try:
        return textwrap.dedent(inspect.getsource(cast(Any, obj))).strip()
    except (OSError, TypeError):
        return ""


def _object_kind(obj: object) -> str:
    if inspect.ismodule(obj):
        return "module"
    if inspect.isclass(obj):
        return "class"
    if inspect.isfunction(obj):
        return "function"
    return type(obj).__name__


def _object_module(obj: object) -> str:
    if inspect.ismodule(obj):
        return obj.__name__
    return str(getattr(obj, "__module__", ""))


def _object_qualname(obj: object) -> str:
    if inspect.ismodule(obj):
        return "<module>"
    return str(getattr(obj, "__qualname__", getattr(obj, "__name__", type(obj).__name__)))


def _display_path(*, source_path: Path | None, roots: Iterable[Path]) -> str:
    return _cached_display_path(source_path, tuple(roots))


@lru_cache(maxsize=4096)
def _cached_display_path(source_path: Path | None, roots: tuple[Path, ...]) -> str:
    if source_path is None:
        return ""
    resolved: Path = source_path.resolve()
    root: Path
    for root in sorted(
        (path.resolve() for path in roots),
        key=lambda item: len(item.parts),
        reverse=True,
    ):
        if _is_relative_to(path=resolved, root=root):
            return resolved.relative_to(root).as_posix()
    return resolved.as_posix()


def _is_relative_to(*, path: Path, root: Path) -> bool:
    try:
        path.relative_to(root.resolve())
    except ValueError:
        return False
    return True


@lru_cache(maxsize=8192)
def _hash_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def _stable_json(value: object) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), default=repr)


def _object_cached[T](
    cache: weakref.WeakKeyDictionary[object, T], obj: object, compute: Callable[[object], T]
) -> T:
    """Memoise per live object; objects that cannot be weakly referenced are computed each time."""
    try:
        return cache[obj]
    except KeyError:
        value: T = compute(obj)
        cache[obj] = value
        return value
    except TypeError:
        return compute(obj)
