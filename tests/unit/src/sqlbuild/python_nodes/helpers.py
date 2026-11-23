"""Write inline Python node modules under the file names their declarations require."""

from __future__ import annotations

import ast
import re
from pathlib import PurePosixPath

_NODE_DECORATORS: frozenset[str] = frozenset({"loader", "asset", "check", "factory"})
_KIND_FILES: dict[str, str] = {"loader": "loader.py", "asset": "asset.py"}
_FACTORY_FILES: dict[str, str] = {"loader": "loaders.py", "asset": "assets.py", "check": "checks.py"}
_CHECKS_FILE: str = "checks.py"
_MIXED_FACTORY_FILE: str = "factory.py"
_UNSEEN_FACTORY_FILE: str = "assets.py"
_INIT_FILE: str = "__init__.py"
_ROOT_PARENTS: re.Pattern[str] = re.compile(r"(Path\(__file__\)(?:\.resolve\(\))?\.parents\[)(\d+)(\])")
_MIN_ROOT_PARENTS: int = 2


def node_module_files(module_path: str, source: str) -> dict[str, str]:
    """The project files for the module `source` written at `module_path`.

    A module declaring nodes becomes a package at `module_path` without its suffix: one unit
    folder per loader, asset or factory, named for the declaration and holding the kind-named
    file, `checks.py` with the module's checks, and an `__init__.py` re-exporting every node so
    imports of the original module keep working. A module without nodes is written unchanged.
    Each file gets its own copy of module-level state, so state nodes share must live in a helper
    module they import.
    """

    tree: ast.Module = ast.parse(source)
    nodes: list[tuple[ast.FunctionDef | ast.AsyncFunctionDef, str]] = [
        (item, kind) for item in tree.body if (kind := _declared_kind(item)) is not None
    ]
    if not nodes:
        return {module_path: source}
    if any(isinstance(item, ast.ImportFrom) and item.level for item in ast.walk(tree)):
        raise ValueError(f"{module_path} declares nodes with relative imports")
    lines: list[str] = source.split("\n")
    names: frozenset[str] = frozenset(item.name for item, _ in nodes)
    others: list[ast.stmt] = [item for item in tree.body if _declared_kind(item) is None]
    package: PurePosixPath = PurePosixPath(module_path).with_suffix("")
    placed: dict[str, str] = {item.name: _piece_path(package, item, kind) for item, kind in nodes}
    preamble: str = "\n".join(_segment(lines, item) for item in others).strip("\n")
    pieces: dict[str, list[ast.FunctionDef | ast.AsyncFunctionDef]] = {}
    for item, _ in nodes:
        pieces.setdefault(placed[item.name], []).append(item)
    files: dict[str, str] = {
        piece: _piece_source(
            preamble=preamble,
            imports=_sibling_imports(piece=piece, members=members, names=names, placed=placed),
            body="\n\n\n".join(_segment(lines, member) for member in members),
            depth=len(PurePosixPath(piece).parts) - 1,
        )
        for piece, members in pieces.items()
    }
    files[str(package / _INIT_FILE)] = "".join(
        f"from .{'.'.join(PurePosixPath(placed[item.name]).relative_to(package).with_suffix('').parts)}"
        f" import {item.name}\n"
        for item, _ in nodes
    )
    return files


def _declared_kind(item: ast.stmt) -> str | None:
    if not isinstance(item, (ast.FunctionDef, ast.AsyncFunctionDef)):
        return None
    for decorator in item.decorator_list:
        target: ast.expr = decorator.func if isinstance(decorator, ast.Call) else decorator
        name: str | None = target.id if isinstance(target, ast.Name) else getattr(target, "attr", None)
        if name in _NODE_DECORATORS:
            return name
    return None


def _piece_path(
    package: PurePosixPath, item: ast.FunctionDef | ast.AsyncFunctionDef, kind: str
) -> str:
    if kind == "check":
        return str(package / _CHECKS_FILE)
    if kind != "factory":
        return str(package / item.name / _KIND_FILES[kind])
    inner: set[str] = {
        found
        for child in ast.walk(item)
        if child is not item and (found := _declared_kind(child)) is not None and found != "factory"
    }
    file_name: str = (
        _UNSEEN_FACTORY_FILE
        if not inner
        else _FACTORY_FILES[next(iter(inner))]
        if len(inner) == 1
        else _MIXED_FACTORY_FILE
    )
    return str(package / item.name / file_name)


def _sibling_imports(
    *,
    piece: str,
    members: list[ast.FunctionDef | ast.AsyncFunctionDef],
    names: frozenset[str],
    placed: dict[str, str],
) -> str:
    used: set[str] = {
        child.id for member in members for child in ast.walk(member) if isinstance(child, ast.Name)
    }
    return "".join(
        f"from {PurePosixPath(placed[name]).with_suffix('').as_posix().replace('/', '.')} "
        f"import {name}\n"
        for name in sorted(used & names)
        if placed[name] != piece
    )


def _piece_source(*, preamble: str, imports: str, body: str, depth: int) -> str:
    head: str = "\n".join(part for part in (preamble, imports.rstrip("\n")) if part)
    text: str = (f"{head}\n\n\n" if head else "") + body + "\n"
    return _ROOT_PARENTS.sub(
        lambda match: match.group(1)
        + (str(depth) if int(match.group(2)) >= _MIN_ROOT_PARENTS else match.group(2))
        + match.group(3),
        text,
    )


def _segment(lines: list[str], item: ast.stmt) -> str:
    start: int = item.lineno
    if isinstance(item, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)) and item.decorator_list:
        start = min(decorator.lineno for decorator in item.decorator_list)
    return "\n".join(lines[start - 1 : item.end_lineno])
