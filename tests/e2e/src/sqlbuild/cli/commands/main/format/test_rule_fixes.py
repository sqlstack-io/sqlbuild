"""Subprocess coverage for compiler-verified Rule repair and bounded fixed points."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.format._test_types import (
    RuleFixPerformanceTestCase,
    RuleFixTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        RuleFixTestCase(
            "verification failure",
            'MODEL (description "Test model.", sql_analysis false);\nWITH unused AS (SELECT 1 AS item) SELECT 2 AS item',
            "refused",
            "SQLCLEAN005",
            1,
            True,
            True,
        ),
        RuleFixTestCase(
            "generated dependency region",
            'MODEL (description "Test model.");\n'
            'WITH unused AS (SELECT customer_id FROM __ref("customers")) SELECT 2 AS item',
            "refused",
            "SQLCLEAN005",
            0,
            False,
            True,
        ),
        RuleFixTestCase(
            "fixed point with overlapping unused CTE deletions",
            'MODEL (description "Test model.");\n'
            "WITH first AS (SELECT 1 AS item), second AS (SELECT item FROM first), "
            "third AS (SELECT item FROM second) SELECT 2 AS item",
            "applied",
            "SQLCLEAN005",
            0,
            False,
            False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rule_fixes_when_running_cli_then_reports_verified_or_refused_changes(
    test_case: RuleFixTestCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text('name = "orders"\nadapter = "duckdb"\n')
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(test_case.sql)
    (model.parent / "customers.sql").write_text(
        'MODEL (description "Test model customers."); SELECT 1 AS customer_id'
    )
    result: subprocess.CompletedProcess[str] = subprocess.run(
        [
            str(Path(sys.executable).with_name("sqb")),
            "--project-dir",
            str(tmp_path),
            "format",
            "--fix",
            "--json",
        ],
        capture_output=True,
        text=True,
        timeout=20,
        check=False,
    )
    assert result.returncode == test_case.expected_returncode, result.stderr
    payload: dict[str, Any] = json.loads(result.stdout)
    assert any(
        fix["code"] == test_case.expected_code and fix["status"] == test_case.expected_status
        for fix in payload["rule_fixes"]
    )
    assert (model.read_text() == test_case.sql) == test_case.expected_unchanged
    assert ("WITH" in model.read_text()) == test_case.expected_with
    assert any(
        v["code"] == "format-fix-verification-failed" for v in payload["violations"]
    ) == bool(test_case.expected_returncode)
    assert "Formatting SQL" in result.stderr


@pytest.mark.performance
@pytest.mark.parametrize(
    "test_case",
    [RuleFixPerformanceTestCase("wide", 200, 8), RuleFixPerformanceTestCase("deep", 4, 100)],
    ids=lambda case: case.description,
)
def test_given_wide_or_deep_unused_ctes_when_fixing_then_work_is_bounded(
    test_case: RuleFixPerformanceTestCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text('name = "orders"\nadapter = "duckdb"\n')
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    columns: str = ", ".join(f"{index} AS item_{index}" for index in range(test_case.width))
    ctes: list[str] = [f"items_0 AS (SELECT {columns})"]
    for index in range(1, test_case.depth):
        ctes.append(f"items_{index} AS (SELECT * FROM items_{index - 1})")
    model.write_text(
        'MODEL (description "Test model."); WITH ' + ", ".join(ctes) + " SELECT 1 AS order_id"
    )
    result: subprocess.CompletedProcess[str] = subprocess.run(
        [
            str(Path(sys.executable).with_name("sqb")),
            "--project-dir",
            str(tmp_path),
            "format",
            "--fix",
            "--json",
        ],
        capture_output=True,
        text=True,
        timeout=20,
        check=False,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    assert ("WITH" in model.read_text()) == test_case.expected_with


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-vv"]))
