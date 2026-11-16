"""`sqb test` and `sqb scenario test` report the warehouse statements they ran, by kind."""

import json
from pathlib import Path
from subprocess import CompletedProcess

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    build_scenario_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import (
    StatementSummaryE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\n\n[connection]\ndatabase = "orders.duckdb"\n'
    ),
    "sources/raw_orders.yml": (
        "sources:\n"
        "  - name: raw_orders\n    description: Test source raw_orders.\n"
        "    schema: main\n    table: raw_orders\n"
        "    columns:\n"
        "      - name: order_id\n        type: INTEGER\n"
        "      - name: amount\n        type: INTEGER\n"
    ),
    "models/orders.sql": (
        "MODEL (description 'Test model orders.');\n\n"
        'SELECT order_id, amount * 2 AS doubled FROM __source("raw_orders")\n'
    ),
}


def _test_file(name: str, doubled: int) -> str:
    return (
        f'TEST (name "{name}");\n\nWITH\n'
        "__source__raw_orders AS (\n  SELECT 1 AS order_id, 5 AS amount\n),\n"
        f"__expected__orders AS (\n  SELECT 1 AS order_id, {doubled} AS doubled\n)\nSELECT 1\n"
    )


@pytest.mark.parametrize(
    "test_case",
    (
        # Each test of one real model stores the model's output (CREATE), compares it with the
        # expected rows (WITH) and drops it (DROP).
        StatementSummaryE2ETestCase(
            description="two passing tests count three statements each",
            expected_doubled=(10, 10),
            expected_exit_code=0,
            expected_by_kind={"CREATE": 2, "DROP": 2, "WITH": 2},
            expected_line=(
                "Statements: 6 (CREATE 2, DROP 2, WITH 2); compile time not reported by duckdb"
            ),
        ),
        # A failed comparison also reads its unexpected and missing sample rows (two more WITH).
        StatementSummaryE2ETestCase(
            description="a failing test's statements count too",
            expected_doubled=(10, 99),
            expected_exit_code=1,
            expected_by_kind={"CREATE": 2, "DROP": 2, "WITH": 4},
            expected_line=(
                "Statements: 8 (WITH 4, CREATE 2, DROP 2); compile time not reported by duckdb"
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_sql_tests_when_running_then_statement_summary_counts_each_statement(
    tmp_path: Path, test_case: StatementSummaryE2ETestCase
) -> None:
    files: dict[str, str] = dict(_PROJECT_FILES)
    for index, doubled in enumerate(test_case.expected_doubled):
        files[f"tests/unit/test_orders_{index}.sql"] = _test_file(f"orders_{index}", doubled)
    project: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="statement_summary", repo_files=files
    )

    text_run: CompletedProcess[str] = run_sqb(command=("--no-color", "test"), project_dir=project)
    json_run: CompletedProcess[str] = run_sqb(
        command=("--no-color", "test", "--json"), project_dir=project
    )

    assert text_run.returncode == test_case.expected_exit_code, text_run.stdout + text_run.stderr
    assert test_case.expected_line in text_run.stdout.splitlines(), text_run.stdout
    assert json_run.returncode == test_case.expected_exit_code, json_run.stderr
    assert json.loads(json_run.stdout)["statements"] == {
        "total": sum(test_case.expected_by_kind.values()),
        "by_kind": dict(test_case.expected_by_kind),
        "compile_time": None,
    }
    assert test_case.expected_line in json_run.stderr.splitlines(), json_run.stderr


def test_given_scenarios_when_running_with_json_then_statement_summary_matches_progress_line(
    tmp_path: Path,
) -> None:
    project: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_statements",
        repo_files=build_scenario_project_files(),
    )

    result: CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "test", "--json"), project_dir=project
    )

    assert result.returncode == 1, result.stderr
    statements: dict[str, object] = json.loads(result.stdout)["statements"]
    by_kind: dict[str, int] = statements["by_kind"]  # type: ignore[assignment]
    assert statements["total"] == sum(by_kind.values()) and {"CREATE", "DROP"} <= set(by_kind)
    assert statements["compile_time"] is None
    kinds: str = ", ".join(
        f"{kind} {count}"
        for kind, count in sorted(by_kind.items(), key=lambda item: (-item[1], item[0]))
    )
    assert (
        f"Statements: {statements['total']} ({kinds}); compile time not reported by duckdb"
        in result.stderr.splitlines()
    ), result.stderr


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
