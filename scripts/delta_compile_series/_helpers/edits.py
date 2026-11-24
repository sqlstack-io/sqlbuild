"""Apply and revert the edits of a delta compile series."""

from __future__ import annotations

import random
import re
from pathlib import Path

from scripts.delta_compile_series.constants import (
    EDIT_ATTEMPTS,
    EDIT_ROOTS,
    PRIVATE_PATH_MARKER,
    PROJECT_CONFIG,
    RANDOM_VALUE_LIMIT,
    SQL_GLOB,
)
from scripts.delta_compile_series.exceptions import DeltaCompileSeriesError
from scripts.delta_compile_series.models import DeltaCompileEdit
from scripts.delta_compile_series.types import EditTarget


def project_files(*, project_dir: Path, edit: DeltaCompileEdit) -> list[Path]:
    """The files `edit` may change."""

    if edit.target is EditTarget.PROJECT_CONFIG:
        return [project_dir / PROJECT_CONFIG]
    root: Path = project_dir / EDIT_ROOTS[edit.target]
    return sorted(
        path for path in root.rglob(SQL_GLOB) if PRIVATE_PATH_MARKER not in path.as_posix()
    )


def applied(*, path: Path, edit: DeltaCompileEdit, rng: random.Random) -> bool:
    """Apply `edit` to `path`; return whether it changed the file."""

    text: str = path.read_text(encoding="utf-8")
    replacement: str = edit.replacement.replace("{value}", str(rng.randint(1, RANDOM_VALUE_LIMIT)))
    edited, count = re.subn(edit.pattern, replacement, text, count=1)
    if count == 0 or edited == text:
        return False
    path.write_text(edited, encoding="utf-8")
    return True


def edited_file(
    *, project_dir: Path, edit: DeltaCompileEdit, rng: random.Random
) -> tuple[Path, str]:
    """Apply `edit` to a randomly chosen file it changes; return the file and its old text."""

    candidates: list[Path] = project_files(project_dir=project_dir, edit=edit)
    for _ in range(EDIT_ATTEMPTS):
        path: Path = rng.choice(candidates)
        original: str = path.read_text(encoding="utf-8")
        if applied(path=path, edit=edit, rng=rng):
            return path, original
    raise DeltaCompileSeriesError(f"no file takes the {edit.kind} edit")
