"""Read Python object source the way `inspect.getsource` does, parsing each file at most once."""

from __future__ import annotations

import ast
import hashlib
import inspect
import linecache
import sys
import textwrap
from pathlib import Path
from types import ModuleType
from typing import Any, cast

# Before Python 3.13, `inspect.findsource` parses the whole file again for every class it locates.
_LOCATES_CLASSES_BY_PARSING: bool = sys.version_info < (3, 13)


def normalized_source(obj: object, class_lines: dict[str, tuple[list[str], dict[str, int]]]) -> str:
    """`inspect.getsource(obj)`, dedented and stripped; empty when there is no source.

    `class_lines` holds each file's lines and class start lines, so a file is parsed at most once.
    """

    try:
        target: object = inspect.unwrap(cast(Any, obj))
        if not (_LOCATES_CLASSES_BY_PARSING and inspect.isclass(target)):
            return textwrap.dedent(inspect.getsource(cast(Any, obj))).strip()
        lines, start = _class_location(target, class_lines)
        return textwrap.dedent("".join(inspect.getblock(lines[start:]))).strip()
    except (OSError, TypeError):
        return ""


def source_file(obj: object) -> Path | None:
    """The file `inspect` reads `obj`'s source from, or `None` when it has none."""

    try:
        raw_path: str | None = inspect.getsourcefile(cast(Any, obj))
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


def _reference_parts(node: ast.AST) -> tuple[str, ...] | None:
    if isinstance(node, ast.Name):
        return (node.id,)
    if isinstance(node, ast.Attribute):
        parent: tuple[str, ...] | None = _reference_parts(node.value)
        if parent is None:
            return None
        return (*parent, node.attr)
    return None


def _class_location(
    cls: type, class_lines: dict[str, tuple[list[str], dict[str, int]]]
) -> tuple[list[str], int]:
    """The lines and start index `inspect.findsource` returns for `cls` on Python 3.12."""

    file: str | None = inspect.getsourcefile(cls)
    if file:
        linecache.checkcache(file)
    else:
        file = inspect.getfile(cls)
        if not (file.startswith("<") and file.endswith(">")):
            raise OSError("source code not available")
    module: ModuleType | None = inspect.getmodule(cls, file)
    lines: list[str] = (
        linecache.getlines(file, module.__dict__) if module else linecache.getlines(file)
    )
    if not lines:
        raise OSError("could not get source code")
    cached: tuple[list[str], dict[str, int]] | None = class_lines.get(file)
    if cached is None or cached[0] is not lines:
        cached = (lines, _class_starts(ast.parse("".join(lines))))
        class_lines[file] = cached
    start: int | None = cached[1].get(cls.__qualname__)
    if start is None:
        raise OSError("could not find class definition")
    return lines, start


def _class_starts(tree: ast.AST) -> dict[str, int]:
    """Each class qualname's first decorator or `class` line, first match in `inspect` order."""

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
