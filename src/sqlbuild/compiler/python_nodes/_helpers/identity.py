"""Helpers for read-only Python-node identity computation."""

from __future__ import annotations

import inspect
import json
from collections.abc import Callable, Iterator, Mapping
from contextlib import contextmanager
from contextvars import ContextVar, Token
from dataclasses import asdict
from pathlib import Path
from types import ModuleType
from typing import Any, cast

from sqlbuild._native import PythonIdentityDependency, PythonNodeIdentity
from sqlbuild.compiler.python_nodes._helpers.object_source import class_anchor, hash_text
from sqlbuild.compiler.python_nodes.classes.identity_pass import IdentityPass

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

_ACTIVE_PASS: ContextVar[IdentityPass | None] = ContextVar("python_identity_pass", default=None)


@contextmanager
def identity_pass() -> Iterator[None]:
    """Share source work between the identities built inside; a nested pass joins the outer one."""

    if _ACTIVE_PASS.get() is not None:
        yield
        return
    token: Token[IdentityPass | None] = _ACTIVE_PASS.set(IdentityPass())
    try:
        yield
    finally:
        _ACTIVE_PASS.reset(token)


def build_python_identity(
    *,
    node_type: str,
    node_name: str,
    function: Callable[..., object],
    project_dir: Path,
    decorator_config: Mapping[str, object],
) -> PythonNodeIdentity:
    """Build read-only identity metadata for one Python node callable."""

    memo: IdentityPass = _ACTIVE_PASS.get() or IdentityPass()
    allowed_roots: tuple[Path, ...] = _allowed_roots(project_dir=project_dir)
    source: str = memo.source(function)
    source_path: Path | None = memo.source_path(function)
    source_hash: str = memo.source_hash(function)
    normalized_config: str = _stable_json(dict(decorator_config))
    definition_hash: str = hash_text(
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
        memo=memo,
    )
    dependency_rows: tuple[dict[str, object], ...] = tuple(
        asdict(dependency) for dependency in dependencies
    )
    display_path: str = _display_path(source_path=source_path, roots=allowed_roots, memo=memo)
    definition_payload: dict[str, object] = {
        "decorator_config": dict(decorator_config),
        "node_name": node_name,
        "node_type": node_type,
        "object_module": function.__module__,
        "object_qualname": _object_qualname(function),
        "source_hash": source_hash,
        "source_path": display_path,
        "source_text": source,
    }
    metadata_payload: dict[str, object] = {"dependencies": dependency_rows}
    version_hash: str = hash_text(
        _stable_json(
            {
                "definition_hash": definition_hash,
                "dependencies": dependency_rows,
            }
        )
    )
    return PythonNodeIdentity(
        node_type=node_type,
        node_name=node_name,
        source_path=display_path,
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
    memo: IdentityPass,
) -> tuple[PythonIdentityDependency, ...]:
    dependencies: dict[tuple[str, str], PythonIdentityDependency] = {}
    visited: set[tuple[str, str]] = set()
    _visit_object(
        obj=function,
        allowed_roots=allowed_roots,
        memo=memo,
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
    memo: IdentityPass,
    visited: set[tuple[str, str]],
    dependencies: dict[tuple[str, str], PythonIdentityDependency],
    include_current: bool,
) -> None:
    source_path: Path | None = memo.source_path(obj)
    qualname: str = _object_qualname(obj)
    if source_path is None:
        return
    key: tuple[str, str] = (str(memo.resolved(source_path)), qualname)
    if key in visited:
        return
    visited_objects: set[tuple[str, str]] = visited
    visited_objects.add(key)

    if include_current:
        dependency: PythonIdentityDependency | None = _dependency_for_object(
            obj=obj,
            source_path=source_path,
            allowed_roots=allowed_roots,
            memo=memo,
        )
        if dependency is not None:
            collected_dependencies: dict[tuple[str, str], PythonIdentityDependency] = dependencies
            collected_dependencies[key] = dependency

    unwrapped: object = inspect.unwrap(cast(Any, obj))
    globals_map: Mapping[str, object] | None = getattr(
        class_anchor(unwrapped) or unwrapped, "__globals__", None
    )
    if globals_map is None:
        module: ModuleType | None = memo.module(obj)
        if module is None:
            return
        globals_map = vars(module)
    for ref in memo.references(obj):
        target: object | None = _resolve_reference(ref=ref, globals_map=globals_map)
        if target is None or target is obj:
            continue
        target_path: Path | None = memo.source_path(target)
        if target_path is None or not _is_first_party_source(
            source_path=target_path,
            allowed_roots=allowed_roots,
            memo=memo,
        ):
            continue
        _visit_object(
            obj=target,
            allowed_roots=allowed_roots,
            memo=memo,
            visited=visited,
            dependencies=dependencies,
            include_current=True,
        )


def _dependency_for_object(
    *, obj: object, source_path: Path, allowed_roots: tuple[Path, ...], memo: IdentityPass
) -> PythonIdentityDependency | None:
    if not _is_first_party_source(source_path=source_path, allowed_roots=allowed_roots, memo=memo):
        return None
    return PythonIdentityDependency(
        kind=_object_kind(obj),
        module=_object_module(obj),
        qualname=_object_qualname(obj),
        source_path=_display_path(source_path=source_path, roots=allowed_roots, memo=memo),
        source_hash=memo.source_hash(obj),
        source_text=memo.source(obj),
    )


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


def _is_first_party_source(
    *, source_path: Path, allowed_roots: tuple[Path, ...], memo: IdentityPass
) -> bool:
    resolved: Path = memo.resolved(source_path)
    if any(part in _IGNORED_PATH_PARTS for part in resolved.parts):
        return False
    return any(_is_relative_to(path=resolved, root=root, memo=memo) for root in allowed_roots)


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


def _display_path(*, source_path: Path | None, roots: tuple[Path, ...], memo: IdentityPass) -> str:
    if source_path is None:
        return ""
    resolved: Path = memo.resolved(source_path)
    root: Path
    for root in sorted(
        (memo.resolved(path) for path in roots),
        key=lambda item: len(item.parts),
        reverse=True,
    ):
        if _is_relative_to(path=resolved, root=root, memo=memo):
            return "/".join(resolved.parts[len(root.parts) :]) or "."
    return resolved.as_posix()


def _is_relative_to(*, path: Path, root: Path, memo: IdentityPass) -> bool:
    """`path.is_relative_to(root.resolve())`, comparing each path's parts computed once per pass."""

    root_parts: tuple[str, ...] = memo.parts(memo.resolved(root))
    return memo.parts(path)[: len(root_parts)] == root_parts


def _stable_json(value: object) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), default=repr)
