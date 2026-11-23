"""E2E coverage of the compile warning for date and time casts, which SQLite turns into numbers."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.sqlite._test_types import (
    SqliteTemporalCastE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteTemporalCastE2ETestCase(
            description="a timestamp cast warns once at the cast",
            expression="CAST(o.ordered_at AS TIMESTAMP)",
            expected_warning_count=1,
        ),
        SqliteTemporalCastE2ETestCase(
            description="a date cast around a text cast warns once",
            expression="CAST(CAST(o.ordered_at AS TEXT) AS DATE)",
            expected_warning_count=1,
        ),
        SqliteTemporalCastE2ETestCase(
            description="a SQLite date function does not warn",
            expression="datetime(o.ordered_at)",
            expected_warning_count=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sqlite_temporal_cast_when_compiling_then_b104_warns(
    test_case: SqliteTemporalCastE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "shop"\nadapter = "sqlite"\n\n[connection]\npath = "shop.sqlite"\n\n'
                "[scopes]\nenforce_placement = false\n"
            ),
            "sql/models/orders.sql": (
                "MODEL (description 'Test model orders.');\n"
                "SELECT 1 AS order_id, '2026-01-01 10:00:00' AS ordered_at\n"
            ),
            "sql/models/order_times.sql": (
                "MODEL (description 'Test model order_times.');\n"
                f'SELECT o.order_id, {test_case.expression} AS ordered_at FROM __ref("orders") AS o\n'
            ),
        },
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile", "--json"), project_dir=project_dir
    )

    assert compiled.returncode == 0, compiled.stdout + compiled.stderr
    report: dict[str, Any] = json.loads(compiled.stdout)
    located: list[tuple[str, str, int, int]] = [
        (item["code"], item["path"], item["line"], item["column"]) for item in report["diagnostics"]
    ]
    assert located.count(("B104", "sql/models/order_times.sql", 2, 20)) == (
        test_case.expected_warning_count
    ), test_case.description
    assert [item[0] for item in located].count("B104") == test_case.expected_warning_count
