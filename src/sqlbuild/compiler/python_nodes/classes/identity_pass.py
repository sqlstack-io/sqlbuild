"""Source facts that every Python-node identity built in one discovery pass shares."""

from __future__ import annotations

import inspect
from collections.abc import Callable
from pathlib import Path
from types import ModuleType
from typing import TypeVar

from sqlbuild.compiler.python_nodes._helpers.object_source import (
    called_references,
    hash_text,
    normalized_source,
    source_file,
)

_Fact = TypeVar("_Fact")


class IdentityPass:
    """Reads, parses, hashes and resolves each object or path once for one discovery pass.

    Tables keep the objects they describe alive, so an object id is never reused during the pass.
    """

    def __init__(self) -> None:
        self._source_paths: dict[int, tuple[object, Path | None]] = {}
        self._sources: dict[int, tuple[object, str]] = {}
        self._hashes: dict[int, tuple[object, str]] = {}
        self._references: dict[int, tuple[object, tuple[tuple[str, ...], ...]]] = {}
        self._modules: dict[int, tuple[object, ModuleType | None]] = {}
        self._class_lines: dict[str, tuple[list[str], dict[str, int]]] = {}
        self._resolved: dict[str, Path] = {}

    def source_path(self, obj: object) -> Path | None:
        """The file `inspect` reads `obj`'s source from."""

        return self._cached(self._source_paths, obj, source_file)

    def source(self, obj: object) -> str:
        """`obj`'s dedented, stripped source."""

        return self._cached(self._sources, obj, lambda item: normalized_source(item, self._class_lines))

    def source_hash(self, obj: object) -> str:
        """The SHA-256 of `obj`'s source."""

        return self._cached(self._hashes, obj, lambda item: hash_text(self.source(item)))

    def references(self, obj: object) -> tuple[tuple[str, ...], ...]:
        """The dotted names `obj`'s source calls."""

        return self._cached(self._references, obj, lambda item: called_references(self.source(item)))

    def module(self, obj: object) -> ModuleType | None:
        """The module `obj` belongs to."""

        return self._cached(self._modules, obj, inspect.getmodule)

    def resolved(self, path: Path) -> Path:
        """`path.resolve()`."""

        key: str = str(path)
        resolved: Path | None = self._resolved.get(key)
        if resolved is None:
            resolved = path.resolve()
            self._resolved[key] = resolved
        return resolved

    @staticmethod
    def _cached(
        table: dict[int, tuple[object, _Fact]], obj: object, compute: Callable[[object], _Fact]
    ) -> _Fact:
        entry: tuple[object, _Fact] | None = table.get(id(obj))
        if entry is None:
            entry = (obj, compute(obj))
            table[id(obj)] = entry
        return entry[1]
