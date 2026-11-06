"""Postgres differential models."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class RunOptions:
    """Where the captures, the SQLQuack checkout and the results are."""

    work_dirs: tuple[tuple[str, Path], ...]
    sqlquack: Path
    out: Path
    mutations: bool
