"""Data models for SQLBuild custom Fensu rules."""

from __future__ import annotations

from dataclasses import dataclass

from scripts.fensu_policy.constants import NATIVE_LAYER_WILDCARD


@dataclass(frozen=True)
class LayerReference:
    """One reference from a helper file to a helper module path."""

    line: int
    target: tuple[str, ...]


@dataclass(frozen=True)
class LayerPlan:
    """Layer assignments of helper module paths and the rank of each layer."""

    assignments: dict[tuple[str, ...], str]
    ranks: dict[str, int]

    def layer_of(self, *, module: tuple[str, ...]) -> str | None:
        """The layer of the longest assigned prefix of `module`; `*` matches any one segment."""

        best: tuple[int, int, str] | None = None
        pattern: tuple[str, ...]
        layer: str
        for pattern, layer in self.assignments.items():
            if len(pattern) > len(module) or not all(
                part in (NATIVE_LAYER_WILDCARD, segment)
                for part, segment in zip(pattern, module, strict=False)
            ):
                continue
            exact: int = sum(part != NATIVE_LAYER_WILDCARD for part in pattern)
            key: tuple[int, int, str] = (len(pattern), exact, layer)
            if best is None or key[:2] > best[:2]:
                best = key
        return None if best is None else best[2]

    def points_upward(self, *, source: str, target: str) -> bool:
        """Whether an import from layer `source` into layer `target` breaks the order."""

        if source == target:
            return False
        return self.ranks[target] >= self.ranks[source]


@dataclass(frozen=True)
class StateDefinition:
    """One control-store table, or one of its columns when `column` is set."""

    path: str
    line: int
    table: str
    column: str | None = None


@dataclass(frozen=True)
class StateReads:
    """Tables and `(table, column)` pairs that production SQL reads."""

    tables: set[str]
    columns: set[tuple[str, str]]


@dataclass(frozen=True)
class EnumVariant:
    """One Rust enum variant and the offsets of its enum's body in the declaring file."""

    path: str
    line: int
    enum: str
    variant: str
    body: tuple[int, int]
