"""Read Python object source the way `inspect.getsource` does, parsing each file at most once."""

from __future__ import annotations

import ast
import hashlib
import inspect
import linecache
import textwrap
from collections.abc import Callable
from pathlib import Path
from types import FunctionType, ModuleType
from typing import Any, cast


def normalized_source(*, obj: object, class_starts: Callable[..., dict[str, int]]) -> str:
    """`inspect.getsource(obj)` dedented and stripped; classes read from their own file."""

    try:
        target: object = inspect.unwrap(cast(Any, obj))
        if not inspect.isclass(target):
            return textwrap.dedent(inspect.getsource(cast(Any, obj))).strip()
        lines, start = _class_location(cast(type, target), class_starts=class_starts)
        return textwrap.dedent("".join(inspect.getblock(lines[start:]))).strip()
    except (OSError, TypeError):
        return ""


def source_file(obj: object) -> Path | None:
    """The file `inspect` reads `obj`'s source from, or `None` when it has none."""

    try:
        raw_path: str | None = inspect.getsourcefile(cast(Any, class_anchor(obj) or obj))
    except TypeError:
        return None
    if raw_path is None:
        return None
    return Path(raw_path)


def hash_text(value: str) -> str:
    """The SHA-256 hex digest of `value`."""

    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def called_references(source: str) -> tuple[tuple[str, ...], ...]:
    """The dotted names `source` calls, sorted."""

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


def class_anchor(obj: object) -> FunctionType | None:
    """A method of class `obj`; its file and globals outlive the module's sys.modules entry."""

    if not inspect.isclass(obj):
        return None
    value: object
    for value in vars(obj).values():
        method: object = value.__func__ if isinstance(value, staticmethod | classmethod) else value
        if not isinstance(method, FunctionType):
            continue
        function: object = inspect.unwrap(method)
        if (
            isinstance(function, FunctionType)
            and function.__qualname__.startswith(f"{obj.__qualname__}.")
            and function.__module__ == obj.__module__
            and not function.__code__.co_filename.startswith("<")
        ):
            return function
    return None


def _reference_parts(node: ast.AST) -> tuple[str, ...] | None:
    if isinstance(node, ast.Name):
        return (node.id,)
    if isinstance(node, ast.Attribute):
        parent: tuple[str, ...] | None = _reference_parts(node.value)
        if parent is None:
            return None
        return (*parent, node.attr)
    return None


def parse_class_starts(lines: list[str]) -> dict[str, int]:
    """Each class qualname's first decorator or `class` line index, as `inspect` finds it."""

    return _class_starts(ast.parse("".join(lines)))


def _class_location(
    cls: type, *, class_starts: Callable[..., dict[str, int]]
) -> tuple[list[str], int]:
    """The lines and start index `inspect.findsource` returns for `cls`, read from its own file."""

    path: Path | None = source_file(cls)
    if path is None:
        raise OSError("source code not available")
    file: str = str(path)
    linecache.checkcache(file)
    module: ModuleType | None = inspect.getmodule(cls, file)
    lines: list[str] = (
        linecache.getlines(file, module.__dict__) if module else linecache.getlines(file)
    )
    if not lines:
        raise OSError("could not get source code")
    first_line: object = vars(cls).get("__firstlineno__")
    if isinstance(first_line, int):
        return lines, first_line - 1
    start: int | None = class_starts(file=file, lines=lines).get(cls.__qualname__)
    if start is None:
        raise OSError("could not find class definition")
    return lines, start


def _class_starts(tree: ast.AST) -> dict[str, int]:
    starts: dict[str, int] = {}
    pending: list[tuple[ast.AST, tuple[str, ...]]] = [(tree, ())]
    while pending:
        node, scope = pending.pop()
        inner: tuple[str, ...] = scope
        if isinstance(node, ast.ClassDef):
            inner = (*scope, node.name)
            start: int = (node.decorator_list[0] if node.decorator_list else node).lineno
            starts.setdefault(".".join(inner), start - 1)
        elif isinstance(node, ast.FunctionDef | ast.AsyncFunctionDef):
            inner = (*scope, node.name, "<locals>")
        pending.extend((child, inner) for child in reversed(list(ast.iter_child_nodes(node))))
    return starts
