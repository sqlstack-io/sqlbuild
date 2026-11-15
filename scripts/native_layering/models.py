"""Native layering policy models."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class NativeLayers:
    """Declared crate order, the crates allowed PyO3 and the first crate allowed polyglot."""

    order: tuple[str, ...]
    python_boundary: tuple[str, ...]
    polyglot_floor: str
