"""E2E coverage that ClickHouse `#` and `//` line comments are comments to SQLBuild's scanners."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.rules._test_types import (
    ClickHouseCommentE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT_TOML: str = (
    'name = "shop"\nadapter = "clickhouse"\n\n[connection]\nhost = "localhost"\n'
    'database = "default"\n\n[defaults]\nschema = "analytics"\n\n'
    "[scopes]\nenforce_placement = false\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseCommentE2ETestCase(
            description="a hash comment with an apostrophe is skipped, reported unavailable until SQLBuild parses the dialect",
            comment="# don't keep the star",
            expected_location="models/orders.sql:1:1 [L007]",
        ),
        ClickHouseCommentE2ETestCase(
            description="a double-slash comment with an apostrophe is skipped, reported unavailable until SQLBuild parses the dialect",
            comment="// don't keep the star",
            expected_location="models/orders.sql:1:1 [L007]",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_clickhouse_line_comment_when_running_rules_then_it_is_not_scanned_as_code(
    test_case: ClickHouseCommentE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML,
            "models/orders.sql": (
                "MODEL (description 'Test model orders.', materialized table);\n"
                f"WITH o AS (SELECT 1 AS id)\n{test_case.comment}\nSELECT * FROM o\n"
            ),
        },
    )

    ran: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "rules", "run", "SQBRMODEL102"), project_dir=project_dir
    )

    output: str = ran.stdout + ran.stderr
    assert "P001" not in output, output
    assert test_case.expected_location in output, test_case.description
