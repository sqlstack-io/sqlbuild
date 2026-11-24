"""Apply a deterministic one-model edit that any generator's benchmark project accepts."""

from __future__ import annotations

from itertools import chain
from pathlib import Path

from scripts.compile_performance_ratio.constants import (
    EDIT_COMMENT,
    MODEL_FILE_PATTERN,
    MODELS_DIRECTORIES,
    STATEMENT_START,
)
from scripts.compile_performance_ratio.exceptions import BenchmarkEditError


def apply_one_model_edit(*, project_dir: Path, revision: int) -> Path:
    """Comment the middle model's query with the revision so each revision is a new edit."""

    models: list[Path] = sorted(
        chain.from_iterable(
            (project_dir / directory).rglob(MODEL_FILE_PATTERN) for directory in MODELS_DIRECTORIES
        ),
        key=lambda path: path.relative_to(project_dir).as_posix(),
    )
    if not models:
        raise BenchmarkEditError(f"{project_dir} has no model files to edit")
    target: Path = models[len(models) // 2]
    edited, count = STATEMENT_START.subn(
        EDIT_COMMENT.format(revision=revision) + r"\1",
        target.read_text(encoding="utf-8"),
        count=1,
    )
    if count != 1:
        raise BenchmarkEditError(f"{target} has no line starting a WITH or SELECT query to edit")
    _ = target.write_text(edited, encoding="utf-8")
    return target
