"""Opt-in performance guard for end-to-end Rules evaluation."""

from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    write_advanced_compile_project,
)
from tests.e2e.src.sqlbuild.cli.commands.main.rules._test_types import (
    RulesPerformanceGuardTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.rules.helpers import write_custom_rules


@pytest.mark.performance
@pytest.mark.parametrize(
    "test_case",
    [
        RulesPerformanceGuardTestCase(
            description="end-to-end Rules checks 3000 advanced models within hard budget",
            model_count=3000,
            hard_ceiling_seconds=20,
            expected_max_elapsed_seconds=15.0,
            expected_returncode=1,
        ),
        RulesPerformanceGuardTestCase(
            description="end-to-end Rules checks 5000 advanced models within hard budget",
            model_count=5000,
            hard_ceiling_seconds=35,
            expected_max_elapsed_seconds=24.0,
            expected_returncode=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_advanced_project_and_disabled_cache_when_running_rules_then_finishes_within_budget(
    tmp_path: Path,
    test_case: RulesPerformanceGuardTestCase,
) -> None:
    project_dir: Path = tmp_path / f"rules_advanced_{test_case.model_count}"
    write_advanced_compile_project(project_dir=project_dir, model_count=test_case.model_count)
    write_custom_rules(project_dir=project_dir)
    config_path: Path = project_dir / "sqlbuild_project.toml"
    config_path.write_text(
        config_path.read_text(encoding="utf-8")
        + ('\n[rules]\nselect = ["SQBR", "SQLCLEAN", "XSQBR"]\n[rules.cache]\nenabled = true\n'),
        encoding="utf-8",
    )

    started_at: float = time.perf_counter()
    result: subprocess.CompletedProcess[str] = subprocess.run(
        [
            str(Path(sys.executable).with_name("sqb")),
            "--project-dir",
            str(project_dir),
            "--no-color",
            "compile",
            "--json",
            "--no-cache",
        ],
        check=False,
        capture_output=True,
        text=True,
        timeout=test_case.hard_ceiling_seconds,
    )
    elapsed_seconds: float = time.perf_counter() - started_at

    assert result.returncode == test_case.expected_returncode
    assert '"compile_timings"' in result.stdout
    assert elapsed_seconds <= test_case.expected_max_elapsed_seconds
