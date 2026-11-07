"""SQL Server differential models."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class RunOptions:
    """Where the SQL comes from, the SQLQuack checkout and where results go."""

    work_dirs: tuple[tuple[str, Path], ...]
    listed: tuple[tuple[str, Path], ...]
    recorded: tuple[tuple[str, Path], ...]
    sqlquack: Path
    out: Path
