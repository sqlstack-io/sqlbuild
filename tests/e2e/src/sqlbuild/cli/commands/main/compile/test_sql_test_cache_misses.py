"""E2E coverage that a warm compile logs why it could not keep cached SQL test artifacts."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    SqlTestCacheMissE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    COMPILE_CACHE_REGION_ENV_VAR,
    append_to_compiled_sql_tests,
    flip_compiled_sql_test_bytes,
    run_installed_sqb,
    truncate_sql_test_cache,
)

_MISS_LINE: str = "SQL test artifact cache missed"
_ENV: dict[str, str] = {COMPILE_CACHE_REGION_ENV_VAR: "east"}


@pytest.mark.parametrize(
    "test_case",
    [
        SqlTestCacheMissE2ETestCase(
            description="resized artifacts log size_changed",
            change=append_to_compiled_sql_tests,
            expected_debug_line=f"{_MISS_LINE}: size_changed=",
        ),
        SqlTestCacheMissE2ETestCase(
            description="same-size rewritten artifacts log content_changed",
            change=flip_compiled_sql_test_bytes,
            expected_debug_line=f"{_MISS_LINE}: content_changed=",
        ),
        SqlTestCacheMissE2ETestCase(
            description="a cut-short cache file logs that it held no usable records",
            change=truncate_sql_test_cache,
            expected_debug_line="(the cache file held no usable records)",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_changed_sql_test_artifacts_when_compiling_warm_then_debug_log_names_the_miss(
    cache_invalidation_project: Path, test_case: SqlTestCacheMissE2ETestCase
) -> None:
    project_dir: Path = cache_invalidation_project
    debug_compile: tuple[str, ...] = ("--debug", "compile")
    cold: subprocess.CompletedProcess[str] = run_installed_sqb(
        project_dir=project_dir, args=debug_compile, env=_ENV
    )
    warm: subprocess.CompletedProcess[str] = run_installed_sqb(
        project_dir=project_dir, args=debug_compile, env=_ENV
    )

    test_case.change(project_dir)
    changed: subprocess.CompletedProcess[str] = run_installed_sqb(
        project_dir=project_dir, args=debug_compile, env=_ENV
    )

    assert (cold.returncode, warm.returncode, changed.returncode) == (0, 0, 0), changed.stderr
    assert _MISS_LINE not in warm.stderr
    assert test_case.expected_debug_line in changed.stderr, changed.stderr[-2000:]
