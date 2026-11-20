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
