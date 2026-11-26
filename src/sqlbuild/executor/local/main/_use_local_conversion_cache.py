"""Where a project's local conversions are kept between runs."""

from __future__ import annotations

from pathlib import Path

from sqlbuild.executor.local._helpers.conversion import use_conversion_cache
from sqlbuild.executor.local.constants import CONVERSION_CACHE_DIR


def use_local_conversion_cache(project_dir: Path) -> None:
    """Keep the project's conversions under its target directory across runs."""

    _ = use_conversion_cache(project_dir / CONVERSION_CACHE_DIR)
