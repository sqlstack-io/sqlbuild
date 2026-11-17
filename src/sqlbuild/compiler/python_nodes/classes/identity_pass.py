"""Source facts that every Python-node identity built in one discovery pass shares."""

from __future__ import annotations

import inspect
import os
from collections.abc import Callable
from pathlib import Path
from types import ModuleType
from typing import cast

from sqlbuild.compiler.python_nodes._helpers.object_source import (
    called_references,
    hash_text,
    normalized_source,
    parse_class_starts,
    source_file,
)


class IdentityPass:
    """Read, parse, hash and resolve each object or path once; facts keep their objects alive."""

    def __init__(self) -> None:
        self._facts: dict[tuple[str, int], tuple[object, object]] = {}
        self._class_starts: dict[str, tuple[list[str], dict[str, int]]] = {}
        self._resolved: dict[str, Path] = {}
        self._parts: dict[str, tuple[str, ...]] = {}

    def source_path(self, obj: object) -> Path | None:
        """The file `inspect` reads `obj`'s source from."""

        return self._fact(name="source_path", obj=obj, compute=source_file)

    def source(self, obj: object) -> str:
        """`obj`'s dedented, stripped source."""

        return self._fact(
            name="source",
            obj=obj,
            compute=lambda item: normalized_source(obj=item, class_starts=self._starts),
        )

    def source_hash(self, obj: object) -> str:
        """The SHA-256 of `obj`'s source."""

        return self._fact(
            name="source_hash", obj=obj, compute=lambda item: hash_text(self.source(item))
        )

    def references(self, obj: object) -> tuple[tuple[str, ...], ...]:
        """The dotted names `obj`'s source calls."""

        return self._fact(
            name="references", obj=obj, compute=lambda item: called_references(self.source(item))
        )

    def module(self, obj: object) -> ModuleType | None:
        """The module `obj` belongs to."""

        return self._fact(name="module", obj=obj, compute=inspect.getmodule)

    def resolved(self, path: Path) -> Path:
        """`path.resolve()`."""

        key: str = str(path)
        resolved: Path | None = self._resolved.get(key)
        if resolved is None:
            resolved = path.resolve()
            self._resolved[key] = resolved
        return resolved

    def parts(self, path: Path) -> tuple[str, ...]:
        """`path.parts` as `PurePath` equality compares them, case-folded where paths are."""

        key: str = str(path)
        parts: tuple[str, ...] | None = self._parts.get(key)
        if parts is None:
            parts = tuple(os.path.normcase(part) for part in path.parts)
            self._parts[key] = parts
        return parts

    def _starts(self, *, file: str, lines: list[str]) -> dict[str, int]:
        cached: tuple[list[str], dict[str, int]] | None = self._class_starts.get(file)
        if cached is None or cached[0] is not lines:
            cached = (lines, parse_class_starts(lines))
            self._class_starts[file] = cached
        return cached[1]

    def _fact[Fact](self, *, name: str, obj: object, compute: Callable[[object], Fact]) -> Fact:
        entry: tuple[object, object] | None = self._facts.get((name, id(obj)))
        if entry is None:
            entry = (obj, compute(obj))
            self._facts[(name, id(obj))] = entry
        return cast(Fact, entry[1])
