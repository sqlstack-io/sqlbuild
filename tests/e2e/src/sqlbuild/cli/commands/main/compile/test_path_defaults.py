"""E2E coverage for path-default glob selection during compile."""

from __future__ import annotations

import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    PathDefaultCompileTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    (
        PathDefaultCompileTestCase(
            description="recursive staging glob supplies model schema offline",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "path_default_globs"
                    adapter = "duckdb"
                    default_target = "dev"

                    [connection]
                    database = ":memory:"

                    [path_defaults."commerce/**/staging"]
                    schema = "staging"

                    [targets.dev]
                    schema = "preserve"
                    """
                ).strip()
                + "\n",
                "sql/models/commerce/partner_feed/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nSELECT 1 AS id\n",
            },
            expected_exit_code=0,
            expected_stderr_fragment="",
        ),
        PathDefaultCompileTestCase(
            description="ambiguous globs fail offline with structured discovery diagnostic",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                    name = "path_default_globs"
                    adapter = "duckdb"

                    [connection]
                    database = ":memory:"

                    [path_defaults."commerce/*/staging"]
                    schema = "shared"

                    [path_defaults."commerce/eu/*"]
                    schema = "shared"
                    """
                ).strip()
                + "\n",
                "sql/models/commerce/eu/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nSELECT 1 AS id\n",
            },
            expected_exit_code=1,
            expected_stderr_fragment=(
                "error[D007]: Model path 'commerce/eu/staging/orders.sql' matches equally specific "
                "path_defaults keys"
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_path_default_globs_when_running_compile_then_selection_is_deterministic(
    test_case: PathDefaultCompileTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="path_default_globs",
        repo_files=test_case.repo_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert test_case.expected_stderr_fragment in result.stderr
