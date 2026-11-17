"""The generated 10K-model DuckShop project the plan preview performance guard plans."""

import subprocess
import sys
from collections.abc import Iterator
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.plan.helpers import (
    DUCKSHOP_10K_GENERATOR,
    DUCKSHOP_10K_SETUP_SECONDS,
    served_duckshop_location,
    timed_plan,
)


@pytest.fixture(scope="module")
def duckshop_10k(tmp_path_factory: pytest.TempPathFactory) -> Iterator[tuple[Path, Path, Path]]:
    """The generated project, a planner capture of one plan of it, and the snapshot it published."""
    root: Path = tmp_path_factory.mktemp("duckshop_10k_preview")
    project: Path = root / "project"
    generated: subprocess.CompletedProcess[str] = subprocess.run(
        (sys.executable, str(DUCKSHOP_10K_GENERATOR / "generate.py"), "--out", str(project)),
        capture_output=True,
        text=True,
        check=False,
        timeout=DUCKSHOP_10K_SETUP_SECONDS,
    )
    assert generated.returncode == 0, generated.stdout + generated.stderr
    capture: Path = root / "capture"
    with served_duckshop_location(
        root / "calibration", project, env={"SQLBUILD_PLANNER_CAPTURE_DIR": str(capture)}
    ) as served:
        _ = timed_plan(served, [], {})
        yield project, next(capture.glob("*plan_output.json")), served.control
