"""Convert local test SQL with the project's SQLQuack allowances and overrides."""

from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path

from sqlbuild.executor.local._helpers.conversion import use_local_inputs
from sqlbuild.executor.local._helpers.local_inputs import local_inputs


def use_local_test_inputs(*, project_dir: Path, allow: Mapping[str, bool]) -> None:
    """Convert with `[testing.local] allow` and `tests/overrides`, which key every conversion."""

    _ = use_local_inputs(local_inputs(project_dir=project_dir, allow=allow))
