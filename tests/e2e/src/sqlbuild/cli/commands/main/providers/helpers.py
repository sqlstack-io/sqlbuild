"""Ways to name a provider test project on the command line."""

from __future__ import annotations

import os
from pathlib import Path

from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import REPO_ROOT


def linked_project(project_dir: Path) -> Path:
    """A symlink to the project, beside it."""
    link: Path = project_dir.with_name(f"{project_dir.name}_link")
    link.symlink_to(project_dir, target_is_directory=True)
    return link


def relative_project(project_dir: Path) -> Path:
    """The project relative to the directory sqb runs in."""
    return Path(os.path.relpath(project_dir, REPO_ROOT))


def blocked(entry: dict[str, object]) -> dict[str, object]:
    """What a `cant_run` entry says can't run and why, without the file and env-file fields."""
    return {field: entry[field] for field in ("provider", "variables", "nodes", "readers", "sinks")}
