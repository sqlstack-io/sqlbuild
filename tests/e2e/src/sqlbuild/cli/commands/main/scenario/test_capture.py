"""E2E tests for sqb scenario capture command."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.scenario._test_types import (
    ScenarioCliE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    assert_scenario_snapshot,
    build_capture_safety_project_files,
    build_scenario_project_files,
    list_scenario_relation_names,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioCliE2ETestCase(
            description="capture rejects disabled sql_analysis",
            command=(
                "--no-color",
                "scenario",
                "capture",
                "order_totals_pass",
            ),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[C455]: scenario capture requires SQL analysis; "
                "sqlbuild_project.toml sets [settings] sql_analysis = false",
                "= help: to capture snapshots for local scenario replay, "
                "set this in sqlbuild_project.toml:\n"
                "            [settings]\n"
                "            sql_analysis = true",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_capture_command_when_sql_validation_disabled_then_fails_clearly(
    test_case: ScenarioCliE2ETestCase,
    tmp_path: Path,
) -> None:
    repo_files: dict[str, str] = build_scenario_project_files()
    repo_files["sqlbuild_project.toml"] += "\n[settings]\nsql_analysis = false\n"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_project",
        repo_files=repo_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    expected_stderr_fragment: str
    for expected_stderr_fragment in test_case.expected_stderr_fragments:
        assert expected_stderr_fragment in result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioCliE2ETestCase(
            description="captures multiple selected scenarios by name and path",
            command=(
                "--no-color",
                "scenario",
                "capture",
                "order_totals_pass",
                "tests/scenarios/nested/orders_assert_pass.sql",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Execution  sqb scenario capture",
                "Scenario Capture (2 selected)",
                "order_totals_pass",
                "orders_assert_pass",
                "1 relation, 2 rows",
                "1 relation, 1 row",
                "source   raw_orders",
                "2 rows, 41 B",
                "1 row, 21 B",
                "snapshot tests/_scenario_snapshots/order_totals_pass/scenario.json",
                "snapshot tests/_scenario_snapshots/orders_assert_pass/scenario.json",
                "PASS=2  FAIL=0  TOTAL=2",
            ),
            expected_retained_prefix_count=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_selected_scenarios_when_running_capture_then_writes_snapshots(
    test_case: ScenarioCliE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_project",
        repo_files=build_capture_safety_project_files(
            use_project_row_limit="project snapshot row limit" in test_case.description,
        ),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    expected_stdout_fragment: str
    for expected_stdout_fragment in test_case.expected_stdout_fragments:
        assert expected_stdout_fragment in result.stdout
    retained_names: tuple[str, ...] = list_scenario_relation_names(
        db_path=project_dir / "scenario_demo.duckdb"
    )
    assert len(retained_names) == test_case.expected_retained_prefix_count
    assert_scenario_snapshot(
        project_dir=project_dir,
        scenario_name="order_totals_pass",
        expected_row_count=2,
    )
    assert_scenario_snapshot(
        project_dir=project_dir,
        scenario_name="orders_assert_pass",
        expected_row_count=1,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioCliE2ETestCase(
            description="captures scenario folder selector and retains materialized input",
            command=("--no-color", "scenario", "capture", "nested", "--retain"),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Scenario Capture (1 selected)",
                "orders_assert_pass",
                "1 relation, 1 row",
                "source   raw_orders",
                "1 row, 21 B",
                "Retained relations:",
                "raw_orders -> main.__sqb_",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
            expected_retained_prefix_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_folder_selector_when_running_capture_with_retain_then_keeps_input_artifacts(
    test_case: ScenarioCliE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_project",
        repo_files=build_scenario_project_files(),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    expected_stdout_fragment: str
    for expected_stdout_fragment in test_case.expected_stdout_fragments:
        assert expected_stdout_fragment in result.stdout
    retained_names: tuple[str, ...] = list_scenario_relation_names(
        db_path=project_dir / "scenario_demo.duckdb"
    )
    assert len(retained_names) == test_case.expected_retained_prefix_count
    assert retained_names[0].endswith("__source__raw_orders")
    assert_scenario_snapshot(
        project_dir=project_dir,
        scenario_name="orders_assert_pass",
        expected_row_count=1,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioCliE2ETestCase(
            description="capture applies local type overrides from project config",
            command=("--no-color", "scenario", "capture", "order_totals_pass"),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Scenario Capture (1 selected)",
                "order_totals_pass",
                "1 relation, 2 rows",
                "source   raw_orders",
                "2 rows",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
            expected_retained_prefix_count=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_local_type_override_when_running_capture_then_manifest_uses_override(
    test_case: ScenarioCliE2ETestCase,
    tmp_path: Path,
) -> None:
    repo_files: dict[str, str] = build_scenario_project_files()
    repo_files["sqlbuild_project.toml"] += (
        "\n[scenario.local_type_overrides.duckdb]\n"
        '"INTEGER" = "BIGINT"\n'
        '"DECIMAL(*,0)" = "BIGINT"\n'
        '"DECIMAL(*,*)" = "DECIMAL({1}, {2})"\n'
    )
    repo_files["tests/scenarios/order_totals_pass.sql"] = (
        'SCENARIO (description "Order totals scenario", tags ["scenario"]);\n\n'
        "WITH\n"
        "__source__raw_orders AS (\n"
        "  SELECT\n"
        "    CAST(1 AS INTEGER) AS id,\n"
        "    CAST(10 AS DECIMAL(12,0)) AS amount,\n"
        "    CAST(1.25 AS DECIMAL(8,2)) AS tax\n"
        "  UNION ALL\n"
        "  SELECT\n"
        "    CAST(2 AS INTEGER) AS id,\n"
        "    CAST(5 AS DECIMAL(12,0)) AS amount,\n"
        "    CAST(0.75 AS DECIMAL(8,2)) AS tax\n"
        "),\n"
        "__expected__order_totals AS (\n"
        "  SELECT 15 AS total_amount\n"
        ")\n"
        "SELECT 1\n"
    )
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_project",
        repo_files=repo_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    expected_stdout_fragment: str
    for expected_stdout_fragment in test_case.expected_stdout_fragments:
        assert expected_stdout_fragment in result.stdout
    assert_scenario_snapshot(
        project_dir=project_dir,
        scenario_name="order_totals_pass",
        expected_row_count=2,
        expected_local_types={"id": "BIGINT", "amount": "BIGINT", "tax": "DECIMAL(8, 2)"},
    )


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioCliE2ETestCase(
            description="capture fails before writing snapshot when row limit is exceeded",
            command=(
                "--no-color",
                "scenario",
                "capture",
                "order_totals_pass",
                "--max-snapshot-rows",
                "1",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "Review captured scenario snapshots before committing",
                "error[X512]:",
                "exceeding the per-relation capture limit of 1 rows",
                "PASS=0  FAIL=1  TOTAL=1",
            ),
        ),
        ScenarioCliE2ETestCase(
            description="capture force bypasses row limit",
            command=(
                "--no-color",
                "scenario",
                "capture",
                "order_totals_pass",
                "--max-snapshot-rows",
                "1",
                "--force",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Review captured scenario snapshots before committing",
                "Size limits are bypassed.",
                "order_totals_pass",
                "1 relation, 2 rows",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
        ),
        ScenarioCliE2ETestCase(
            description="capture uses project snapshot row limit",
            command=("--no-color", "scenario", "capture", "order_totals_pass"),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "error[X512]:",
                "exceeding the per-relation capture limit of 1 rows",
                "PASS=0  FAIL=1  TOTAL=1",
            ),
        ),
        ScenarioCliE2ETestCase(
            description="capture CLI row limit overrides project snapshot row limit",
            command=(
                "--no-color",
                "scenario",
                "capture",
                "order_totals_pass",
                "--max-snapshot-rows",
                "2",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "order_totals_pass",
                "1 relation, 2 rows",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_capture_safety_options_when_running_capture_then_enforces_limits(
    test_case: ScenarioCliE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_project",
        repo_files=build_capture_safety_project_files(
            use_project_row_limit="project snapshot row limit" in test_case.description,
        ),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    expected_stdout_fragment: str
    for expected_stdout_fragment in test_case.expected_stdout_fragments:
        assert expected_stdout_fragment in result.stdout
    snapshot_path: Path = (
        project_dir
        / "tests"
        / "_scenario_snapshots"
        / "order_totals_pass"
        / "sources"
        / "raw_orders.jsonl"
    )
    assert snapshot_path.exists() is (test_case.expected_exit_code == 0)
