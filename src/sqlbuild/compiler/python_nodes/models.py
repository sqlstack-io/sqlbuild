"""Records of Python-node identity computation."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from sqlbuild._native import PythonIdentityDependency


@dataclass(frozen=True)
class ReachedWalk:
    """One walk's first-party dependencies, the module executions count and roots it ran under."""

    executions: int
    allowed_roots: tuple[Path, ...]
    dependencies: dict[tuple[str, str], PythonIdentityDependency]
