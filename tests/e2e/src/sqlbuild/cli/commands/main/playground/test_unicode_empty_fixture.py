"""Unicode fixture exemptions through the generated playground and real CLI."""

import json
import re
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.playground._test_types import (
    UnicodeEmptyFixtureTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        UnicodeEmptyFixtureTestCase(text, text)
        for text in ("complété", "total €", "order 😀", "complété € 😀")
    ],
    ids=lambda case: case.description,
)
def test_given_unicode_playground_fixture_when_running_rules_and_format_then_empty_input_is_allowed(
    test_case: UnicodeEmptyFixtureTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "orders_playground"
    scaffold: subprocess.CompletedProcess[str] = run_sqb(
        command=("playground", str(project)), project_dir=tmp_path
    )
    assert scaffold.returncode == 0, scaffold.stdout + scaffold.stderr
    fixture: Path = project / "tests" / "unit" / "test_fact_orders.sql"
    sql: str = fixture.read_text(encoding="utf-8")
    sql = re.sub(
        r"__ref__stg_payments AS \(.*?\n\)",
        "__ref__stg_payments AS (\n  SELECT * FROM __EMPTY_FIXTURE()\n)",
        sql,
        flags=re.DOTALL,
    )
    fixture.write_text(sql, encoding="utf-8")
    baseline: subprocess.CompletedProcess[str] = run_sqb(
        command=("rules", "--json", "run", "SQLCLEAN021"), project_dir=project
    )
    baseline_count: int = json.loads(baseline.stdout)["finding_count"]
    fixture.write_text(
        sql.replace("'completed' AS status", f"'{test_case.status}' AS status"), encoding="utf-8"
    )
    before: subprocess.CompletedProcess[str] = run_sqb(
        command=("rules", "--json", "run", "SQLCLEAN021"), project_dir=project
    )
    assert before.returncode == baseline.returncode, before.stdout + before.stderr
    assert (
        json.loads(before.stdout)["finding_count"] - baseline_count
        == test_case.expected_added_findings
    )
    assert json.loads(before.stdout)["findings"] == json.loads(baseline.stdout)["findings"]
    formatted_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("format", "--json", str(fixture)), project_dir=project
    )
    assert formatted_result.returncode == 0, formatted_result.stdout + formatted_result.stderr
    formatted: str = fixture.read_text(encoding="utf-8")
    assert test_case.status in formatted
    assert "__EMPTY_FIXTURE()" in formatted
    after: subprocess.CompletedProcess[str] = run_sqb(
        command=("rules", "--json", "run", "SQLCLEAN021"), project_dir=project
    )
    assert after.returncode == baseline.returncode, after.stdout + after.stderr
    assert (
        json.loads(after.stdout)["finding_count"] - baseline_count
        == test_case.expected_added_findings
    )
    checked: subprocess.CompletedProcess[str] = run_sqb(
        command=("format", "--check", "--json", str(fixture)),
        project_dir=project,
    )
    assert checked.returncode == 0, checked.stdout + checked.stderr
    assert fixture.read_text(encoding="utf-8") == formatted


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
